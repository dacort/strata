//! UI components and rendering.

mod context_selector;
mod file_preview;
mod footer;
mod help;
mod provider_selector;
mod tree_view;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::{App, AppMode};

pub use context_selector::render_context_selector;
pub use file_preview::render_file_preview;
pub use footer::render_footer;
pub use help::render_help;
pub use provider_selector::render_provider_selector;
pub use tree_view::render_tree;

/// Main UI layout - Full-width tree view with footer
pub fn render(frame: &mut Frame, app: &mut App) {
    match app.mode {
        AppMode::SelectProvider => {
            // Show provider selector modal centered on screen
            render_provider_selector(frame, app, centered_rect(60, 50, frame.area()));
        }
        AppMode::SelectResource => {
            // Show resource selector (context selector) modal centered on screen
            render_context_selector(frame, app, centered_rect(60, 50, frame.area()));
        }
        AppMode::Browse => {
            // Normal browsing mode - tree view with optional preview pane and footer
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),    // Main content area (tree + preview)
                    Constraint::Length(1), // Footer/status bar
                ])
                .split(frame.area());

            let main_area = chunks[0];
            let footer_area = chunks[1];

            // Split main area horizontally if preview is visible
            if app.preview_visible {
                let main_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(40), // Tree view
                        Constraint::Percentage(60), // Preview pane
                    ])
                    .split(main_area);

                let tree_area = main_chunks[0];
                let preview_area = main_chunks[1];

                // Render tree and preview side by side
                render_tree(frame, app, tree_area);
                render_file_preview(frame, app, preview_area);
            } else {
                // No preview - tree takes full width
                render_tree(frame, app, main_area);
            }

            render_footer(frame, app, footer_area);

            // Render help overlay if active (still a modal)
            if app.show_help {
                render_help(frame, centered_rect(60, 70, frame.area()));
            }

            // Render context selector if active (backwards compatibility)
            if app.show_context_selector {
                render_context_selector(frame, app, centered_rect(60, 50, frame.area()));
            }
        }
    }
}

/// Helper to create a centered rectangle for overlays
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
