//! Action context - state information for action predicates and execution.

use crate::provider::ObjectInfo;

/// Context information passed to actions for predicate evaluation and execution.
///
/// This struct captures the current state of the application that actions
/// need to determine applicability and perform operations.
#[derive(Debug, Clone)]
pub struct ActionContext {
    /// Currently selected object (if any)
    pub selected: Option<ObjectInfo>,

    /// Provider name (e.g., "s3", "local", "huggingface")
    pub provider_name: String,

    /// Whether the current selection is expanded (for directories)
    pub is_expanded: bool,
}

impl ActionContext {
    /// Create a new action context
    pub fn new(
        selected: Option<ObjectInfo>,
        provider_name: impl Into<String>,
        is_expanded: bool,
    ) -> Self {
        Self {
            selected,
            provider_name: provider_name.into(),
            is_expanded,
        }
    }

    /// Check if the selected object is of a specific type
    pub fn is_object_type(&self, object_type: &crate::provider::ObjectType) -> bool {
        self.selected
            .as_ref()
            .map(|obj| &obj.object_type == object_type)
            .unwrap_or(false)
    }

    /// Check if any object is selected
    pub fn has_selection(&self) -> bool {
        self.selected.is_some()
    }

    /// Get the object type of the selected item
    pub fn object_type(&self) -> Option<&crate::provider::ObjectType> {
        self.selected.as_ref().map(|obj| &obj.object_type)
    }

    /// Get the selected object's key
    pub fn selected_key(&self) -> Option<&str> {
        self.selected.as_ref().map(|obj| obj.key.as_str())
    }

    /// Get the selected object's size
    pub fn selected_size(&self) -> Option<u64> {
        self.selected.as_ref().and_then(|obj| obj.size)
    }

    /// Check if the provider supports a specific capability
    pub fn provider_supports(&self, capability: ProviderCapability) -> bool {
        match capability {
            ProviderCapability::RangeRequests => {
                // S3 and most cloud providers support range requests
                matches!(self.provider_name.as_str(), "s3" | "mock")
            }
            ProviderCapability::Metadata => {
                // Most providers support metadata
                true
            }
            ProviderCapability::DirectDownload => {
                // S3 and cloud providers support direct download
                matches!(self.provider_name.as_str(), "s3" | "mock")
            }
        }
    }

    /// Check if the selected item is a directory/prefix
    pub fn is_directory(&self) -> bool {
        use crate::provider::ObjectType;
        self.is_object_type(&ObjectType::Prefix)
    }

    /// Check if the selected item is a file (not a directory)
    pub fn is_file(&self) -> bool {
        self.has_selection() && !self.is_directory()
    }
}

/// Provider capabilities that actions can check for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCapability {
    /// Support for HTTP range requests (partial downloads)
    RangeRequests,
    /// Support for metadata queries (HEAD requests)
    Metadata,
    /// Support for direct file downloads
    DirectDownload,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ObjectInfo, ObjectType};

    fn create_test_context(obj_type: ObjectType, provider: &str) -> ActionContext {
        ActionContext::new(
            Some(ObjectInfo {
                name: "test".to_string(),
                key: "test".to_string(),
                object_type: obj_type,
                size: Some(1024),
                last_modified: None,
            }),
            provider,
            false,
        )
    }

    #[test]
    fn test_is_object_type() {
        let context = create_test_context(ObjectType::Archive, "s3");
        assert!(context.is_object_type(&ObjectType::Archive));
        assert!(!context.is_object_type(&ObjectType::Text));
    }

    #[test]
    fn test_has_selection() {
        let context = create_test_context(ObjectType::Text, "s3");
        assert!(context.has_selection());

        let no_selection = ActionContext::new(None, "s3", false);
        assert!(!no_selection.has_selection());
    }

    #[test]
    fn test_object_type() {
        let context = create_test_context(ObjectType::Columnar, "s3");
        assert_eq!(context.object_type(), Some(&ObjectType::Columnar));
    }

    #[test]
    fn test_selected_key() {
        let context = create_test_context(ObjectType::Text, "s3");
        assert_eq!(context.selected_key(), Some("test"));
    }

    #[test]
    fn test_selected_size() {
        let context = create_test_context(ObjectType::Binary, "s3");
        assert_eq!(context.selected_size(), Some(1024));
    }

    #[test]
    fn test_provider_supports() {
        let context = create_test_context(ObjectType::Text, "s3");
        assert!(context.provider_supports(ProviderCapability::RangeRequests));
        assert!(context.provider_supports(ProviderCapability::Metadata));
        assert!(context.provider_supports(ProviderCapability::DirectDownload));
    }

    #[test]
    fn test_is_directory() {
        let dir_context = create_test_context(ObjectType::Prefix, "s3");
        assert!(dir_context.is_directory());
        assert!(!dir_context.is_file());

        let file_context = create_test_context(ObjectType::Text, "s3");
        assert!(!file_context.is_directory());
        assert!(file_context.is_file());
    }

    #[test]
    fn test_is_expanded() {
        let expanded = ActionContext::new(
            Some(ObjectInfo {
                name: "dir/".to_string(),
                key: "dir/".to_string(),
                object_type: ObjectType::Prefix,
                size: None,
                last_modified: None,
            }),
            "s3",
            true,
        );
        assert!(expanded.is_expanded);

        let collapsed = ActionContext::new(
            Some(ObjectInfo {
                name: "dir/".to_string(),
                key: "dir/".to_string(),
                object_type: ObjectType::Prefix,
                size: None,
                last_modified: None,
            }),
            "s3",
            false,
        );
        assert!(!collapsed.is_expanded);
    }
}
