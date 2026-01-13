//! Provider abstraction for object storage backends.
//!
//! Providers implement capabilities; UI consumes capabilities.
//! This keeps the core UI provider-agnostic.

use std::fmt;

/// Represents an item in object storage (prefix, object, file, etc.)
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    /// Display name (e.g., "logs/", "data.parquet")
    pub name: String,
    /// Full key/path within the provider
    pub key: String,
    /// Object type for context-aware handling
    pub object_type: ObjectType,
    /// Size in bytes (None for prefixes/directories)
    pub size: Option<u64>,
    /// Last modified timestamp (None if unavailable)
    pub last_modified: Option<String>,
}

impl ObjectInfo {
    pub fn prefix(name: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            key: key.into(),
            object_type: ObjectType::Prefix,
            size: None,
            last_modified: None,
        }
    }

    pub fn object(name: impl Into<String>, key: impl Into<String>, size: u64) -> Self {
        let name = name.into();
        let object_type = ObjectType::from_extension(&name);
        Self {
            name,
            key: key.into(),
            object_type,
            size: Some(size),
            last_modified: None,
        }
    }
}

/// Object type for content-aware handling.
/// Determines default view and available actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectType {
    /// Directory/prefix - navigable
    Prefix,
    /// Plain text file - previewable
    Text,
    /// Archive (zip, tar, etc.) - browsable
    Archive,
    /// Columnar data (parquet, arrow) - inspectable
    Columnar,
    /// Binary/unknown - show metadata only
    Binary,
}

impl ObjectType {
    /// Infer type from file extension
    pub fn from_extension(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.ends_with('/') {
            ObjectType::Prefix
        } else if lower.ends_with(".txt")
            || lower.ends_with(".md")
            || lower.ends_with(".json")
            || lower.ends_with(".yaml")
            || lower.ends_with(".yml")
            || lower.ends_with(".toml")
            || lower.ends_with(".csv")
            || lower.ends_with(".log")
            || lower.ends_with(".py")
            || lower.ends_with(".rs")
            || lower.ends_with(".js")
            || lower.ends_with(".ts")
        {
            ObjectType::Text
        } else if lower.ends_with(".zip")
            || lower.ends_with(".tar")
            || lower.ends_with(".tar.gz")
            || lower.ends_with(".tgz")
            || lower.ends_with(".tar.bz2")
        {
            ObjectType::Archive
        } else if lower.ends_with(".parquet") || lower.ends_with(".arrow") {
            ObjectType::Columnar
        } else {
            ObjectType::Binary
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            ObjectType::Prefix => "📁",
            ObjectType::Text => "📄",
            ObjectType::Archive => "📦",
            ObjectType::Columnar => "📊",
            ObjectType::Binary => "📎",
        }
    }
}

impl fmt::Display for ObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectType::Prefix => write!(f, "prefix"),
            ObjectType::Text => write!(f, "text"),
            ObjectType::Archive => write!(f, "archive"),
            ObjectType::Columnar => write!(f, "columnar"),
            ObjectType::Binary => write!(f, "binary"),
        }
    }
}

/// Result of a paginated listing operation
#[derive(Debug)]
pub struct ListResult {
    pub objects: Vec<ObjectInfo>,
    pub continuation_token: Option<String>,
    pub is_truncated: bool,
}

/// Provider context - configured backend + auth + root namespace
#[derive(Debug, Clone)]
pub struct ProviderContext {
    pub provider_name: String,
    pub root: String, // bucket name, repo, org, etc.
    pub current_prefix: String,
}

impl ProviderContext {
    pub fn display_path(&self) -> String {
        if self.current_prefix.is_empty() {
            format!("{}://{}", self.provider_name, self.root)
        } else {
            format!("{}://{}/{}", self.provider_name, self.root, self.current_prefix)
        }
    }
}

/// Provider trait - implemented by S3, GCS, HuggingFace, etc.
///
/// Note: We use a concrete type approach rather than dyn trait because
/// async traits are not dyn-compatible without boxing futures.
pub trait Provider: Send + Sync + 'static {
    /// List objects at the given prefix with pagination
    fn list(
        &self,
        prefix: &str,
        continuation_token: Option<&str>,
        max_keys: usize,
    ) -> impl std::future::Future<Output = anyhow::Result<ListResult>> + Send;

    /// Get object metadata without downloading content
    fn head(&self, key: &str) -> impl std::future::Future<Output = anyhow::Result<ObjectInfo>> + Send;

    /// Download a range of bytes from an object
    fn get_range(&self, key: &str, start: u64, end: u64) -> impl std::future::Future<Output = anyhow::Result<Vec<u8>>> + Send;

    /// Provider name for display
    fn name(&self) -> &str;
}
