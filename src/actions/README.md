# Actions Module

Context-aware action system for data-shell TUI.

## Module Structure

```
actions/
├── mod.rs           # Action trait and public API
├── context.rs       # ActionContext - state for predicate evaluation
├── result.rs        # ActionResult - outcome types
├── registry.rs      # ActionRegistry - central action store
├── examples.rs      # Example action implementations
└── README.md        # This file
```

## Quick Start

### 1. Define an Action

```rust
use crate::actions::{Action, ActionContext, ActionResult};
use anyhow::Result;

struct MyAction;

impl Action for MyAction {
    fn id(&self) -> &str { "my_action" }
    fn title(&self) -> &str { "My Action" }

    fn predicate(&self, context: &ActionContext) -> bool {
        // Determine if this action applies
        context.has_selection()
    }

    fn execute(&self, context: &ActionContext) -> Result<ActionResult> {
        Ok(ActionResult::message("Action executed!"))
    }
}
```

### 2. Register Actions

```rust
use std::sync::Arc;
use crate::actions::ActionRegistry;

let mut registry = ActionRegistry::new();
registry.register(Arc::new(MyAction));
```

### 3. Discover and Execute

```rust
// Build context from app state
let context = ActionContext::new(
    selected_object,
    provider_name,
    is_expanded,
);

// Get applicable actions
let actions = registry.applicable_actions(&context);

// Execute
if let Some(action) = actions.first() {
    let result = action.execute(&context)?;
    // Handle result...
}
```

## Core Types

### Action Trait

```rust
pub trait Action: Send + Sync {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn description(&self) -> Option<&str>;
    fn predicate(&self, context: &ActionContext) -> bool;
    fn execute(&self, context: &ActionContext) -> Result<ActionResult>;
    fn priority(&self) -> i32;
    fn shortcut(&self) -> Option<char>;
}
```

### ActionContext

Provides:
- Selected object information
- Provider capabilities
- View state (expanded/collapsed)
- Helper methods for common predicates

### ActionResult

Variants:
- `Navigate { path }` - Change location
- `Expand { parent_key, children }` - Insert content inline
- `Preview { key, content }` - Show preview
- `Message(String)` - Status message
- `Error(String)` - Error message
- `Async(String)` - Background operation
- `NoOp` - No visible effect
- `Multiple(Vec<ActionResult>)` - Combined results

### ActionRegistry

Methods:
- `register()` - Add action
- `applicable_actions()` - Filter by context
- `get_action()` - Lookup by ID
- `actions_with_shortcut()` - Lookup by key

## Examples

See `examples.rs` for complete implementations:

```rust
use crate::actions::examples::create_example_registry;

let registry = create_example_registry();
// Returns registry with:
// - PreviewTextAction
// - ExpandArchiveAction
// - DownloadAction
// - InspectColumnarAction
// - NavigateAction
```

## Testing

```bash
# Run all tests
cargo test actions

# Run specific module
cargo test actions::registry
```

## Documentation

See [ACTION_FRAMEWORK.md](../../ACTION_FRAMEWORK.md) for:
- Architecture overview
- Integration guide
- Future roadmap
- API reference

## Status

✅ **Phase 1 Complete** (Foundation)
- Core traits defined
- Context management
- Result types
- Registry implementation
- Example actions
- Comprehensive tests (35 passing)

## Contributing

1. Implement `Action` trait
2. Add unit tests
3. Document behavior
4. Use appropriate priority
5. Choose unique shortcuts

See examples in `examples.rs`.
