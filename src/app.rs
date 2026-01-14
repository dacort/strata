//! Application state and core logic.
//!
//! The app maintains:
//! - Tree state for hierarchical browsing
//! - Status messages
//! - UI state (help overlay, etc.)

use std::time::{Duration, Instant};

use crate::preview::{FilePreview, PreviewMode};
use crate::provider::{ContextInfo, ObjectInfo, ProviderContext};
use crate::registry::ProviderInfo;
use crate::tree::TreeState;

/// Application mode - determines what UI is shown
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Selecting a provider (step 1)
    SelectProvider,
    /// Selecting a resource (bucket, dataset, etc.) (step 2)
    SelectResource,
    /// Normal tree browsing
    Browse,
}

/// Status message severity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Warn,
    Error,
}

/// A status message with optional timeout
#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub level: StatusLevel,
    pub expires_at: Option<Instant>,
}

impl StatusMessage {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: StatusLevel::Info,
            expires_at: Some(Instant::now() + Duration::from_secs(5)),
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: StatusLevel::Error,
            expires_at: None, // Errors persist until dismissed
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.map_or(false, |t| Instant::now() > t)
    }
}

/// Main application state
pub struct App {
    /// Current application mode
    pub mode: AppMode,
    /// Current context (bucket, prefix, etc.) - None when selecting provider
    pub context: Option<ProviderContext>,
    /// Tree state for browsing
    pub tree: TreeState,
    /// Status bar messages
    pub status: Option<StatusMessage>,
    /// Whether app should quit
    pub should_quit: bool,
    /// Show help overlay
    pub show_help: bool,
    /// Show context selector modal (deprecated - use mode instead)
    pub show_context_selector: bool,
    /// Available contexts (buckets, projects, etc.)
    pub contexts: Vec<ContextInfo>,
    /// Selected index in context selector
    pub context_selector_index: usize,
    /// Available providers
    pub providers: Vec<ProviderInfo>,
    /// Selected index in provider selector
    pub provider_selector_index: usize,
    /// Selected provider ID (when in SelectResource mode)
    pub selected_provider_id: Option<String>,
    /// Loading indicator state
    pub loading_spinner: usize,
    /// Scroll offset for the tree view
    pub scroll_offset: usize,
    /// File preview pane visibility
    pub preview_visible: bool,
    /// Whether preview pane has focus (vs tree)
    pub preview_focused: bool,
    /// Current file preview data
    pub file_preview: Option<FilePreview>,
}

impl App {
    /// Create app in Browse mode with a known context
    pub fn new(context: ProviderContext) -> Self {
        Self {
            mode: AppMode::Browse,
            context: Some(context),
            tree: TreeState::new(),
            status: None,
            should_quit: false,
            show_help: false,
            show_context_selector: false,
            contexts: Vec::new(),
            context_selector_index: 0,
            providers: Vec::new(),
            provider_selector_index: 0,
            selected_provider_id: None,
            loading_spinner: 0,
            scroll_offset: 0,
            preview_visible: false,
            preview_focused: false,
            file_preview: None,
        }
    }

    /// Create app in SelectProvider mode
    pub fn new_with_provider_selector(providers: Vec<ProviderInfo>) -> Self {
        Self {
            mode: AppMode::SelectProvider,
            context: None,
            tree: TreeState::new(),
            status: None,
            should_quit: false,
            show_help: false,
            show_context_selector: false,
            contexts: Vec::new(),
            context_selector_index: 0,
            providers,
            provider_selector_index: 0,
            selected_provider_id: None,
            loading_spinner: 0,
            scroll_offset: 0,
            preview_visible: false,
            preview_focused: false,
            file_preview: None,
        }
    }

    /// Transition to SelectResource mode with a chosen provider
    pub fn enter_resource_selector(&mut self, provider_id: String) {
        self.mode = AppMode::SelectResource;
        self.selected_provider_id = Some(provider_id);
        self.context_selector_index = 0;
        self.contexts.clear();
    }

    /// Transition to Browse mode with a chosen context
    pub fn enter_browse_mode(&mut self, context: ProviderContext) {
        self.mode = AppMode::Browse;
        self.context = Some(context);
        self.tree = TreeState::new();
    }

    /// Go back to provider selector
    pub fn back_to_provider_selector(&mut self) {
        self.mode = AppMode::SelectProvider;
        self.selected_provider_id = None;
        self.contexts.clear();
    }

    /// Set a status message
    pub fn set_status(&mut self, msg: StatusMessage) {
        self.status = Some(msg);
    }

    /// Clear expired status messages
    pub fn clear_expired_status(&mut self) {
        if let Some(ref status) = self.status {
            if status.is_expired() {
                self.status = None;
            }
        }
    }

    /// Toggle help overlay
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    /// Open context selector
    pub fn open_context_selector(&mut self) {
        self.show_context_selector = true;
        self.context_selector_index = 0;
    }

    /// Close context selector
    pub fn close_context_selector(&mut self) {
        self.show_context_selector = false;
    }

    /// Select previous context
    pub fn context_selector_prev(&mut self) {
        if !self.contexts.is_empty() {
            self.context_selector_index = self.context_selector_index.saturating_sub(1);
        }
    }

    /// Select next context
    pub fn context_selector_next(&mut self) {
        if !self.contexts.is_empty() {
            self.context_selector_index =
                (self.context_selector_index + 1).min(self.contexts.len() - 1);
        }
    }

    /// Get currently selected context name
    pub fn selected_context_name(&self) -> Option<String> {
        self.contexts.get(self.context_selector_index).map(|c| c.name.clone())
    }

    /// Select previous provider
    pub fn provider_selector_prev(&mut self) {
        if !self.providers.is_empty() {
            self.provider_selector_index = self.provider_selector_index.saturating_sub(1);
        }
    }

    /// Select next provider
    pub fn provider_selector_next(&mut self) {
        if !self.providers.is_empty() {
            self.provider_selector_index =
                (self.provider_selector_index + 1).min(self.providers.len() - 1);
        }
    }

    /// Get currently selected provider
    pub fn selected_provider(&self) -> Option<&ProviderInfo> {
        self.providers.get(self.provider_selector_index)
    }

    /// Quit the application
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Advance loading spinner
    pub fn tick_spinner(&mut self) {
        self.loading_spinner = (self.loading_spinner + 1) % 8;
    }

    /// Get spinner character
    pub fn spinner_char(&self) -> char {
        const SPINNER: [char; 8] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧'];
        SPINNER[self.loading_spinner]
    }

    /// Ensure selected item is visible by adjusting scroll
    pub fn ensure_visible(&mut self, visible_height: usize) {
        if visible_height == 0 {
            return;
        }

        let selected = self.tree.selected_index;

        // Scroll up if selection is above viewport
        if selected < self.scroll_offset {
            self.scroll_offset = selected;
        }

        // Scroll down if selection is below viewport
        if selected >= self.scroll_offset + visible_height {
            self.scroll_offset = selected - visible_height + 1;
        }
    }

    /// Open file preview pane
    pub fn open_file_preview(&mut self, info: &ObjectInfo) {
        self.file_preview = Some(FilePreview::new(info));
        self.preview_visible = true;
        self.preview_focused = false; // Tree keeps focus initially
    }

    /// Close file preview pane
    pub fn close_file_preview(&mut self) {
        self.preview_visible = false;
        self.preview_focused = false;
        self.file_preview = None;
    }

    /// Focus the preview pane
    pub fn focus_preview(&mut self) {
        if self.preview_visible {
            self.preview_focused = true;
        }
    }

    /// Focus the tree pane
    pub fn focus_tree(&mut self) {
        self.preview_focused = false;
    }

    /// Set preview mode (head/tail) and mark as loading
    pub fn set_preview_mode(&mut self, mode: PreviewMode) {
        if let Some(ref mut preview) = self.file_preview {
            preview.mode = mode;
            preview.set_loading();
        }
    }

    /// Scroll preview up
    pub fn preview_scroll_up(&mut self) {
        if let Some(ref mut preview) = self.file_preview {
            preview.scroll_up();
        }
    }

    /// Scroll preview down
    pub fn preview_scroll_down(&mut self, visible_height: usize) {
        if let Some(ref mut preview) = self.file_preview {
            preview.scroll_down(visible_height);
        }
    }

    /// Page up in preview
    pub fn preview_page_up(&mut self, visible_height: usize) {
        if let Some(ref mut preview) = self.file_preview {
            preview.page_up(visible_height);
        }
    }

    /// Page down in preview
    pub fn preview_page_down(&mut self, visible_height: usize) {
        if let Some(ref mut preview) = self.file_preview {
            preview.page_down(visible_height);
        }
    }
}
