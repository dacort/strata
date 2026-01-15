//! Provider selector modal - allows choosing a provider (S3, GCS, HuggingFace, etc.)

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

pub fn render_provider_selector(frame: &mut Frame, app: &App, area: Rect) {
    // Clear the area first (for overlay effect)
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Select Provider ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Choose a data provider to explore:",
            Style::default().fg(Color::White),
        )]),
        Line::from(vec![Span::styled(
            "  ─────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )]),
        Line::from(""),
    ];

    // Add provider items
    for (idx, provider) in app.providers.iter().enumerate() {
        let is_selected = idx == app.provider_selector_index;
        let prefix = if is_selected { "> " } else { "  " };

        let name_style = if is_selected {
            if provider.enabled {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            }
        } else if provider.enabled {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut spans = vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(provider.name, name_style),
        ];

        if let Some(ref status) = provider.status {
            spans.push(Span::styled(
                format!("  {}", status),
                Style::default().fg(Color::DarkGray),
            ));
        }

        lines.push(Line::from(spans));
    }

    // Add padding and footer
    lines.push(Line::from(""));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled("↑↓", Style::default().fg(Color::Cyan)),
        Span::styled(" navigate  ", Style::default().fg(Color::White)),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::styled(" select  ", Style::default().fg(Color::White)),
        Span::styled("q", Style::default().fg(Color::Cyan)),
        Span::styled(" quit", Style::default().fg(Color::White)),
    ]));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(Color::Black));

    frame.render_widget(paragraph, area);
}
