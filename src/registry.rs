//! Provider registry - maintains list of available providers.

/// Information about a provider
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub enabled: bool,
    pub status: Option<String>,
}

/// Get all available providers
pub fn get_available_providers() -> Vec<ProviderInfo> {
    vec![
        ProviderInfo {
            id: "s3",
            name: "AWS S3",
            enabled: true,
            status: None,
        },
        ProviderInfo {
            id: "gcs",
            name: "Google Cloud Storage",
            enabled: false,
            status: Some("(coming soon)".to_string()),
        },
        ProviderInfo {
            id: "hf-datasets",
            name: "HuggingFace Datasets",
            enabled: false,
            status: Some("(coming soon)".to_string()),
        },
    ]
}

/// Parse a URI into provider and resource information
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedUri {
    S3 { bucket: String },
    HuggingFace { resource_type: String, path: String },
    // Future: Gcs { project: String, bucket: String }
}

/// Parse a URI string like "s3://bucket-name" or "hf://datasets/org/repo"
pub fn parse_uri(uri: &str) -> Option<ParsedUri> {
    if let Some(rest) = uri.strip_prefix("s3://") {
        let bucket = rest.trim_end_matches('/').to_string();
        if !bucket.is_empty() {
            return Some(ParsedUri::S3 { bucket });
        }
    }

    if let Some(rest) = uri.strip_prefix("hf://") {
        let parts: Vec<&str> = rest.trim_end_matches('/').split('/').collect();
        if parts.len() >= 2 {
            return Some(ParsedUri::HuggingFace {
                resource_type: parts[0].to_string(),
                path: parts[1..].join("/"),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_s3_uri() {
        assert_eq!(
            parse_uri("s3://my-bucket"),
            Some(ParsedUri::S3 {
                bucket: "my-bucket".to_string()
            })
        );
        assert_eq!(
            parse_uri("s3://my-bucket/"),
            Some(ParsedUri::S3 {
                bucket: "my-bucket".to_string()
            })
        );
    }

    #[test]
    fn test_parse_hf_uri() {
        assert_eq!(
            parse_uri("hf://datasets/org/repo"),
            Some(ParsedUri::HuggingFace {
                resource_type: "datasets".to_string(),
                path: "org/repo".to_string()
            })
        );
    }

    #[test]
    fn test_parse_invalid_uri() {
        assert_eq!(parse_uri("invalid"), None);
        assert_eq!(parse_uri("s3://"), None);
        assert_eq!(parse_uri("hf://datasets"), None);
    }
}
