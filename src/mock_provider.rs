//! Mock provider for testing and demos.

use crate::provider::{ContextInfo, ListResult, ObjectInfo, Provider};

/// Mock provider that returns fake data for testing
#[derive(Clone)]
pub struct MockProvider {
    pub name: String,
}

impl MockProvider {
    pub fn new() -> Self {
        Self {
            name: "mock".to_string(),
        }
    }

    /// Create a mock ZIP file in memory and return the requested range
    fn get_mock_zip_data(&self, start: u64, end: u64) -> anyhow::Result<Vec<u8>> {
        use std::io::{Cursor, Write};
        use zip::write::{FileOptions, SimpleFileOptions, ZipWriter};

        // Create a ZIP file in memory
        let mut zip_buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut zip_buffer);
            let options: FileOptions<()> = SimpleFileOptions::default().into();

            // Add a few test files
            zip.start_file("README.txt", options)?;
            zip.write_all(b"This is a test README file inside the ZIP archive.\n")?;

            zip.start_file("data/sample.json", options)?;
            zip.write_all(b"{\"name\": \"test\", \"value\": 42}\n")?;

            zip.start_file("data/config.yaml", options)?;
            zip.write_all(b"version: 1.0\nname: test-config\n")?;

            zip.start_file("scripts/setup.sh", options)?;
            zip.write_all(b"#!/bin/bash\necho 'Setup complete'\n")?;

            zip.finish()?;
        }

        let full_data = zip_buffer.into_inner();

        // Return the requested range
        let start_idx = start as usize;
        let end_idx = (end as usize + 1).min(full_data.len());

        if start_idx >= full_data.len() {
            return Ok(Vec::new());
        }

        Ok(full_data[start_idx..end_idx].to_vec())
    }
}

impl Provider for MockProvider {
    async fn list(
        &self,
        prefix: &str,
        continuation_token: Option<&str>,
        _max_keys: usize,
    ) -> anyhow::Result<ListResult> {
        // Simulate some network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Simulate pagination for the "data/raw/" prefix
        if prefix == "data/raw/" {
            let page = continuation_token
                .and_then(|t| t.parse::<usize>().ok())
                .unwrap_or(0);

            return match page {
                0 => {
                    // First page - 3 items
                    Ok(ListResult {
                        objects: vec![
                            ObjectInfo::object(
                                "2024-01-01.json",
                                "data/raw/2024-01-01.json",
                                1024 * 100,
                            ),
                            ObjectInfo::object(
                                "2024-01-02.json",
                                "data/raw/2024-01-02.json",
                                1024 * 150,
                            ),
                            ObjectInfo::object(
                                "2024-01-03.json",
                                "data/raw/2024-01-03.json",
                                1024 * 120,
                            ),
                        ],
                        continuation_token: Some("1".to_string()),
                        is_truncated: true,
                    })
                }
                1 => {
                    // Second page - 3 more items
                    Ok(ListResult {
                        objects: vec![
                            ObjectInfo::object(
                                "2024-01-04.json",
                                "data/raw/2024-01-04.json",
                                1024 * 130,
                            ),
                            ObjectInfo::object(
                                "2024-01-05.json",
                                "data/raw/2024-01-05.json",
                                1024 * 140,
                            ),
                            ObjectInfo::object(
                                "2024-01-06.json",
                                "data/raw/2024-01-06.json",
                                1024 * 110,
                            ),
                        ],
                        continuation_token: Some("2".to_string()),
                        is_truncated: true,
                    })
                }
                2 => {
                    // Third page - final 2 items
                    Ok(ListResult {
                        objects: vec![
                            ObjectInfo::object(
                                "2024-01-07.json",
                                "data/raw/2024-01-07.json",
                                1024 * 125,
                            ),
                            ObjectInfo::object(
                                "2024-01-08.json",
                                "data/raw/2024-01-08.json",
                                1024 * 135,
                            ),
                        ],
                        continuation_token: None,
                        is_truncated: false,
                    })
                }
                _ => {
                    // No more items
                    Ok(ListResult {
                        objects: vec![],
                        continuation_token: None,
                        is_truncated: false,
                    })
                }
            };
        }

        let objects = if prefix.is_empty() {
            // Root level - show some directories
            vec![
                ObjectInfo::prefix("logs/", "logs/"),
                ObjectInfo::prefix("data/", "data/"),
                ObjectInfo::prefix("models/", "models/"),
                ObjectInfo::object("README.md", "README.md", 2048),
                ObjectInfo::object("config.yaml", "config.yaml", 512),
            ]
        } else if prefix == "logs/" {
            vec![
                ObjectInfo::object("app.log", "logs/app.log", 1024 * 1024 * 50),
                ObjectInfo::object("error.log", "logs/error.log", 1024 * 512),
                ObjectInfo::object("access.log.gz", "logs/access.log.gz", 1024 * 1024 * 200),
            ]
        } else if prefix == "data/" {
            vec![
                ObjectInfo::prefix("raw/", "data/raw/"),
                ObjectInfo::prefix("processed/", "data/processed/"),
                ObjectInfo::object("users.parquet", "data/users.parquet", 1024 * 1024 * 150),
                ObjectInfo::object(
                    "events.parquet",
                    "data/events.parquet",
                    1024 * 1024 * 1024 * 2,
                ),
                ObjectInfo::object("backup.tar.gz", "data/backup.tar.gz", 1024 * 1024 * 500),
                ObjectInfo::object("archive.zip", "data/archive.zip", 1024 * 1024 * 10),
            ]
        } else if prefix == "models/" {
            vec![
                ObjectInfo::prefix("v1/", "models/v1/"),
                ObjectInfo::prefix("v2/", "models/v2/"),
                ObjectInfo::object("model_config.json", "models/model_config.json", 2048),
            ]
        } else {
            vec![]
        };

        Ok(ListResult {
            objects,
            continuation_token: None,
            is_truncated: false,
        })
    }

    async fn head(&self, key: &str) -> anyhow::Result<ObjectInfo> {
        // Return appropriate sizes for known files
        let size = if key == "data/archive.zip" {
            1024 * 1024 * 10 // 10MB
        } else {
            1024 // 1KB default
        };
        Ok(ObjectInfo::object(key, key, size))
    }

    async fn get_range(&self, key: &str, start: u64, end: u64) -> anyhow::Result<Vec<u8>> {
        // For ZIP files, return a minimal valid ZIP structure
        if key == "data/archive.zip" {
            return self.get_mock_zip_data(start, end);
        }

        // Default mock content
        Ok(b"Mock file content\nLine 2\nLine 3\n".to_vec())
    }

    async fn list_contexts(&self) -> anyhow::Result<Vec<ContextInfo>> {
        // Return some fake bucket names for testing
        Ok(vec![
            ContextInfo {
                name: "demo-bucket".to_string(),
                description: Some("Demo bucket for testing".to_string()),
            },
            ContextInfo {
                name: "my-data-bucket".to_string(),
                description: Some("Sample data storage".to_string()),
            },
            ContextInfo {
                name: "production-logs".to_string(),
                description: None,
            },
            ContextInfo {
                name: "ml-training-artifacts".to_string(),
                description: Some("Machine learning models and datasets".to_string()),
            },
        ])
    }

    fn name(&self) -> &str {
        &self.name
    }
}
