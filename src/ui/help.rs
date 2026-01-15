//! Help overlay - displays available keybindings.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub fn render_help(frame: &mut Frame, area: Rect) {
    // Clear the area first (for overlay effect)
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let help_lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Navigation",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        keybinding_line("↑/k", "Move up"),
        keybinding_line("↓/j", "Move down"),
        keybinding_line("Enter", "Expand dir / preview file"),
        keybinding_line("→/l", "Expand directory"),
        keybinding_line("←/h", "Collapse or go to parent"),
        keybinding_line("g", "Go to first item"),
        keybinding_line("G", "Go to last item"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  File Preview",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        keybinding_line("H", "Head mode (first lines)"),
        keybinding_line("T", "Tail mode (last lines)"),
        keybinding_line("E", "Open in external pager"),
        keybinding_line("S", "Save file locally"),
        keybinding_line("j/k", "Scroll in preview"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Actions",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        keybinding_line("r", "Refresh tree"),
        keybinding_line("L", "Load more items (truncated)"),
        keybinding_line("c", "Switch context (bucket/resource)"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  General",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        keybinding_line("?", "Toggle this help"),
        keybinding_line("q/Esc", "Quit"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Press any key to close",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]),
    ];

    let paragraph = Paragraph::new(help_lines)
        .block(block)
        .style(Style::default().bg(Color::Black));

    frame.render_widget(paragraph, area);
}

fn keybinding_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:12}", key), Style::default().fg(Color::Cyan)),
        Span::styled(desc.to_string(), Style::default().fg(Color::White)),
    ])
}
