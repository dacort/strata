//! Action framework for context-aware operations.
//!
//! This module provides a flexible action system where operations are
//! dynamically discovered based on:
//! - Object type (archive, text, columnar data, etc.)
//! - Provider capabilities (S3, local fs, etc.)
//! - Current view state
//!
//! Actions can:
//! - Navigate to new locations
//! - Expand archives inline
//! - Show previews
//! - Download/upload objects
//! - Transform data

mod context;
pub mod examples;
mod registry;
mod result;
pub mod zip;

pub use context::ActionContext;
pub use registry::ActionRegistry;
pub use result::ActionResult;

use anyhow::Result;

/// Core trait for all actions in the system.
///
/// Actions represent operations that can be performed on objects
/// in the file tree. They are context-aware and self-describing.
pub trait Action: Send + Sync {
    /// Unique identifier for this action (e.g., "expand_archive", "preview_text")
    fn id(&self) -> &str;

    /// Human-readable title for display in action menu
    fn title(&self) -> &str;

    /// Optional short help text explaining what this action does
    fn description(&self) -> Option<&str> {
        None
    }

    /// Determines if this action is applicable in the given context.
    ///
    /// This is the core predicate that enables context-aware action discovery.
    /// Examples:
    /// - Archive expansion only applies to .tar.gz, .zip files
    /// - Preview applies to text files
    /// - Download applies to any file with certain provider capabilities
    fn predicate(&self, context: &ActionContext) -> bool;

    /// Execute this action with the given context.
    ///
    /// Returns an ActionResult that describes what should happen next:
    /// - Navigate to a new location
    /// - Expand content inline in the tree
    /// - Show a preview
    /// - Display an error
    fn execute(&self, context: &ActionContext) -> Result<ActionResult>;

    /// Optional priority for sorting actions in the menu (higher = earlier).
    /// Default priority is 0.
    fn priority(&self) -> i32 {
        0
    }

    /// Optional keyboard shortcut (e.g., 'e' for expand, 'p' for preview)
    fn shortcut(&self) -> Option<char> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ObjectInfo, ObjectType};

    /// Mock action for testing
    struct TestAction {
        id: String,
        applies: bool,
    }

    impl Action for TestAction {
        fn id(&self) -> &str {
            &self.id
        }

        fn title(&self) -> &str {
            "Test Action"
        }

        fn description(&self) -> Option<&str> {
            Some("A test action for unit tests")
        }

        fn predicate(&self, _context: &ActionContext) -> bool {
            self.applies
        }

        fn execute(&self, _context: &ActionContext) -> Result<ActionResult> {
            Ok(ActionResult::Message("Test action executed".to_string()))
        }

        fn priority(&self) -> i32 {
            10
        }

        fn shortcut(&self) -> Option<char> {
            Some('t')
        }
    }

    #[test]
    fn test_action_trait_basic() {
        let action = TestAction {
            id: "test_action".to_string(),
            applies: true,
        };

        assert_eq!(action.id(), "test_action");
        assert_eq!(action.title(), "Test Action");
        assert_eq!(action.description(), Some("A test action for unit tests"));
        assert_eq!(action.priority(), 10);
        assert_eq!(action.shortcut(), Some('t'));
    }

    #[test]
    fn test_action_predicate() {
        let applies = TestAction {
            id: "applies".to_string(),
            applies: true,
        };
        let not_applies = TestAction {
            id: "not_applies".to_string(),
            applies: false,
        };

        let context = ActionContext {
            selected: Some(ObjectInfo {
                name: "test.txt".to_string(),
                key: "test.txt".to_string(),
                object_type: ObjectType::Text,
                size: Some(100),
                last_modified: None,
            }),
            provider_name: "s3".to_string(),
            is_expanded: false,
        };

        assert!(applies.predicate(&context));
        assert!(!not_applies.predicate(&context));
    }

    #[test]
    fn test_action_execution() {
        let action = TestAction {
            id: "test".to_string(),
            applies: true,
        };

        let context = ActionContext {
            selected: None,
            provider_name: "s3".to_string(),
            is_expanded: false,
        };

        let result = action.execute(&context).unwrap();
        match result {
            ActionResult::Message(msg) => {
                assert_eq!(msg, "Test action executed");
            }
            _ => panic!("Expected Message result"),
        }
    }
}
