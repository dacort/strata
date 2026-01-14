//! File preview types and content processing.
//!
//! Handles safe file viewing with binary detection, line splitting,
//! and preview mode management.

use crate::provider::ObjectInfo;

/// Preview configuration constants
pub const PREVIEW_BYTES: u64 = 8192; // 8KB head/tail fetch
pub const PREVIEW_MAX_LINES: usize = 200; // Max lines to display
const BINARY_THRESHOLD: f64 = 0.10; // 10% non-printable = binary

/// Preview mode - viewing head or tail of file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Head,
    Tail,
}

/// Content variants for preview
#[derive(Debug, Clone)]
pub enum PreviewContent {
    /// Not yet loaded
    NotLoaded,
    /// Loading in progress
    Loading,
    /// Text content with lines
    Text {
        lines: Vec<String>,
        total_bytes: usize,
        truncated: bool,
    },
    /// Binary file - show metadata only
    Binary,
    /// Error loading
    Error(String),
}

/// Preview state for a file
#[derive(Debug, Clone)]
pub struct FilePreview {
    /// Object key being previewed
    pub key: String,
    /// Display name (filename portion)
    pub name: String,
    /// File size if known
    pub size: Option<u64>,
    /// Last modified timestamp
    pub last_modified: Option<String>,
    /// Current preview mode
    pub mode: PreviewMode,
    /// Preview content
    pub content: PreviewContent,
    /// Scroll offset in lines
    pub scroll_offset: usize,
}

impl FilePreview {
    /// Create a new preview from object info
    pub fn new(info: &ObjectInfo) -> Self {
        Self {
            key: info.key.clone(),
            name: info.name.clone(),
            size: info.size,
            last_modified: info.last_modified.clone(),
            mode: PreviewMode::Head,
            content: PreviewContent::Loading,
            scroll_offset: 0,
        }
    }

    /// Create preview from fetched bytes
    pub fn from_bytes(
        key: String,
        name: String,
        size: Option<u64>,
        last_modified: Option<String>,
        data: Vec<u8>,
        mode: PreviewMode,
    ) -> Self {
        let content = if is_binary_content(&data) {
            PreviewContent::Binary
        } else {
            let total_bytes = data.len();
            let (lines, truncated) = split_into_lines(&data, PREVIEW_MAX_LINES);
            PreviewContent::Text {
                lines,
                total_bytes,
                truncated,
            }
        };

        Self {
            key,
            name,
            size,
            last_modified,
            mode,
            content,
            scroll_offset: 0,
        }
    }

    /// Update content from fetched bytes, preserving metadata
    pub fn update_content(&mut self, data: Vec<u8>, mode: PreviewMode) {
        self.mode = mode;
        self.scroll_offset = 0;

        if is_binary_content(&data) {
            self.content = PreviewContent::Binary;
        } else {
            let total_bytes = data.len();
            let (lines, truncated) = split_into_lines(&data, PREVIEW_MAX_LINES);
            self.content = PreviewContent::Text {
                lines,
                total_bytes,
                truncated,
            };
        }
    }

    /// Set loading state
    pub fn set_loading(&mut self) {
        self.content = PreviewContent::Loading;
    }

    /// Set error state
    pub fn set_error(&mut self, err: String) {
        self.content = PreviewContent::Error(err);
    }

    /// Get number of content lines
    pub fn line_count(&self) -> usize {
        match &self.content {
            PreviewContent::Text { lines, .. } => lines.len(),
            _ => 0,
        }
    }

    /// Scroll up by one line
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// Scroll down by one line
    pub fn scroll_down(&mut self, visible_height: usize) {
        let max_offset = self.line_count().saturating_sub(visible_height);
        if self.scroll_offset < max_offset {
            self.scroll_offset += 1;
        }
    }

    /// Scroll up by a page
    pub fn page_up(&mut self, visible_height: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(visible_height);
    }

    /// Scroll down by a page
    pub fn page_down(&mut self, visible_height: usize) {
        let max_offset = self.line_count().saturating_sub(visible_height);
        self.scroll_offset = (self.scroll_offset + visible_height).min(max_offset);
    }
}

/// Detect if content is binary (high ratio of non-printable chars)
pub fn is_binary_content(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    // Check first chunk for binary indicators
    let check_len = data.len().min(1024);
    let check_data = &data[..check_len];

    // Count non-printable characters (excluding common whitespace)
    let non_printable = check_data
        .iter()
        .filter(|&&b| {
            // Allow: printable ASCII, tab, newline, carriage return
            !(b == b'\t' || b == b'\n' || b == b'\r' || (0x20..=0x7E).contains(&b))
                // Also allow UTF-8 continuation bytes (for non-ASCII text)
                && !(0x80..=0xBF).contains(&b)
                // Allow UTF-8 leading bytes
                && !(0xC0..=0xF7).contains(&b)
        })
        .count();

    let ratio = non_printable as f64 / check_len as f64;
    ratio > BINARY_THRESHOLD
}

/// Split bytes into lines, respecting max lines limit
pub fn split_into_lines(data: &[u8], max_lines: usize) -> (Vec<String>, bool) {
    // Try to decode as UTF-8, falling back to lossy conversion
    let text = String::from_utf8_lossy(data);

    let mut lines: Vec<String> = Vec::new();
    let mut truncated = false;

    for line in text.lines() {
        if lines.len() >= max_lines {
            truncated = true;
            break;
        }
        // Truncate very long lines to prevent UI issues
        let line = if line.len() > 500 {
            format!("{}...", &line[..500])
        } else {
            line.to_string()
        };
        lines.push(line);
    }

    (lines, truncated)
}

/// Format file size for display
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_text_content() {
        let text = b"Hello, world!\nThis is a test file.\n";
        assert!(!is_binary_content(text));
    }

    #[test]
    fn test_is_binary_binary_content() {
        let binary: Vec<u8> = (0..100).collect();
        assert!(is_binary_content(&binary));
    }

    #[test]
    fn test_is_binary_empty() {
        assert!(!is_binary_content(&[]));
    }

    #[test]
    fn test_split_into_lines() {
        let text = b"line1\nline2\nline3\n";
        let (lines, truncated) = split_into_lines(text, 10);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
        assert!(!truncated);
    }

    #[test]
    fn test_split_into_lines_truncated() {
        let text = b"line1\nline2\nline3\nline4\nline5\n";
        let (lines, truncated) = split_into_lines(text, 3);
        assert_eq!(lines.len(), 3);
        assert!(truncated);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }
}
