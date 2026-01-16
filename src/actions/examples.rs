//! Example actions demonstrating the action framework.
//!
//! These examples show how to implement common actions:
//! - Preview text files
//! - Expand archives inline
//! - Download files
//! - Navigate directories

use anyhow::Result;
use std::sync::Arc;

use super::{Action, ActionContext, ActionRegistry, ActionResult};
use crate::provider::ObjectType;

/// Example: Preview text files
pub struct PreviewTextAction;

impl Action for PreviewTextAction {
    fn id(&self) -> &str {
        "preview_text"
    }

    fn title(&self) -> &str {
        "Preview File"
    }

    fn description(&self) -> Option<&str> {
        Some("Show a preview of the file contents")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        // Only applies to text files
        context.is_object_type(&ObjectType::Text)
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            // In a real implementation, this would fetch the content
            // For now, return a message
            Ok(ActionResult::message(format!(
                "Preview: {} ({})",
                obj.name,
                obj.size
                    .map_or("unknown size".to_string(), |s| format!("{} bytes", s))
            )))
        } else {
            Ok(ActionResult::error("No file selected"))
        }
    }

    fn priority(&self) -> i32 {
        100 // High priority - show first
    }

    fn shortcut(&self) -> Option<char> {
        Some('p')
    }
}

/// Example: Expand archive files inline
pub struct ExpandArchiveAction;

impl Action for ExpandArchiveAction {
    fn id(&self) -> &str {
        "expand_archive"
    }

    fn title(&self) -> &str {
        "Expand Archive"
    }

    fn description(&self) -> Option<&str> {
        Some("Browse archive contents inline")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        // Only applies to archive files
        context.is_object_type(&ObjectType::Archive)
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            // In a real implementation, this would:
            // 1. Download the archive
            // 2. Extract the contents list
            // 3. Return an Expand result with children
            Ok(ActionResult::message(format!(
                "Would expand archive: {}",
                obj.name
            )))
        } else {
            Ok(ActionResult::error("No archive selected"))
        }
    }

    fn priority(&self) -> i32 {
        90
    }

    fn shortcut(&self) -> Option<char> {
        Some('e')
    }
}

/// Example: Download file to local filesystem
pub struct DownloadAction;

impl Action for DownloadAction {
    fn id(&self) -> &str {
        "download"
    }

    fn title(&self) -> &str {
        "Download to Local"
    }

    fn description(&self) -> Option<&str> {
        Some("Download file to current directory")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        // Applies to any file (not directories) if provider supports downloads
        context.is_file()
            && context.provider_supports(super::context::ProviderCapability::DirectDownload)
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            Ok(ActionResult::async_op(format!(
                "Downloading {} ...",
                obj.name
            )))
        } else {
            Ok(ActionResult::error("No file selected"))
        }
    }

    fn priority(&self) -> i32 {
        50
    }

    fn shortcut(&self) -> Option<char> {
        Some('d')
    }
}

/// Example: Inspect columnar data files (Parquet, Arrow)
pub struct InspectColumnarAction;

impl Action for InspectColumnarAction {
    fn id(&self) -> &str {
        "inspect_columnar"
    }

    fn title(&self) -> &str {
        "Inspect Schema"
    }

    fn description(&self) -> Option<&str> {
        Some("Show schema and statistics for columnar data")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        context.is_object_type(&ObjectType::Columnar)
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            Ok(ActionResult::message(format!(
                "Schema inspection for: {}",
                obj.name
            )))
        } else {
            Ok(ActionResult::error("No columnar file selected"))
        }
    }

    fn priority(&self) -> i32 {
        80
    }

    fn shortcut(&self) -> Option<char> {
        Some('i')
    }
}

/// Example: Navigate into a directory
pub struct NavigateAction;

impl Action for NavigateAction {
    fn id(&self) -> &str {
        "navigate"
    }

    fn title(&self) -> &str {
        "Open Directory"
    }

    fn description(&self) -> Option<&str> {
        Some("Navigate into the selected directory")
    }

    fn predicate(&self, context: &ActionContext) -> bool {
        context.is_directory() && !context.is_expanded
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        if let Some(ref obj) = context.selected {
            Ok(ActionResult::navigate(obj.key.clone()))
        } else {
            Ok(ActionResult::error("No directory selected"))
        }
    }

    fn priority(&self) -> i32 {
        100
    }

    fn shortcut(&self) -> Option<char> {
        Some('o')
    }
}

/// Create a registry with all example actions registered
pub fn create_example_registry() -> ActionRegistry {
    let mut registry = ActionRegistry::new();

    registry.register(Arc::new(PreviewTextAction));
    registry.register(Arc::new(ExpandArchiveAction));
    registry.register(Arc::new(DownloadAction));
    registry.register(Arc::new(InspectColumnarAction));
    registry.register(Arc::new(NavigateAction));

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ObjectInfo;

    fn create_test_context(obj_type: ObjectType, expanded: bool) -> ActionContext {
        ActionContext::new(
            Some(ObjectInfo {
                name: "test".to_string(),
                key: "test".to_string(),
                object_type: obj_type,
                size: Some(1024),
                last_modified: None,
            }),
            "s3",
            expanded,
        )
    }

    #[test]
    fn test_preview_text_action() {
        let action = PreviewTextAction;
        let context = create_test_context(ObjectType::Text, false);

        assert_eq!(action.id(), "preview_text");
        assert_eq!(action.shortcut(), Some('p'));
        assert!(action.predicate(&context));

        let result = action.execute(&context).unwrap();
        assert!(result.is_visible());
    }

    #[test]
    fn test_expand_archive_action() {
        let action = ExpandArchiveAction;

        // Should apply to archives
        let archive_context = create_test_context(ObjectType::Archive, false);
        assert!(action.predicate(&archive_context));

        // Should not apply to text files
        let text_context = create_test_context(ObjectType::Text, false);
        assert!(!action.predicate(&text_context));
    }

    #[test]
    fn test_download_action() {
        let action = DownloadAction;

        // Should apply to files
        let file_context = create_test_context(ObjectType::Text, false);
        assert!(action.predicate(&file_context));

        // Should not apply to directories
        let dir_context = create_test_context(ObjectType::Prefix, false);
        assert!(!action.predicate(&dir_context));
    }

    #[test]
    fn test_inspect_columnar_action() {
        let action = InspectColumnarAction;

        // Should apply to columnar files
        let columnar_context = create_test_context(ObjectType::Columnar, false);
        assert!(action.predicate(&columnar_context));

        // Should not apply to other file types
        let text_context = create_test_context(ObjectType::Text, false);
        assert!(!action.predicate(&text_context));
    }

    #[test]
    fn test_navigate_action() {
        let action = NavigateAction;

        // Should apply to collapsed directories
        let collapsed_dir = create_test_context(ObjectType::Prefix, false);
        assert!(action.predicate(&collapsed_dir));

        // Should not apply to expanded directories
        let expanded_dir = create_test_context(ObjectType::Prefix, true);
        assert!(!action.predicate(&expanded_dir));

        // Should not apply to files
        let file_context = create_test_context(ObjectType::Text, false);
        assert!(!action.predicate(&file_context));
    }

    #[test]
    fn test_example_registry() {
        let registry = create_example_registry();
        assert_eq!(registry.len(), 5);

        // Test that actions are discoverable
        assert!(registry.get_action("preview_text").is_some());
        assert!(registry.get_action("expand_archive").is_some());
        assert!(registry.get_action("download").is_some());
        assert!(registry.get_action("inspect_columnar").is_some());
        assert!(registry.get_action("navigate").is_some());
    }

    #[test]
    fn test_context_aware_action_discovery() {
        let registry = create_example_registry();

        // For a text file, should get preview and download actions
        let text_context = create_test_context(ObjectType::Text, false);
        let applicable = registry.applicable_actions(&text_context);
        assert!(applicable.len() >= 2);
        assert!(applicable.iter().any(|a| a.id() == "preview_text"));
        assert!(applicable.iter().any(|a| a.id() == "download"));

        // For an archive, should get expand and download actions
        let archive_context = create_test_context(ObjectType::Archive, false);
        let applicable = registry.applicable_actions(&archive_context);
        assert!(applicable.iter().any(|a| a.id() == "expand_archive"));
        assert!(applicable.iter().any(|a| a.id() == "download"));

        // For a directory, should get navigate action
        let dir_context = create_test_context(ObjectType::Prefix, false);
        let applicable = registry.applicable_actions(&dir_context);
        assert!(applicable.iter().any(|a| a.id() == "navigate"));
    }

    #[test]
    fn test_action_priority_sorting() {
        let registry = create_example_registry();
        let context = create_test_context(ObjectType::Text, false);
        let applicable = registry.applicable_actions(&context);

        // Higher priority actions should come first
        if applicable.len() >= 2 {
            let first_priority = applicable[0].priority();
            for action in &applicable[1..] {
                assert!(first_priority >= action.priority());
            }
        }
    }
}
