//! Preview pane - displays object metadata and content preview.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::{App, Focus};
use crate::provider::ObjectType;

pub fn render_preview(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::Preview;

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title("Preview")
        .borders(Borders::ALL)
        .border_style(border_style);

    // Get selected object info
    let content = if let Some(obj) = app.listing.selected() {
        build_preview_content(obj, app.preview_content.as_deref())
    } else {
        vec![Line::from(Span::styled(
            "Select an object to preview",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        ))]
    };

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn build_preview_content(obj: &crate::provider::ObjectInfo, preview_text: Option<&str>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header with object info
    lines.push(Line::from(vec![
        Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
        Span::styled(obj.name.clone(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));

    lines.push(Line::from(vec![
        Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} {}", obj.object_type.icon(), obj.object_type), Style::default().fg(Color::Cyan)),
    ]));

    if let Some(size) = obj.size {
        lines.push(Line::from(vec![
            Span::styled("Size: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_size_detailed(size), Style::default().fg(Color::Yellow)),
        ]));
    }

    if let Some(modified) = &obj.last_modified {
        lines.push(Line::from(vec![
            Span::styled("Modified: ", Style::default().fg(Color::DarkGray)),
            Span::styled(modified.clone(), Style::default().fg(Color::Green)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("Key: ", Style::default().fg(Color::DarkGray)),
        Span::styled(obj.key.clone(), Style::default().fg(Color::White)),
    ]));

    // Add type-specific hints
    lines.push(Line::from(""));
    match obj.object_type {
        ObjectType::Prefix => {
            lines.push(Line::from(Span::styled(
                "Press Enter to navigate into this directory",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
        ObjectType::Text => {
            lines.push(Line::from(Span::styled(
                "Press Enter to preview • p for pager",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
        ObjectType::Archive => {
            lines.push(Line::from(Span::styled(
                "Press Enter to browse archive contents",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
        ObjectType::Columnar => {
            lines.push(Line::from(Span::styled(
                "Press Enter to inspect schema and data",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
        ObjectType::Binary => {
            lines.push(Line::from(Span::styled(
                "Press d to download • i for hex inspector",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
    }

    // Add preview content if available
    if let Some(text) = preview_text {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "─── Preview ───",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(""));

        for line in text.lines().take(20) {
            lines.push(Line::from(Span::raw(line.to_string())));
        }

        if text.lines().count() > 20 {
            lines.push(Line::from(Span::styled(
                "... (truncated)",
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            )));
        }
    }

    lines
}

fn format_size_detailed(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB ({} bytes)", bytes as f64 / GB as f64, bytes)
    } else if bytes >= MB {
        format!("{:.2} MB ({} bytes)", bytes as f64 / MB as f64, bytes)
    } else if bytes >= KB {
        format!("{:.2} KB ({} bytes)", bytes as f64 / KB as f64, bytes)
    } else {
        format!("{} bytes", bytes)
    }
}
