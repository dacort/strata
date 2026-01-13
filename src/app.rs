//! Application state and core logic.
//!
//! The app maintains:
//! - Navigation stack (for back/forward)
//! - Current listing state
//! - Focus management
//! - Status messages

use std::time::{Duration, Instant};

use crate::provider::{ObjectInfo, ProviderContext};

/// Which pane currently has focus
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Navigator,
    Preview,
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

    pub fn warn(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            level: StatusLevel::Warn,
            expires_at: Some(Instant::now() + Duration::from_secs(10)),
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

/// Entry in navigation stack - allows back/forward
#[derive(Debug, Clone)]
pub struct NavEntry {
    pub prefix: String,
    pub selected_index: usize,
    pub scroll_offset: usize,
}

/// Current listing state
#[derive(Debug, Default)]
pub struct ListingState {
    pub objects: Vec<ObjectInfo>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub is_loading: bool,
    pub continuation_token: Option<String>,
    pub has_more: bool,
}

impl ListingState {
    pub fn selected(&self) -> Option<&ObjectInfo> {
        self.objects.get(self.selected_index)
    }

    pub fn select_next(&mut self) {
        if !self.objects.is_empty() {
            self.selected_index = (self.selected_index + 1).min(self.objects.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected_index = self.selected_index.saturating_sub(1);
    }

    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    pub fn select_last(&mut self) {
        if !self.objects.is_empty() {
            self.selected_index = self.objects.len() - 1;
        }
    }
}

/// Main application state
pub struct App {
    /// Current context (bucket, prefix, etc.)
    pub context: ProviderContext,
    /// Navigation history for back/forward
    pub nav_stack: Vec<NavEntry>,
    /// Current position in nav_stack (-1 means at head)
    pub nav_index: isize,
    /// Current listing state
    pub listing: ListingState,
    /// Which pane has focus
    pub focus: Focus,
    /// Status bar messages
    pub status: Option<StatusMessage>,
    /// Whether app should quit
    pub should_quit: bool,
    /// Show help overlay
    pub show_help: bool,
    /// Preview content for selected object
    pub preview_content: Option<String>,
    /// Loading indicator state
    pub loading_spinner: usize,
}

impl App {
    pub fn new(context: ProviderContext) -> Self {
        Self {
            context,
            nav_stack: Vec::new(),
            nav_index: -1,
            listing: ListingState::default(),
            focus: Focus::Navigator,
            status: None,
            should_quit: false,
            show_help: false,
            preview_content: None,
            loading_spinner: 0,
        }
    }

    /// Push current state to nav stack and navigate to new prefix
    pub fn navigate_to(&mut self, prefix: String) {
        // Save current state
        if !self.listing.objects.is_empty() {
            let entry = NavEntry {
                prefix: self.context.current_prefix.clone(),
                selected_index: self.listing.selected_index,
                scroll_offset: self.listing.scroll_offset,
            };

            // Truncate forward history if we're not at the end
            if self.nav_index >= 0 {
                self.nav_stack.truncate(self.nav_index as usize + 1);
            }
            self.nav_stack.push(entry);
            self.nav_index = self.nav_stack.len() as isize - 1;
        }

        // Navigate to new prefix
        self.context.current_prefix = prefix;
        self.listing = ListingState::default();
        self.listing.is_loading = true;
        self.preview_content = None;
    }

    /// Go back in navigation history
    pub fn navigate_back(&mut self) -> bool {
        if self.nav_index >= 0 {
            let entry = &self.nav_stack[self.nav_index as usize];
            self.context.current_prefix = entry.prefix.clone();
            self.listing.selected_index = entry.selected_index;
            self.listing.scroll_offset = entry.scroll_offset;
            self.nav_index -= 1;
            self.listing.is_loading = true;
            true
        } else {
            false
        }
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
}
