//! Action registry - central store for discovering and managing actions.

use std::sync::Arc;

use super::{Action, ActionContext};

/// Central registry for all actions in the system.
///
/// The registry stores actions and provides methods to:
/// - Register new actions
/// - Query applicable actions for a given context
/// - Retrieve actions by ID
pub struct ActionRegistry {
    /// Registered actions (using Arc for cheap cloning)
    actions: Vec<Arc<dyn Action>>,
}

impl ActionRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    /// Register a new action
    ///
    /// # Example
    /// ```ignore
    /// let mut registry = ActionRegistry::new();
    /// registry.register(Arc::new(PreviewTextAction));
    /// ```
    pub fn register(&mut self, action: Arc<dyn Action>) {
        self.actions.push(action);
    }

    /// Get all actions applicable to the given context.
    ///
    /// Returns actions sorted by priority (highest first), then by title.
    pub fn applicable_actions(&self, context: &ActionContext) -> Vec<Arc<dyn Action>> {
        let mut applicable: Vec<Arc<dyn Action>> = self
            .actions
            .iter()
            .filter(|action| action.predicate(context))
            .cloned()
            .collect();

        // Sort by priority (descending), then by title (ascending)
        applicable.sort_by(|a, b| {
            b.priority()
                .cmp(&a.priority())
                .then_with(|| a.title().cmp(b.title()))
        });

        applicable
    }

    /// Get an action by its ID
    pub fn get_action(&self, id: &str) -> Option<Arc<dyn Action>> {
        self.actions
            .iter()
            .find(|action| action.id() == id)
            .cloned()
    }

    /// Get all registered actions (regardless of context)
    pub fn all_actions(&self) -> &[Arc<dyn Action>] {
        &self.actions
    }

    /// Get the count of registered actions
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Clear all registered actions
    pub fn clear(&mut self) {
        self.actions.clear();
    }

    /// Get actions with a specific shortcut key
    pub fn actions_with_shortcut(&self, shortcut: char) -> Vec<Arc<dyn Action>> {
        self.actions
            .iter()
            .filter(|action| action.shortcut() == Some(shortcut))
            .cloned()
            .collect()
    }
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::ActionResult;
    use crate::provider::{ObjectInfo, ObjectType};
    use anyhow::Result;

    // Mock actions for testing
    struct HighPriorityAction;
    impl Action for HighPriorityAction {
        fn id(&self) -> &str {
            "high_priority"
        }
        fn title(&self) -> &str {
            "High Priority"
        }
        fn predicate(&self, _context: &ActionContext) -> bool {
            true
        }
        fn execute(&self, _context: &ActionContext) -> Result<ActionResult> {
            Ok(ActionResult::noop())
        }
        fn priority(&self) -> i32 {
            100
        }
    }

    struct LowPriorityAction;
    impl Action for LowPriorityAction {
        fn id(&self) -> &str {
            "low_priority"
        }
        fn title(&self) -> &str {
            "Low Priority"
        }
        fn predicate(&self, _context: &ActionContext) -> bool {
            true
        }
        fn execute(&self, _context: &ActionContext) -> Result<ActionResult> {
            Ok(ActionResult::noop())
        }
        fn priority(&self) -> i32 {
            10
        }
    }

    struct ArchiveOnlyAction;
    impl Action for ArchiveOnlyAction {
        fn id(&self) -> &str {
            "archive_only"
        }
        fn title(&self) -> &str {
            "Archive Only"
        }
        fn predicate(&self, context: &ActionContext) -> bool {
            context.is_object_type(&ObjectType::Archive)
        }
        fn execute(&self, _context: &ActionContext) -> Result<ActionResult> {
            Ok(ActionResult::noop())
        }
    }

    struct ShortcutAction;
    impl Action for ShortcutAction {
        fn id(&self) -> &str {
            "shortcut"
        }
        fn title(&self) -> &str {
            "Shortcut Action"
        }
        fn predicate(&self, _context: &ActionContext) -> bool {
            true
        }
        fn execute(&self, _context: &ActionContext) -> Result<ActionResult> {
            Ok(ActionResult::noop())
        }
        fn shortcut(&self) -> Option<char> {
            Some('s')
        }
    }

    fn create_test_context(obj_type: ObjectType) -> ActionContext {
        ActionContext::new(
            Some(ObjectInfo {
                name: "test".to_string(),
                key: "test".to_string(),
                object_type: obj_type,
                size: Some(100),
                last_modified: None,
            }),
            "s3",
            false,
        )
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ActionRegistry::new();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        registry.register(Arc::new(HighPriorityAction));
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());

        let action = registry.get_action("high_priority");
        assert!(action.is_some());
        assert_eq!(action.unwrap().id(), "high_priority");
    }

    #[test]
    fn test_applicable_actions_filtering() {
        let mut registry = ActionRegistry::new();
        registry.register(Arc::new(HighPriorityAction));
        registry.register(Arc::new(ArchiveOnlyAction));

        // Context with text file - should only match HighPriorityAction
        let text_context = create_test_context(ObjectType::Text);
        let applicable = registry.applicable_actions(&text_context);
        assert_eq!(applicable.len(), 1);
        assert_eq!(applicable[0].id(), "high_priority");

        // Context with archive - should match both
        let archive_context = create_test_context(ObjectType::Archive);
        let applicable = registry.applicable_actions(&archive_context);
        assert_eq!(applicable.len(), 2);
    }

    #[test]
    fn test_action_sorting() {
        let mut registry = ActionRegistry::new();
        registry.register(Arc::new(LowPriorityAction));
        registry.register(Arc::new(HighPriorityAction));

        let context = create_test_context(ObjectType::Text);
        let applicable = registry.applicable_actions(&context);

        // Should be sorted by priority (high first)
        assert_eq!(applicable.len(), 2);
        assert_eq!(applicable[0].id(), "high_priority");
        assert_eq!(applicable[1].id(), "low_priority");
    }

    #[test]
    fn test_all_actions() {
        let mut registry = ActionRegistry::new();
        registry.register(Arc::new(HighPriorityAction));
        registry.register(Arc::new(LowPriorityAction));

        let all = registry.all_actions();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn test_clear() {
        let mut registry = ActionRegistry::new();
        registry.register(Arc::new(HighPriorityAction));
        assert_eq!(registry.len(), 1);

        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }

    #[test]
    fn test_get_nonexistent_action() {
        let registry = ActionRegistry::new();
        assert!(registry.get_action("nonexistent").is_none());
    }

    #[test]
    fn test_actions_with_shortcut() {
        let mut registry = ActionRegistry::new();
        registry.register(Arc::new(ShortcutAction));
        registry.register(Arc::new(HighPriorityAction));

        let with_shortcut = registry.actions_with_shortcut('s');
        assert_eq!(with_shortcut.len(), 1);
        assert_eq!(with_shortcut[0].id(), "shortcut");

        let no_shortcut = registry.actions_with_shortcut('x');
        assert_eq!(no_shortcut.len(), 0);
    }

    #[test]
    fn test_default() {
        let registry = ActionRegistry::default();
        assert!(registry.is_empty());
    }
}
