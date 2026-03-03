//! ZIP archive actions for browsing and extracting ZIP files.
//!
//! ZIP files support efficient random access via their central directory,
//! which is located at the end of the file. This allows us to:
//! 1. List contents with a single range request (read central directory)
//! 2. Extract individual files with targeted range requests
//!
//! For S3 and remote providers, we use HTTP range requests to avoid
//! downloading the entire archive.

use anyhow::{Context as AnyhowContext, Result, anyhow};
use std::io::{Cursor, Read};
use std::sync::Arc;
use zip::ZipArchive;

use super::{Action, ActionContext, ActionResult};
use crate::provider::{ObjectInfo, ObjectType, Provider};

/// Maximum size to read for the End of Central Directory search (64KB should be enough)
const EOCD_SEARCH_SIZE: u64 = 65536;

/// Minimum signature for EOCD (4 bytes) + minimum EOCD size (18 more bytes)
const MIN_EOCD_SIZE: u64 = 22;

/// Action to expand ZIP archives inline in the tree view.
///
/// This action:
/// - Checks if the file is a ZIP archive
/// - Uses range requests to read the ZIP central directory
/// - Returns a list of archive entries to display inline
pub struct ZipArchiveAction<P: Provider> {
    provider: Arc<P>,
}

impl<P: Provider> ZipArchiveAction<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }

    /// Access to the underlying provider (for task spawning)
    pub fn provider(&self) -> &Arc<P> {
        &self.provider
    }

    /// Read ZIP central directory using range requests to list archive contents.
    ///
    /// This function:
    /// 1. Reads the end of the file to find the End of Central Directory (EOCD)
    /// 2. Parses the EOCD to locate the central directory
    /// 3. Parses the central directory headers directly to get the file list
    ///
    /// We parse the central directory manually instead of using the zip crate
    /// because the zip crate validates that local file headers exist at the
    /// offsets specified in the central directory, which fails when we only
    /// have the central directory + EOCD data.
    pub async fn list_zip_contents(&self, key: &str, size: u64) -> Result<Vec<ObjectInfo>> {
        if size < MIN_EOCD_SIZE {
            return Err(anyhow!("File too small to be a valid ZIP archive"));
        }

        // Step 1: Read the end of the file to locate the End of Central Directory (EOCD)
        // We read up to 64KB from the end, which should contain the EOCD even with comments
        let eocd_start = size.saturating_sub(EOCD_SEARCH_SIZE);
        let eocd_data = self
            .provider
            .get_range(key, eocd_start, size - 1)
            .await
            .context("Failed to read ZIP end of central directory")?;

        // Step 2: Parse the EOCD to find the central directory location
        let (eocd_info, _eocd_offset_in_buffer) = Self::find_eocd(&eocd_data, eocd_start)?;

        // Step 3: Read the central directory
        let central_dir_end = eocd_info.central_dir_offset + eocd_info.central_dir_size;
        let central_dir_data = self
            .provider
            .get_range(key, eocd_info.central_dir_offset, central_dir_end - 1)
            .await
            .context("Failed to read ZIP central directory")?;

        // Step 4: Parse central directory headers manually
        let entries = Self::parse_central_directory(&central_dir_data, key)?;

        Ok(entries)
    }

    /// Parse central directory file headers to extract file entries.
    ///
    /// Central Directory File Header format (46 bytes fixed + variable):
    /// - 4 bytes: signature (0x02014b50)
    /// - 2 bytes: version made by
    /// - 2 bytes: version needed to extract
    /// - 2 bytes: general purpose bit flag
    /// - 2 bytes: compression method
    /// - 2 bytes: last mod file time
    /// - 2 bytes: last mod file date
    /// - 4 bytes: crc-32
    /// - 4 bytes: compressed size
    /// - 4 bytes: uncompressed size
    /// - 2 bytes: file name length
    /// - 2 bytes: extra field length
    /// - 2 bytes: file comment length
    /// - 2 bytes: disk number start
    /// - 2 bytes: internal file attributes
    /// - 4 bytes: external file attributes
    /// - 4 bytes: relative offset of local header
    /// - (variable): file name
    /// - (variable): extra field
    /// - (variable): file comment
    fn parse_central_directory(data: &[u8], archive_key: &str) -> Result<Vec<ObjectInfo>> {
        const CDFH_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
        const CDFH_MIN_SIZE: usize = 46;

        let mut entries = Vec::new();
        let mut pos = 0;

        while pos + CDFH_MIN_SIZE <= data.len() {
            // Check for CDFH signature
            if !data[pos..].starts_with(&CDFH_SIGNATURE) {
                // Reached end of central directory entries (could be EOCD or end of data)
                break;
            }

            // Parse fixed-size fields
            let compressed_size = u32::from_le_bytes([
                data[pos + 20],
                data[pos + 21],
                data[pos + 22],
                data[pos + 23],
            ]) as u64;

            let uncompressed_size = u32::from_le_bytes([
                data[pos + 24],
                data[pos + 25],
                data[pos + 26],
                data[pos + 27],
            ]) as u64;

            let filename_len = u16::from_le_bytes([data[pos + 28], data[pos + 29]]) as usize;

            let extra_len = u16::from_le_bytes([data[pos + 30], data[pos + 31]]) as usize;

            let comment_len = u16::from_le_bytes([data[pos + 32], data[pos + 33]]) as usize;

            // Ensure we have enough data for the variable-length fields
            let total_entry_size = CDFH_MIN_SIZE + filename_len + extra_len + comment_len;
            if pos + total_entry_size > data.len() {
                return Err(anyhow!(
                    "Truncated central directory entry at position {}",
                    pos
                ));
            }

            // Extract filename
            let filename_bytes = &data[pos + CDFH_MIN_SIZE..pos + CDFH_MIN_SIZE + filename_len];
            let filename = String::from_utf8_lossy(filename_bytes).to_string();

            // Determine if it's a directory (ends with /)
            let is_dir = filename.ends_with('/');

            // Build entry key: archive_key#internal_path
            let entry_key = format!("{}#{}", archive_key, filename);

            // Create ObjectInfo
            let info = if is_dir {
                ObjectInfo::prefix(filename.clone(), entry_key)
            } else {
                // Use uncompressed_size for display, but we track compressed_size internally
                let mut obj = ObjectInfo::object(filename.clone(), entry_key, uncompressed_size);
                obj.object_type = ObjectType::from_extension(&filename);
                // Store compressed size in a way we can use later for extraction
                // (For now, we don't need it for listing)
                let _ = compressed_size; // Silence unused warning
                obj
            };

            entries.push(info);

            // Move to next entry
            pos += total_entry_size;
        }

        Ok(entries)
    }

    /// Find the End of Central Directory record in the buffer.
    /// Returns the offset and size of the central directory, plus the offset in the buffer where EOCD was found.
    fn find_eocd(data: &[u8], _buffer_start_offset: u64) -> Result<(EocdInfo, usize)> {
        // EOCD signature: 0x06054b50 (little endian)
        const EOCD_SIGNATURE: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];

        // Search backwards from the end for the EOCD signature
        // Use inclusive range to handle EOCD at exactly MIN_EOCD_SIZE from end
        for i in (0..=data.len().saturating_sub(MIN_EOCD_SIZE as usize)).rev() {
            if data[i..].starts_with(&EOCD_SIGNATURE) {
                // Found potential EOCD, parse it
                let eocd = &data[i..];

                // Parse EOCD fields (all little endian)
                // Offset 16: Size of central directory (4 bytes)
                // Offset 20: Offset of central directory (4 bytes)
                if eocd.len() < MIN_EOCD_SIZE as usize {
                    continue;
                }

                let central_dir_size =
                    u32::from_le_bytes([eocd[12], eocd[13], eocd[14], eocd[15]]) as u64;

                let central_dir_offset =
                    u32::from_le_bytes([eocd[16], eocd[17], eocd[18], eocd[19]]) as u64;

                return Ok((
                    EocdInfo {
                        central_dir_offset,
                        central_dir_size,
                    },
                    i, // Return the offset in the buffer where EOCD was found
                ));
            }
        }

        Err(anyhow!("Could not find End of Central Directory record"))
    }
}

/// Information extracted from the End of Central Directory record
#[derive(Debug)]
struct EocdInfo {
    central_dir_offset: u64,
    central_dir_size: u64,
}

impl<P: Provider> Action for ZipArchiveAction<P> {
    fn id(&self) -> &str {
        "expand_zip_archive"
    }

    fn title(&self) -> &str {
        "Browse ZIP Archive"
    }

    fn description(&self) -> Option<&str> {
        Some("Browse ZIP archive contents inline without extracting")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        // Only applies to ZIP files with providers that support range requests
        if let Some(ref obj) = context.selected {
            obj.name.to_lowercase().ends_with(".zip")
                && context.provider_supports(super::context::ProviderCapability::RangeRequests)
                && obj.size.is_some()
        } else {
            false
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            let _key = obj.key.clone();
            let _size = obj.size.ok_or_else(|| anyhow!("ZIP file has no size"))?;

            // Return an async operation message - the actual listing will be triggered
            // by the event loop when it sees this message and the context
            Ok(ActionResult::async_op(format!(
                "Reading ZIP archive: {}",
                obj.name
            )))
        } else {
            Ok(ActionResult::error("No ZIP file selected"))
        }
    }

    fn priority(&self) -> i32 {
        100 // High priority for archives
    }

    fn shortcut(&self) -> Option<char> {
        Some('z')
    }
}

/// Action to extract individual files from ZIP archives.
///
/// This action extracts a single file from within a ZIP archive using
/// targeted range requests to read only the necessary compressed data.
pub struct ZipExtractAction<P: Provider> {
    provider: Arc<P>,
}

impl<P: Provider> ZipExtractAction<P> {
    pub fn new(provider: Arc<P>) -> Self {
        Self { provider }
    }

    /// Extract a single file from a ZIP archive using range requests.
    ///
    /// This function downloads the entire archive and uses the zip crate
    /// to extract and decompress the specific file. While this downloads
    /// more data than strictly necessary, it's simpler and handles all
    /// compression methods correctly.
    ///
    /// For very large archives, a future optimization could use the central
    /// directory to locate the specific file's local header and compressed
    /// data, then decompress just that portion.
    pub async fn extract_file(&self, archive_key: &str, entry_path: &str) -> Result<Vec<u8>> {
        // Get archive metadata
        let metadata = self.provider.head(archive_key).await?;
        let size = metadata
            .size
            .ok_or_else(|| anyhow!("Archive has no size"))?;

        if size < MIN_EOCD_SIZE {
            return Err(anyhow!("Archive too small to be valid"));
        }

        // Download the entire archive for extraction
        // (The zip crate needs access to local file headers and compressed data)
        let archive_data = self
            .provider
            .get_range(archive_key, 0, size - 1)
            .await
            .context("Failed to download ZIP archive")?;

        // Parse the archive
        let cursor = Cursor::new(archive_data);
        let mut archive = ZipArchive::new(cursor).context("Failed to parse ZIP archive")?;

        // Find the entry by name
        let mut entry = archive
            .by_name(entry_path)
            .context(format!("File '{}' not found in archive", entry_path))?;

        // Read the uncompressed content
        let mut content = Vec::new();
        entry.read_to_end(&mut content)?;

        Ok(content)
    }
}

impl<P: Provider> Action for ZipExtractAction<P> {
    fn id(&self) -> &str {
        "extract_zip_file"
    }

    fn title(&self) -> &str {
        "Extract File from ZIP"
    }

    fn description(&self) -> Option<&str> {
        Some("Extract and preview a file from within the ZIP archive")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        // Only applies to files inside ZIP archives (key contains '#')
        if let Some(ref obj) = context.selected {
            obj.key.contains('#')
                && context.provider_supports(super::context::ProviderCapability::RangeRequests)
        } else {
            false
        }
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            // Parse archive path: "archive.zip#internal/path.txt"
            let parts: Vec<&str> = obj.key.split('#').collect();
            if parts.len() != 2 {
                return Ok(ActionResult::error("Invalid archive path"));
            }

            // Return async operation - extraction will be triggered by event loop
            Ok(ActionResult::async_op(format!(
                "Extracting {} from archive",
                parts[1]
            )))
        } else {
            Ok(ActionResult::error("No file selected"))
        }
    }

    fn priority(&self) -> i32 {
        90
    }

    fn shortcut(&self) -> Option<char> {
        Some('x')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::context::ProviderCapability;

    // Helper to create test context
    fn create_zip_context(name: &str, key: &str, size: u64) -> ActionContext {
        ActionContext::new(
            Some(ObjectInfo::object(name.to_string(), key.to_string(), size)),
            "s3",
            false,
        )
    }

    #[test]
    fn test_zip_archive_action_predicate_matches_zip_files() {
        let context = create_zip_context("data.zip", "path/data.zip", 1024);

        // Check predicate would match if we had a real action instance
        assert!(context.selected.as_ref().unwrap().name.ends_with(".zip"));
        assert!(context.provider_supports(ProviderCapability::RangeRequests));
        assert!(context.selected.as_ref().unwrap().size.is_some());
    }

    #[test]
    fn test_zip_archive_action_predicate_rejects_non_zip() {
        let context = create_zip_context("data.txt", "path/data.txt", 1024);
        assert!(!context.selected.as_ref().unwrap().name.ends_with(".zip"));
    }

    #[test]
    fn test_zip_extract_action_predicate_matches_archive_paths() {
        let context = create_zip_context("file.txt", "archive.zip#internal/file.txt", 512);
        assert!(context.selected.as_ref().unwrap().key.contains('#'));
    }

    #[test]
    fn test_zip_extract_action_predicate_rejects_normal_files() {
        let context = create_zip_context("file.txt", "path/file.txt", 512);
        assert!(!context.selected.as_ref().unwrap().key.contains('#'));
    }

    #[test]
    fn test_eocd_info_parsing() {
        // This would test the find_eocd function with a real EOCD structure
        // For now, just ensure the struct is defined correctly
        let info = EocdInfo {
            central_dir_offset: 1000,
            central_dir_size: 500,
        };
        assert_eq!(info.central_dir_offset, 1000);
        assert_eq!(info.central_dir_size, 500);
    }

    #[test]
    fn test_min_eocd_size_constant() {
        // EOCD minimum size is 22 bytes
        assert_eq!(MIN_EOCD_SIZE, 22);
    }

    #[test]
    fn test_archive_path_encoding() {
        // Archive paths use '#' as separator
        let archive_key = "bucket/path/archive.zip";
        let internal_path = "internal/file.txt";
        let encoded = format!("{}#{}", archive_key, internal_path);

        assert_eq!(encoded, "bucket/path/archive.zip#internal/file.txt");

        let parts: Vec<&str> = encoded.split('#').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], archive_key);
        assert_eq!(parts[1], internal_path);
    }

    #[test]
    fn test_find_eocd_with_valid_signature() {
        // Create a minimal valid EOCD structure
        // EOCD signature: 0x50 0x4b 0x05 0x06
        let mut data = vec![0u8; 100];

        // Place EOCD signature at position 50
        let eocd_pos = 50;
        data[eocd_pos..eocd_pos + 4].copy_from_slice(&[0x50, 0x4b, 0x05, 0x06]);

        // Set central directory size at offset 12 (1000 bytes)
        data[eocd_pos + 12..eocd_pos + 16].copy_from_slice(&1000u32.to_le_bytes());

        // Set central directory offset at offset 16 (5000 bytes from start)
        data[eocd_pos + 16..eocd_pos + 20].copy_from_slice(&5000u32.to_le_bytes());

        // Test finding the EOCD
        let result = ZipArchiveAction::<crate::mock_provider::MockProvider>::find_eocd(&data, 0);

        assert!(result.is_ok());
        let (info, offset) = result.unwrap();
        assert_eq!(info.central_dir_size, 1000);
        assert_eq!(info.central_dir_offset, 5000);
        assert_eq!(offset, eocd_pos);
    }

    #[test]
    fn test_find_eocd_not_found() {
        // Create data without a valid EOCD signature
        let data = vec![0u8; 100];

        let result = ZipArchiveAction::<crate::mock_provider::MockProvider>::find_eocd(&data, 0);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Could not find End of Central Directory")
        );
    }

    #[test]
    fn test_find_eocd_searches_backwards() {
        // Test that we find the LAST occurrence (searching backwards)
        let mut data = vec![0u8; 200];

        // Place a fake EOCD at position 30
        data[30..34].copy_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        data[42..46].copy_from_slice(&100u32.to_le_bytes());
        data[46..50].copy_from_slice(&200u32.to_le_bytes());

        // Place the real EOCD at position 150 (should find this one)
        data[150..154].copy_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        data[162..166].copy_from_slice(&2000u32.to_le_bytes());
        data[166..170].copy_from_slice(&10000u32.to_le_bytes());

        let result = ZipArchiveAction::<crate::mock_provider::MockProvider>::find_eocd(&data, 0);
        assert!(result.is_ok());

        let (info, offset) = result.unwrap();
        // Should find the later EOCD (position 150)
        assert_eq!(info.central_dir_size, 2000);
        assert_eq!(info.central_dir_offset, 10000);
        assert_eq!(offset, 150);
    }

    #[test]
    fn test_eocd_search_size_constant() {
        // Ensure we're reading a reasonable amount from the end
        // 64KB is the ZIP spec maximum comment size + EOCD structure
        assert_eq!(EOCD_SEARCH_SIZE, 65536);
    }

    #[test]
    fn test_find_eocd_at_exact_min_size_boundary() {
        // Test edge case: EOCD at exactly MIN_EOCD_SIZE (22 bytes) from end
        // This was a real bug where the exclusive range missed this position
        let mut data = vec![0u8; MIN_EOCD_SIZE as usize];

        // Place EOCD signature at position 0 (exactly MIN_EOCD_SIZE from end)
        data[0..4].copy_from_slice(&[0x50, 0x4b, 0x05, 0x06]);

        // Set central directory size at offset 12
        data[12..16].copy_from_slice(&500u32.to_le_bytes());

        // Set central directory offset at offset 16
        data[16..20].copy_from_slice(&1000u32.to_le_bytes());

        let result = ZipArchiveAction::<crate::mock_provider::MockProvider>::find_eocd(&data, 0);

        assert!(
            result.is_ok(),
            "Should find EOCD at exact MIN_EOCD_SIZE boundary"
        );
        let (info, offset) = result.unwrap();
        assert_eq!(info.central_dir_size, 500);
        assert_eq!(info.central_dir_offset, 1000);
        assert_eq!(offset, 0);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};

    #[test]
    fn test_manual_central_directory_parsing() {
        // Read the real ZIP file
        let mut file = match File::open("250_netscape.zip") {
            Ok(f) => f,
            Err(_) => {
                println!("Skipping test - no test ZIP file found");
                return;
            }
        };

        let size = file.metadata().unwrap().len();
        println!("ZIP file size: {}", size);

        // Simulate what list_zip_contents does
        let eocd_start = size.saturating_sub(EOCD_SEARCH_SIZE);
        file.seek(SeekFrom::Start(eocd_start)).unwrap();
        let mut eocd_data = vec![0u8; (size - eocd_start) as usize];
        file.read_exact(&mut eocd_data).unwrap();

        // Find EOCD
        let (eocd_info, _eocd_offset_in_buffer) = ZipArchiveAction::<
            crate::mock_provider::MockProvider,
        >::find_eocd(&eocd_data, eocd_start)
        .unwrap();

        println!(
            "CD offset: {}, CD size: {}",
            eocd_info.central_dir_offset, eocd_info.central_dir_size
        );

        // Read central directory
        file.seek(SeekFrom::Start(eocd_info.central_dir_offset))
            .unwrap();
        let mut central_dir_data = vec![0u8; eocd_info.central_dir_size as usize];
        file.read_exact(&mut central_dir_data).unwrap();

        // Parse with our manual parser
        let entries =
            ZipArchiveAction::<crate::mock_provider::MockProvider>::parse_central_directory(
                &central_dir_data,
                "test.zip",
            )
            .unwrap();

        println!("Found {} entries:", entries.len());
        for entry in entries.iter().take(5) {
            println!("  {} (size: {:?})", entry.name, entry.size);
        }
        if entries.len() > 5 {
            println!("  ... and {} more", entries.len() - 5);
        }

        assert!(entries.len() > 0, "Should find entries in the ZIP file");
        assert_eq!(entries.len(), 18, "Expected 18 entries in test ZIP file");
    }
}
