//! AWS S3 provider implementation.

use aws_sdk_s3::Client;
use crate::provider::{ContextInfo, ListResult, ObjectInfo, Provider};

/// S3 provider backed by the AWS SDK
#[derive(Clone)]
pub struct S3Provider {
    client: Client,
    bucket: String,
}

impl S3Provider {
    /// Create a new S3 provider with default credentials
    pub async fn new(bucket: impl Into<String>) -> anyhow::Result<Self> {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);

        Ok(Self {
            client,
            bucket: bucket.into(),
        })
    }

    /// Create an S3 provider with a custom client (for testing)
    pub fn with_client(client: Client, bucket: impl Into<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
        }
    }

    /// Switch to a different bucket (reuses the same client)
    pub fn with_bucket(&self, bucket: impl Into<String>) -> Self {
        Self {
            client: self.client.clone(),
            bucket: bucket.into(),
        }
    }
}

impl Provider for S3Provider {
    async fn list(
        &self,
        prefix: &str,
        continuation_token: Option<&str>,
        max_keys: usize,
    ) -> anyhow::Result<ListResult> {
        let mut request = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .delimiter("/")
            .max_keys(max_keys as i32);

        // Only set prefix if non-empty
        if !prefix.is_empty() {
            request = request.prefix(prefix);
        }

        // Handle pagination
        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }

        let response = request.send().await?;

        let mut objects = Vec::new();

        // Add common prefixes (directories) first
        if let Some(prefixes) = response.common_prefixes {
            for prefix_obj in prefixes {
                if let Some(prefix_str) = prefix_obj.prefix {
                    // Extract the last segment as the name
                    let name = prefix_str
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or(&prefix_str)
                        .to_string() + "/";

                    objects.push(ObjectInfo::prefix(name, prefix_str));
                }
            }
        }

        // Add objects
        if let Some(contents) = response.contents {
            for obj in contents {
                if let Some(key) = obj.key {
                    // Skip the prefix itself if it appears as an object
                    if key.ends_with('/') {
                        continue;
                    }

                    // Extract just the filename from the full key
                    let name = key
                        .rsplit('/')
                        .next()
                        .unwrap_or(&key)
                        .to_string();

                    let size = obj.size.unwrap_or(0) as u64;
                    let mut info = ObjectInfo::object(name, key.clone(), size);

                    // Add last modified timestamp if available
                    if let Some(last_modified) = obj.last_modified {
                        info.last_modified = Some(last_modified.to_string());
                    }

                    objects.push(info);
                }
            }
        }

        Ok(ListResult {
            objects,
            continuation_token: response.next_continuation_token,
            is_truncated: response.is_truncated.unwrap_or(false),
        })
    }

    async fn head(&self, key: &str) -> anyhow::Result<ObjectInfo> {
        let response = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;

        let name = key
            .rsplit('/')
            .next()
            .unwrap_or(key)
            .to_string();

        let size = response.content_length.unwrap_or(0) as u64;
        let mut info = ObjectInfo::object(name, key, size);

        if let Some(last_modified) = response.last_modified {
            info.last_modified = Some(last_modified.to_string());
        }

        Ok(info)
    }

    async fn get_range(&self, key: &str, start: u64, end: u64) -> anyhow::Result<Vec<u8>> {
        let range = format!("bytes={}-{}", start, end);

        let response = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .range(range)
            .send()
            .await?;

        let data = response
            .body
            .collect()
            .await?
            .into_bytes()
            .to_vec();

        Ok(data)
    }

    async fn list_contexts(&self) -> anyhow::Result<Vec<ContextInfo>> {
        let response = self.client.list_buckets().send().await?;

        let mut contexts = Vec::new();
        if let Some(buckets) = response.buckets {
            for bucket in buckets {
                if let Some(name) = bucket.name {
                    contexts.push(ContextInfo {
                        name,
                        description: None,
                    });
                }
            }
        }

        Ok(contexts)
    }

    fn name(&self) -> &str {
        "s3"
    }
}
