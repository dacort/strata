//! Action results - outcomes of action execution.

use crate::provider::ObjectInfo;

/// Result of executing an action.
///
/// Actions return one of these variants to tell the application
/// what should happen next.
#[derive(Debug, Clone)]
pub enum ActionResult {
    /// Navigate to a different location in the tree
    Navigate {
        /// Path or key to navigate to
        path: String,
    },

    /// Expand content inline in the tree (e.g., archive contents)
    Expand {
        /// Parent key where children should be inserted
        parent_key: String,
        /// Children to insert
        children: Vec<ObjectInfo>,
    },

    /// Show a preview of the content
    Preview {
        /// Key of the object to preview
        key: String,
        /// Content to display
        content: Vec<u8>,
    },

    /// Display a message to the user
    Message(String),

    /// Action completed with no visible effect
    NoOp,

    /// Action failed with an error
    Error(String),

    /// Trigger an async operation (e.g., download)
    /// The string is a message to show while the operation is in progress
    Async(String),

    /// Multiple results to be processed in sequence
    Multiple(Vec<ActionResult>),
}

impl ActionResult {
    /// Create a navigation result
    pub fn navigate(path: impl Into<String>) -> Self {
        ActionResult::Navigate { path: path.into() }
    }

    /// Create an expand result
    pub fn expand(parent_key: impl Into<String>, children: Vec<ObjectInfo>) -> Self {
        ActionResult::Expand {
            parent_key: parent_key.into(),
            children,
        }
    }

    /// Create a preview result
    pub fn preview(key: impl Into<String>, content: Vec<u8>) -> Self {
        ActionResult::Preview {
            key: key.into(),
            content,
        }
    }

    /// Create a message result
    pub fn message(msg: impl Into<String>) -> Self {
        ActionResult::Message(msg.into())
    }

    /// Create an error result
    pub fn error(err: impl Into<String>) -> Self {
        ActionResult::Error(err.into())
    }

    /// Create an async operation result
    pub fn async_op(msg: impl Into<String>) -> Self {
        ActionResult::Async(msg.into())
    }

    /// Create a no-op result
    pub fn noop() -> Self {
        ActionResult::NoOp
    }

    /// Combine multiple results
    pub fn multiple(results: Vec<ActionResult>) -> Self {
        ActionResult::Multiple(results)
    }

    /// Check if this result represents an error
    pub fn is_error(&self) -> bool {
        matches!(self, ActionResult::Error(_))
    }

    /// Check if this result requires async processing
    pub fn is_async(&self) -> bool {
        matches!(self, ActionResult::Async(_))
    }

    /// Check if this result will show something to the user
    pub fn is_visible(&self) -> bool {
        matches!(
            self,
            ActionResult::Navigate { .. }
                | ActionResult::Expand { .. }
                | ActionResult::Preview { .. }
                | ActionResult::Message(_)
                | ActionResult::Error(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ObjectInfo;

    #[test]
    fn test_navigate() {
        let result = ActionResult::navigate("path/to/dir");
        match &result {
            ActionResult::Navigate { path } => assert_eq!(path, "path/to/dir"),
            _ => panic!("Expected Navigate result"),
        }
        assert!(result.is_visible());
    }

    #[test]
    fn test_expand() {
        let children = vec![ObjectInfo::object("file.txt", "path/file.txt", 100)];
        let result = ActionResult::expand("parent/", children.clone());
        match &result {
            ActionResult::Expand {
                parent_key,
                children: c,
            } => {
                assert_eq!(parent_key, "parent/");
                assert_eq!(c.len(), 1);
            }
            _ => panic!("Expected Expand result"),
        }
        assert!(result.is_visible());
    }

    #[test]
    fn test_preview() {
        let content = b"Hello, world!".to_vec();
        let result = ActionResult::preview("file.txt", content.clone());
        match &result {
            ActionResult::Preview { key, content: c } => {
                assert_eq!(key, "file.txt");
                assert_eq!(c, &content);
            }
            _ => panic!("Expected Preview result"),
        }
        assert!(result.is_visible());
    }

    #[test]
    fn test_message() {
        let result = ActionResult::message("Operation complete");
        match &result {
            ActionResult::Message(msg) => assert_eq!(msg, "Operation complete"),
            _ => panic!("Expected Message result"),
        }
        assert!(result.is_visible());
    }

    #[test]
    fn test_error() {
        let result = ActionResult::error("Something went wrong");
        match &result {
            ActionResult::Error(err) => assert_eq!(err, "Something went wrong"),
            _ => panic!("Expected Error result"),
        }
        assert!(result.is_error());
        assert!(result.is_visible());
    }

    #[test]
    fn test_async_op() {
        let result = ActionResult::async_op("Downloading...");
        match &result {
            ActionResult::Async(msg) => assert_eq!(msg, "Downloading..."),
            _ => panic!("Expected Async result"),
        }
        assert!(result.is_async());
    }

    #[test]
    fn test_noop() {
        let result = ActionResult::noop();
        assert!(matches!(result, ActionResult::NoOp));
        assert!(!result.is_visible());
        assert!(!result.is_error());
        assert!(!result.is_async());
    }

    #[test]
    fn test_multiple() {
        let results = vec![
            ActionResult::message("First"),
            ActionResult::message("Second"),
        ];
        let result = ActionResult::multiple(results);
        match &result {
            ActionResult::Multiple(r) => assert_eq!(r.len(), 2),
            _ => panic!("Expected Multiple result"),
        }
    }
}
