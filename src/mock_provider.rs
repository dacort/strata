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
}

impl Provider for MockProvider {
    async fn list(
        &self,
        prefix: &str,
        _continuation_token: Option<&str>,
        _max_keys: usize,
    ) -> anyhow::Result<ListResult> {
        // Simulate some network delay
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

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
                ObjectInfo::object("events.parquet", "data/events.parquet", 1024 * 1024 * 1024 * 2),
                ObjectInfo::object("backup.tar.gz", "data/backup.tar.gz", 1024 * 1024 * 500),
            ]
        } else if prefix == "data/raw/" {
            vec![
                ObjectInfo::object("2024-01-01.json", "data/raw/2024-01-01.json", 1024 * 100),
                ObjectInfo::object("2024-01-02.json", "data/raw/2024-01-02.json", 1024 * 150),
                ObjectInfo::object("2024-01-03.json", "data/raw/2024-01-03.json", 1024 * 120),
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
        Ok(ObjectInfo::object(key, key, 1024))
    }

    async fn get_range(&self, _key: &str, _start: u64, _end: u64) -> anyhow::Result<Vec<u8>> {
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
