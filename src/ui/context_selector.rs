//! Context selector modal - allows switching between buckets/contexts.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

pub fn render_context_selector(frame: &mut Frame, app: &App, area: Rect) {
    // Clear the area first (for overlay effect)
    frame.render_widget(Clear, area);

    // Determine title and header based on mode
    let (title, header) = if app.mode == crate::app::AppMode::SelectResource {
        // In resource selector mode, show provider-specific header
        if let Some(ref provider_id) = app.selected_provider_id {
            match provider_id.as_str() {
                "s3" => (" Select S3 Bucket ", "  S3 Buckets"),
                "gcs" => (" Select GCS Bucket ", "  GCS Buckets"),
                "hf-datasets" => (" Select Dataset ", "  HuggingFace Datasets"),
                _ => (" Select Resource ", "  Resources"),
            }
        } else {
            (" Select Resource ", "  Resources")
        }
    } else {
        // In browse mode context switching, use the existing logic
        (" Select Context ", "")
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let mut lines = vec![
        Line::from(""),
    ];

    // Only show header if not in browse mode
    if app.mode == crate::app::AppMode::SelectResource {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {}", header),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));
    } else if let Some(ref context) = app.context {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} Buckets", context.provider_name.to_uppercase()),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled(
            "  ─────────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    lines.push(Line::from(""));

    // Calculate how many items can fit in the visible area
    // Account for: 2 borders + header lines (4) + footer lines (3)
    let header_lines = 4u16;
    let footer_lines = 3u16;
    let chrome_height = 2 + header_lines + footer_lines; // borders + header + footer
    let available_height = area.height.saturating_sub(chrome_height) as usize;
    
    // Calculate scroll offset to keep selected item visible
    let total_items = app.contexts.len();
    let scroll_offset = if available_height > 0 && total_items > available_height {
        // Keep the selected item visible with some context
        let selected = app.context_selector_index;
        if selected < available_height / 2 {
            0
        } else if selected >= total_items.saturating_sub(available_height / 2) {
            total_items.saturating_sub(available_height)
        } else {
            selected.saturating_sub(available_height / 2)
        }
    } else {
        0
    };

    // Add context items (only the visible window)
    let end_idx = (scroll_offset + available_height).min(total_items);
    for idx in scroll_offset..end_idx {
        let context = &app.contexts[idx];
        let is_selected = idx == app.context_selector_index;
        let prefix = if is_selected { "> " } else { "  " };

        let name_style = if is_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(prefix, Style::default().fg(Color::Cyan)),
            Span::styled(context.name.clone(), name_style),
        ];

        if let Some(ref desc) = context.description {
            spans.push(Span::styled(
                format!(" - {}", desc),
                Style::default().fg(Color::DarkGray),
            ));
        }

        lines.push(Line::from(spans));
    }

    // Add padding and footer
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Show different footer based on whether we can go back
    if app.mode == crate::app::AppMode::SelectResource {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("↑↓", Style::default().fg(Color::Cyan)),
            Span::styled(" navigate  ", Style::default().fg(Color::White)),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" select  ", Style::default().fg(Color::White)),
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::styled(" back  ", Style::default().fg(Color::White)),
            Span::styled("q", Style::default().fg(Color::Cyan)),
            Span::styled(" quit", Style::default().fg(Color::White)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled("↑↓", Style::default().fg(Color::Cyan)),
            Span::styled(" navigate  ", Style::default().fg(Color::White)),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" select  ", Style::default().fg(Color::White)),
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::styled(" close", Style::default().fg(Color::White)),
        ]));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(Color::Black));

    frame.render_widget(paragraph, area);
}
