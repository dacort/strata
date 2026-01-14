//! Footer/status bar - displays status messages, selection path, and keybinding hints.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, StatusLevel};

pub fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let (left, center, right) = build_footer_content(app);

    // Calculate spacing
    let left_len: usize = left.iter().map(|s| s.content.len()).sum();
    let center_len: usize = center.iter().map(|s| s.content.len()).sum();
    let right_len: usize = right.iter().map(|s| s.content.len()).sum();

    let total_content = left_len + center_len + right_len;
    let available_space = (area.width as usize).saturating_sub(total_content);

    // Distribute space: half before center, half after
    let left_space = available_space / 2;
    let right_space = available_space - left_space;

    let mut spans = left;
    spans.push(Span::raw(" ".repeat(left_space.max(1))));
    spans.extend(center);
    spans.push(Span::raw(" ".repeat(right_space.max(1))));
    spans.extend(right);

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(paragraph, area);
}

fn build_footer_content(app: &App) -> (Vec<Span<'static>>, Vec<Span<'static>>, Vec<Span<'static>>) {
    // Left: status message or item count
    let left = if let Some(ref status) = app.status {
        let (icon, color) = match status.level {
            StatusLevel::Info => ("ℹ", Color::Cyan),
            StatusLevel::Warn => ("⚠", Color::Yellow),
            StatusLevel::Error => ("✗", Color::Red),
        };
        vec![
            Span::styled(format!(" {} ", icon), Style::default().fg(color)),
            Span::styled(status.text.clone(), Style::default().fg(Color::White)),
        ]
    } else {
        // Default: show item count
        let count = app.tree.visible.len();
        vec![Span::styled(
            format!(" {} items ", count),
            Style::default().fg(Color::White),
        )]
    };

    // Center: current selection path
    let center = if let Some(node) = app.tree.selected() {
        vec![Span::styled(
            node.info.key.clone(),
            Style::default().fg(Color::Yellow),
        )]
    } else {
        vec![]
    };

    // Right: keybinding hints
    let right = vec![
        Span::styled(" c ", Style::default().fg(Color::Cyan)),
        Span::styled("context ", Style::default().fg(Color::White)),
        Span::styled(" ? ", Style::default().fg(Color::Cyan)),
        Span::styled("help ", Style::default().fg(Color::White)),
        Span::styled(" q ", Style::default().fg(Color::Cyan)),
        Span::styled("quit ", Style::default().fg(Color::White)),
    ];

    (left, center, right)
}
