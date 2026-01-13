//! UI components and rendering.

mod navigator;
mod preview;
mod footer;
mod help;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};

use crate::app::App;

pub use navigator::render_navigator;
pub use preview::render_preview;
pub use footer::render_footer;
pub use help::render_help;

/// Main UI layout - Navigator | Preview | Footer
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(5),      // Main content area
            Constraint::Length(1),   // Footer/status bar
        ])
        .split(frame.area());

    let main_area = chunks[0];
    let footer_area = chunks[1];

    // Split main area into Navigator and Preview panes
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Navigator
            Constraint::Percentage(60), // Preview
        ])
        .split(main_area);

    let navigator_area = panes[0];
    let preview_area = panes[1];

    // Render each component
    render_navigator(frame, app, navigator_area);
    render_preview(frame, app, preview_area);
    render_footer(frame, app, footer_area);

    // Render help overlay if active
    if app.show_help {
        render_help(frame, centered_rect(60, 70, frame.area()));
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
