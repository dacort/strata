//! Footer/status bar - displays status messages and keybinding hints.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, StatusLevel};

pub fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let (left, right) = build_footer_content(app);

    // Calculate spacing
    let left_len: usize = left.iter().map(|s| s.content.len()).sum();
    let right_len: usize = right.iter().map(|s| s.content.len()).sum();
    let space_needed = area.width as usize - left_len - right_len;

    let mut spans = left;
    spans.push(Span::raw(" ".repeat(space_needed.max(1))));
    spans.extend(right);

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(paragraph, area);
}

fn build_footer_content(app: &App) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
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
        // Default status: show object count
        let count = app.listing.objects.len();
        let more = if app.listing.has_more { "+" } else { "" };
        vec![Span::styled(
            format!(" {} objects{} ", count, more),
            Style::default().fg(Color::White),
        )]
    };

    // Right side: keybinding hints
    let right = vec![
        Span::styled(" ? ", Style::default().fg(Color::Cyan)),
        Span::styled("help ", Style::default().fg(Color::DarkGray)),
        Span::styled(" q ", Style::default().fg(Color::Cyan)),
        Span::styled("quit ", Style::default().fg(Color::DarkGray)),
    ];

    (left, right)
}
