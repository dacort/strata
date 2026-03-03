//! Event handling - keyboard input and async events.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::app::{App, AppMode};
use crate::preview::{PREVIEW_BYTES, PreviewMode};
use crate::provider::ObjectInfo;

/// Application events
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input
    Key(KeyEvent),
    /// Tick for animations/timeouts
    Tick,
    /// Root listing loaded
    RootLoaded(Vec<ObjectInfo>, bool),
    /// Children loaded for a prefix
    ChildrenLoaded {
        parent_key: String,
        objects: Vec<ObjectInfo>,
        has_more: bool,
        continuation_token: Option<String>,
    },
    /// More children loaded for a prefix (pagination)
    MoreChildrenLoaded {
        parent_key: String,
        objects: Vec<ObjectInfo>,
        has_more: bool,
        continuation_token: Option<String>,
    },
    /// Contexts loaded
    ContextsLoaded(Vec<crate::provider::ContextInfo>),
    /// Loading error
    LoadError(String, String), // (prefix, error message)
    /// File preview content loaded
    PreviewLoaded {
        key: String,
        content: Vec<u8>,
        mode: PreviewMode,
    },
    /// Pager process exited
    PagerExited,
}

/// Result of handling a key event
pub enum KeyResult {
    /// Nothing happened
    None,
    /// Event was handled, no action needed
    Handled,
    /// Need to load children for this prefix
    LoadChildren(String),
    /// Need to refresh/reload root
    Refresh,
    /// Need to load contexts
    LoadContexts,
    /// Switch to a new context
    SwitchContext(String),
    /// Provider selected, need to initialize and load resources
    ProviderSelected(String),
    /// Fetch head of file for preview (key, bytes to fetch)
    FetchPreviewHead(String, u64),
    /// Fetch tail of file for preview (key, file_size, bytes to fetch)
    FetchPreviewTail(String, u64, u64),
    /// Open file in external pager
    OpenInPager(String),
    /// Save file to local path (remote_key, local_path)
    SaveToLocal(String, String),
    /// Load more items for a directory (parent_key)
    LoadMore(String),
}

/// Spawn a task to read keyboard events
pub fn spawn_event_reader(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        loop {
            // Poll for events with timeout for tick
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read()
                    && tx.send(AppEvent::Key(key)).await.is_err()
                {
                    break;
                }
            } else {
                // Send tick event
                if tx.send(AppEvent::Tick).await.is_err() {
                    break;
                }
            }
        }
    });
}

/// Handle a key event
pub fn handle_key(app: &mut App, key: KeyEvent) -> KeyResult {
    // Route to appropriate handler based on mode
    match app.mode {
        AppMode::SelectProvider => handle_provider_selector_key(app, key),
        AppMode::SelectResource => handle_resource_selector_key(app, key),
        AppMode::Browse => handle_browse_key(app, key),
    }
}

/// Handle keys in provider selector mode
fn handle_provider_selector_key(app: &mut App, key: KeyEvent) -> KeyResult {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.quit();
            KeyResult::Handled
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.provider_selector_prev();
            KeyResult::Handled
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.provider_selector_next();
            KeyResult::Handled
        }
        KeyCode::Enter => {
            if let Some(provider) = app.selected_provider() {
                if provider.enabled {
                    KeyResult::ProviderSelected(provider.id.to_string())
                } else {
                    // Provider not yet available
                    KeyResult::Handled
                }
            } else {
                KeyResult::Handled
            }
        }
        _ => KeyResult::Handled,
    }
}

/// Handle keys in resource selector mode
fn handle_resource_selector_key(app: &mut App, key: KeyEvent) -> KeyResult {
    match key.code {
        KeyCode::Esc => {
            // Go back to provider selector
            app.back_to_provider_selector();
            KeyResult::Handled
        }
        KeyCode::Char('q') => {
            app.quit();
            KeyResult::Handled
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.context_selector_prev();
            KeyResult::Handled
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.context_selector_next();
            KeyResult::Handled
        }
        KeyCode::Enter => {
            if let Some(context_name) = app.selected_context_name() {
                KeyResult::SwitchContext(context_name)
            } else {
                KeyResult::Handled
            }
        }
        _ => KeyResult::Handled,
    }
}

/// Handle keys in browse mode
fn handle_browse_key(app: &mut App, key: KeyEvent) -> KeyResult {
    // Context selector modal captures input (highest priority)
    if app.show_context_selector {
        return handle_context_selector_key(app, key);
    }

    // Help overlay captures all input
    if app.show_help {
        app.show_help = false;
        return KeyResult::Handled;
    }

    // Preview pane is visible - route based on focus
    if app.preview_visible {
        if app.preview_focused {
            // Preview has focus
            return handle_preview_focused_key(app, key);
        } else {
            // Tree has focus, but check for preview-related keys
            return handle_tree_with_preview_key(app, key);
        }
    }

    // No preview visible - normal tree navigation
    // Global keybindings
    match key.code {
        KeyCode::Char('q') => {
            app.quit();
            return KeyResult::Handled;
        }
        // Esc: cancel loading if active, otherwise quit
        KeyCode::Esc => {
            if app.tree.any_loading() {
                app.tree.cancel_all_loading();
                return KeyResult::Handled;
            }
            app.quit();
            return KeyResult::Handled;
        }
        KeyCode::Char('?') => {
            app.toggle_help();
            return KeyResult::Handled;
        }
        KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_context_selector();
            return KeyResult::LoadContexts;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit();
            return KeyResult::Handled;
        }
        _ => {}
    }

    // Tree navigation
    handle_tree_key(app, key)
}

fn handle_context_selector_key(app: &mut App, key: KeyEvent) -> KeyResult {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.close_context_selector();
            KeyResult::Handled
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.context_selector_prev();
            KeyResult::Handled
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.context_selector_next();
            KeyResult::Handled
        }
        KeyCode::Enter => {
            if let Some(context_name) = app.selected_context_name() {
                app.close_context_selector();
                KeyResult::SwitchContext(context_name)
            } else {
                KeyResult::Handled
            }
        }
        _ => KeyResult::Handled,
    }
}

/// Handle keys when preview pane has focus
fn handle_preview_focused_key(app: &mut App, key: KeyEvent) -> KeyResult {
    match key.code {
        // Tab switches focus back to tree
        KeyCode::Tab => {
            app.focus_tree();
            KeyResult::Handled
        }

        // Esc closes preview entirely (layered "step back")
        KeyCode::Esc => {
            app.close_file_preview();
            KeyResult::Handled
        }

        // 'q' quits the application
        KeyCode::Char('q') => {
            app.quit();
            KeyResult::Handled
        }

        // 'h' or Left arrow return focus to tree
        KeyCode::Char('h') | KeyCode::Left => {
            app.focus_tree();
            KeyResult::Handled
        }

        // Open in pager
        KeyCode::Char('E') | KeyCode::Char('e') => {
            if let Some(ref preview) = app.file_preview {
                KeyResult::OpenInPager(preview.key.clone())
            } else {
                KeyResult::Handled
            }
        }

        // Switch to head mode
        KeyCode::Char('H') => {
            if let Some(ref preview) = app.file_preview {
                let key = preview.key.clone();
                let size = preview.size.unwrap_or(PREVIEW_BYTES);
                let fetch_bytes = PREVIEW_BYTES.min(size);
                app.set_preview_mode(PreviewMode::Head);
                KeyResult::FetchPreviewHead(key, fetch_bytes)
            } else {
                KeyResult::Handled
            }
        }

        // Switch to tail mode
        KeyCode::Char('T') => {
            if let Some(ref preview) = app.file_preview {
                if let Some(size) = preview.size {
                    let key = preview.key.clone();
                    let fetch_bytes = PREVIEW_BYTES.min(size);
                    app.set_preview_mode(PreviewMode::Tail);
                    KeyResult::FetchPreviewTail(key, size, fetch_bytes)
                } else {
                    KeyResult::Handled
                }
            } else {
                KeyResult::Handled
            }
        }

        // Save to local
        KeyCode::Char('S') => {
            if let Some(ref preview) = app.file_preview {
                let remote_key = preview.key.clone();
                // Use filename portion as local filename
                let filename = preview.name.clone();
                KeyResult::SaveToLocal(remote_key, filename)
            } else {
                KeyResult::Handled
            }
        }

        // Scroll up (arrow keys mirror vim keys)
        KeyCode::Up | KeyCode::Char('k') => {
            app.preview_scroll_up();
            KeyResult::Handled
        }

        // Scroll down (arrow keys mirror vim keys)
        KeyCode::Down | KeyCode::Char('j') => {
            // TODO: Get actual visible height from UI
            app.preview_scroll_down(20);
            KeyResult::Handled
        }

        // Page up
        KeyCode::PageUp | KeyCode::Char('b') => {
            app.preview_page_up(20);
            KeyResult::Handled
        }

        // Page down
        KeyCode::PageDown | KeyCode::Char('f') => {
            app.preview_page_down(20);
            KeyResult::Handled
        }

        _ => KeyResult::Handled,
    }
}

/// Handle keys when tree has focus but preview is visible
fn handle_tree_with_preview_key(app: &mut App, key: KeyEvent) -> KeyResult {
    match key.code {
        // Tab moves focus to preview
        KeyCode::Tab => {
            app.focus_preview();
            KeyResult::Handled
        }

        // 'q' quits the application
        KeyCode::Char('q') => {
            app.quit();
            KeyResult::Handled
        }

        // Esc closes preview pane (layered "step back")
        KeyCode::Esc => {
            app.close_file_preview();
            KeyResult::Handled
        }

        // Global keybindings
        KeyCode::Char('?') => {
            app.toggle_help();
            KeyResult::Handled
        }
        KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.open_context_selector();
            KeyResult::LoadContexts
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit();
            KeyResult::Handled
        }

        // Tree navigation (j/k and arrow keys work normally)
        // Note: We need to handle 'l' and Right specially for files
        _ => handle_tree_key(app, key),
    }
}

fn handle_tree_key(app: &mut App, key: KeyEvent) -> KeyResult {
    match key.code {
        // Navigation (arrow keys mirror vim keys exactly)
        KeyCode::Up | KeyCode::Char('k') => {
            app.tree.select_prev();
            KeyResult::Handled
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Check if we're at a loading boundary
            if let Some(parent_key) = app.tree.at_load_more_boundary() {
                if app.tree.is_loading(&parent_key) {
                    // Already loading, stay put until load completes
                    return KeyResult::Handled;
                }
                // Not loading yet, trigger load but stay on current item
                app.tree.set_loading(&parent_key, true);
                return KeyResult::LoadMore(parent_key);
            }
            // Normal navigation
            app.tree.select_next();
            KeyResult::Handled
        }
        // Sibling navigation: ] = next sibling, [ = previous sibling
        KeyCode::Char(']') => {
            app.tree.select_next_sibling();
            KeyResult::Handled
        }
        KeyCode::Char('[') => {
            app.tree.select_prev_sibling();
            KeyResult::Handled
        }
        KeyCode::Char('g') => {
            app.tree.select_first();
            KeyResult::Handled
        }
        KeyCode::Char('G') => {
            app.tree.select_last();
            KeyResult::Handled
        }

        // Enter: expand/collapse directories, open files
        KeyCode::Enter => {
            if let Some(key) = app.tree.selected_key().cloned() {
                // Clone info we need before mutable operations
                let node_info = app.tree.nodes.get(&key).map(|n| (n.is_dir, n.info.clone()));

                if let Some((is_dir, info)) = node_info {
                    if is_dir {
                        // Toggle expansion
                        let was_expanded = app.tree.is_expanded(&key);
                        app.tree.toggle_expanded(&key);

                        // If now expanded and children not loaded, trigger load
                        if !was_expanded && app.tree.needs_children(&key) {
                            app.tree.set_loading(&key, true);
                            return KeyResult::LoadChildren(key);
                        }
                    } else {
                        // File selected - open preview modal
                        let size = info.size.unwrap_or(PREVIEW_BYTES);
                        let fetch_bytes = PREVIEW_BYTES.min(size);
                        app.open_file_preview(&info);
                        return KeyResult::FetchPreviewHead(key, fetch_bytes);
                    }
                }
            }
            KeyResult::Handled
        }

        // Left arrow / h: collapse current or go to parent
        KeyCode::Left | KeyCode::Char('h') => {
            if let Some(key) = app.tree.selected_key().cloned() {
                if app.tree.is_expanded(&key) {
                    // Collapse this node and cancel any pending load
                    app.tree.cancel_loading(&key);
                    app.tree.toggle_expanded(&key);
                } else if let Some(node) = app.tree.nodes.get(&key) {
                    // Move to parent
                    if !node.parent_key.is_empty() {
                        // Find parent's index in visible list
                        if let Some(idx) =
                            app.tree.visible.iter().position(|k| k == &node.parent_key)
                        {
                            app.tree.selected_index = idx;
                        }
                    }
                }
            }
            KeyResult::Handled
        }

        // Right arrow / l: expand directory, or focus preview if on file and preview visible
        KeyCode::Right | KeyCode::Char('l') => {
            if let Some(key) = app.tree.selected_key().cloned()
                && let Some(node) = app.tree.nodes.get(&key)
            {
                if node.is_dir {
                    // Directory: expand if collapsed
                    if !app.tree.is_expanded(&key) {
                        app.tree.toggle_expanded(&key);

                        if app.tree.needs_children(&key) {
                            app.tree.set_loading(&key, true);
                            return KeyResult::LoadChildren(key);
                        }
                    }
                } else {
                    // File: if preview is visible, focus it
                    if app.preview_visible {
                        app.focus_preview();
                    }
                    // Otherwise do nothing (Enter opens preview)
                }
            }
            KeyResult::Handled
        }

        // Refresh
        KeyCode::Char('r') => KeyResult::Refresh,

        // Manual load more items (for truncated listings)
        KeyCode::Char('L') => {
            if let Some(key) = app.tree.selected_key()
                && let Some(node) = app.tree.nodes.get(key)
            {
                // Check if the current selection's parent has more children to load
                let parent_key = if node.parent_key.is_empty() {
                    // At root level - can't load more (yet)
                    return KeyResult::Handled;
                } else {
                    node.parent_key.clone()
                };

                // Check if parent has more children and not already loading
                if let Some(parent_node) = app.tree.nodes.get(&parent_key)
                    && parent_node.has_more_children
                    && parent_node.continuation_token.is_some()
                    && !app.tree.is_loading(&parent_key)
                {
                    app.tree.set_loading(&parent_key, true);
                    return KeyResult::LoadMore(parent_key);
                }
            }
            KeyResult::Handled
        }

        _ => KeyResult::None,
    }
}
