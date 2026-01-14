//! UI components and rendering.

mod tree_view;
mod footer;
mod help;
mod context_selector;
mod provider_selector;
mod file_preview;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::{App, AppMode};

pub use tree_view::render_tree;
pub use footer::render_footer;
pub use help::render_help;
pub use context_selector::render_context_selector;
pub use provider_selector::render_provider_selector;
pub use file_preview::render_file_preview;

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
            // Normal browsing mode - tree view with footer
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),      // Tree view
                    Constraint::Length(1),   // Footer/status bar
                ])
                .split(frame.area());

            let tree_area = chunks[0];
            let footer_area = chunks[1];

            // Render tree view (full width)
            render_tree(frame, app, tree_area);
            render_footer(frame, app, footer_area);

            // Render file preview if active (highest priority modal)
            if app.show_file_preview {
                render_file_preview(frame, app, centered_rect(80, 80, frame.area()));
            }

            // Render help overlay if active
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
