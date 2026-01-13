//! Navigator pane - displays object listing with selection.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::app::{App, Focus};
use crate::provider::ObjectType;

pub fn render_navigator(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::Navigator;

    // Build title with path and loading indicator
    let mut title = app.context.display_path();
    if app.listing.is_loading {
        title = format!("{} {}", app.spinner_char(), title);
    }

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    // Build list items
    let items: Vec<ListItem> = app
        .listing
        .objects
        .iter()
        .enumerate()
        .map(|(i, obj)| {
            let is_selected = i == app.listing.selected_index;

            // Format: icon name [size]
            let icon = obj.object_type.icon();
            let size_str = match &obj.size {
                Some(s) => format_size(*s),
                None => String::new(),
            };

            let name_style = match obj.object_type {
                ObjectType::Prefix => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                ObjectType::Text => Style::default().fg(Color::Green),
                ObjectType::Archive => Style::default().fg(Color::Yellow),
                ObjectType::Columnar => Style::default().fg(Color::Magenta),
                ObjectType::Binary => Style::default().fg(Color::White),
            };

            let mut spans = vec![
                Span::raw(format!("{} ", icon)),
                Span::styled(&obj.name, name_style),
            ];

            if !size_str.is_empty() {
                spans.push(Span::styled(
                    format!("  {}", size_str),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            let line = Line::from(spans);

            let item = ListItem::new(line);
            if is_selected && is_focused {
                item.style(Style::default().bg(Color::DarkGray))
            } else {
                item
            }
        })
        .collect();

    // Show empty state or list
    if items.is_empty() && !app.listing.is_loading {
        let empty_items = vec![ListItem::new(Line::from(vec![Span::styled(
            "  (empty)",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )]))];
        let list = List::new(empty_items).block(block);
        frame.render_widget(list, area);
    } else {
        let list = List::new(items).block(block);

        // Use ListState for selection highlight
        let mut state = ListState::default();
        state.select(Some(app.listing.selected_index));

        frame.render_stateful_widget(list, area, &mut state);
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
