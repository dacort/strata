//! File preview pane - displays file content with metadata.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::app::App;
use crate::preview::{PreviewContent, PreviewMode, format_size};

/// Render the file preview pane
pub fn render_file_preview(frame: &mut Frame, app: &App, area: Rect) {
    let preview = match &app.file_preview {
        Some(p) => p,
        None => return,
    };

    // Build title with filename and mode indicator
    let mode_indicator = match preview.mode {
        PreviewMode::Head => "HEAD",
        PreviewMode::Tail => "TAIL",
    };
    let title = format!(" {} [{}] ", preview.name, mode_indicator);

    // Create main block with focus-aware border color
    let border_color = if app.preview_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Split area into header, content, and footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header with metadata
            Constraint::Min(1),    // Content area
            Constraint::Length(2), // Footer with keybindings
        ])
        .split(block.inner(area));

    // Render the outer block
    frame.render_widget(block, area);

    // Render header with metadata
    render_header(frame, preview, chunks[0]);

    // Render content based on type
    match &preview.content {
        PreviewContent::Loading => {
            render_loading(frame, app, chunks[1]);
        }
        PreviewContent::Text {
            lines, truncated, ..
        } => {
            render_text_content(frame, lines, *truncated, preview.scroll_offset, chunks[1]);
        }
        PreviewContent::Binary => {
            render_binary_info(frame, preview, chunks[1]);
        }
        PreviewContent::Error(err) => {
            render_error(frame, err, chunks[1]);
        }
        PreviewContent::NotLoaded => {
            render_loading(frame, app, chunks[1]);
        }
    }

    // Render footer with keybindings
    render_footer(frame, preview, chunks[2]);
}

fn render_header(frame: &mut Frame, preview: &crate::preview::FilePreview, area: Rect) {
    let mut spans = Vec::new();

    // File size
    if let Some(size) = preview.size {
        spans.push(Span::styled("Size: ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format_size(size),
            Style::default().fg(Color::White),
        ));
        spans.push(Span::raw("  "));
    }

    // Last modified
    if let Some(ref modified) = preview.last_modified {
        spans.push(Span::styled(
            "Modified: ",
            Style::default().fg(Color::DarkGray),
        ));
        // Parse and format nicely if possible, otherwise show raw
        let display = if modified.len() > 10 {
            &modified[..10] // Just the date part
        } else {
            modified
        };
        spans.push(Span::styled(
            display.to_string(),
            Style::default().fg(Color::White),
        ));
    }

    // Full path on second line
    let lines = vec![
        Line::from(spans),
        Line::from(vec![Span::styled(
            format!("Path: {}", preview.key),
            Style::default().fg(Color::DarkGray),
        )]),
    ];

    let header = Paragraph::new(lines);
    frame.render_widget(header, area);
}

fn render_loading(frame: &mut Frame, app: &App, area: Rect) {
    let loading_text = format!(" {} Loading...", app.spinner_char());
    let loading = Paragraph::new(Line::from(vec![Span::styled(
        loading_text,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::ITALIC),
    )]))
    .wrap(Wrap { trim: false });

    frame.render_widget(loading, area);
}

fn render_text_content(
    frame: &mut Frame,
    lines: &[String],
    truncated: bool,
    scroll_offset: usize,
    area: Rect,
) {
    let visible_height = area.height as usize;
    let total_lines = lines.len();

    // Build visible lines with line numbers
    let display_lines: Vec<Line> = lines
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(idx, line)| {
            let line_num = format!("{:5} ", idx + 1);
            Line::from(vec![
                Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                Span::styled(line.clone(), Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let content = Paragraph::new(display_lines);
    frame.render_widget(content, area);

    // Show scroll indicator if needed
    if total_lines > visible_height {
        let scroll_info = format!(
            " [{}-{}/{}]{}",
            scroll_offset + 1,
            (scroll_offset + visible_height).min(total_lines),
            total_lines,
            if truncated { " (truncated)" } else { "" }
        );

        // Render in top-right corner
        if area.width > scroll_info.len() as u16 {
            let x = area.x + area.width - scroll_info.len() as u16;
            let indicator_area = Rect::new(x, area.y, scroll_info.len() as u16, 1);
            let indicator = Paragraph::new(Span::styled(
                scroll_info,
                Style::default().fg(Color::Yellow),
            ));
            frame.render_widget(indicator, indicator_area);
        }
    }
}

fn render_binary_info(frame: &mut Frame, preview: &crate::preview::FilePreview, area: Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Binary file - preview not available",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
    ];

    if let Some(size) = preview.size {
        lines.push(Line::from(vec![
            Span::styled("  File size: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format_size(size), Style::default().fg(Color::White)),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "  Press [E] to open in pager or [S] to save locally",
        Style::default().fg(Color::Cyan),
    )]));

    let content = Paragraph::new(lines);
    frame.render_widget(content, area);
}

fn render_error(frame: &mut Frame, err: &str, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Error loading preview:",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            format!("  {}", err),
            Style::default().fg(Color::Red),
        )]),
    ];

    let content = Paragraph::new(lines);
    frame.render_widget(content, area);
}

fn render_footer(frame: &mut Frame, preview: &crate::preview::FilePreview, area: Rect) {
    let is_text = matches!(preview.content, PreviewContent::Text { .. });

    let mut spans = vec![Span::raw(" ")];

    // Head/Tail only for text files
    if is_text {
        spans.push(Span::styled("[H]", Style::default().fg(Color::Cyan)));
        spans.push(Span::styled("ead ", Style::default().fg(Color::White)));
        spans.push(Span::styled("[T]", Style::default().fg(Color::Cyan)));
        spans.push(Span::styled("ail ", Style::default().fg(Color::White)));
    }

    // Always show pager and save
    spans.push(Span::styled("[E]", Style::default().fg(Color::Cyan)));
    spans.push(Span::styled(
        "dit/pager ",
        Style::default().fg(Color::White),
    ));
    spans.push(Span::styled("[S]", Style::default().fg(Color::Cyan)));
    spans.push(Span::styled("ave ", Style::default().fg(Color::White)));
    spans.push(Span::styled("[Esc]", Style::default().fg(Color::Cyan)));
    spans.push(Span::styled(" close", Style::default().fg(Color::White)));

    // Scroll hint for text
    if is_text {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            "j/k scroll",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let footer = Paragraph::new(Line::from(spans));
    frame.render_widget(footer, area);
}
