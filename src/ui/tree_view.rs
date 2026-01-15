//! Tree view renderer - displays hierarchical object listing.

use chrono::{DateTime, Utc};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem};

use crate::app::App;

pub fn render_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    // Calculate visible height (minus borders)
    let visible_height = area.height.saturating_sub(2) as usize;

    // Ensure selection is visible
    app.ensure_visible(visible_height);

    // Build title with bucket name and loading indicator
    let mut title = app
        .context
        .as_ref()
        .map(|c| c.display_path())
        .unwrap_or_else(|| "No Context".to_string());
    if app.tree.any_loading() {
        title = format!(" {} {} ", app.spinner_char(), title);
    } else {
        title = format!(" {} ", title);
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    // Build list items from visible tree nodes
    let visible_nodes = app.tree.visible_nodes();

    // Calculate available width for the content area (minus borders)
    let content_width = area.width.saturating_sub(2) as usize;

    let items: Vec<ListItem> = visible_nodes
        .iter()
        .enumerate()
        .skip(app.scroll_offset)
        .take(visible_height)
        .map(|(idx, (key, node))| {
            let is_selected = idx == app.tree.selected_index;
            let is_loading = app.tree.is_loading(key);

            // Build the line with right-aligned size and timestamp
            let mut spans = Vec::new();

            // Tree prefix (│ ├─ └─ etc.)
            let tree_prefix = app.tree.get_tree_prefix(key);
            let prefix_len = tree_prefix.chars().count();
            spans.push(Span::styled(
                tree_prefix,
                Style::default().fg(Color::DarkGray),
            ));

            // Expand/collapse indicator for directories
            let indicator = if node.is_dir {
                if is_loading {
                    format!("{} ", app.spinner_char())
                } else if app.tree.is_expanded(key) {
                    "▾ ".to_string()
                } else {
                    "▸ ".to_string()
                }
            } else {
                "─ ".to_string()
            };
            let indicator_len = indicator.chars().count();
            spans.push(Span::styled(
                indicator,
                Style::default().fg(Color::DarkGray),
            ));

            // Name - directories in blue, files in white
            let name_style = if node.is_dir {
                Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            let name_len = node.info.name.chars().count();
            spans.push(Span::styled(node.info.name.clone(), name_style));

            // Calculate size and timestamp strings
            let size_str = if let Some(size) = node.info.size {
                format_size(size)
            } else {
                String::new()
            };

            let timestamp_str = if !node.is_dir {
                if let Some(ref last_modified) = node.info.last_modified {
                    format_timestamp(last_modified)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Define column widths
            const SIZE_COL_WIDTH: usize = 10;
            const TIMESTAMP_COL_WIDTH: usize = 8;
            const MIN_SPACING: usize = 2;

            // Calculate total width used by name and prefix
            let name_and_prefix_len = prefix_len + indicator_len + name_len;

            // Calculate padding needed to right-align size and timestamp
            let metadata_len = if !size_str.is_empty() || !timestamp_str.is_empty() {
                let size_len = if !size_str.is_empty() {
                    SIZE_COL_WIDTH
                } else {
                    0
                };
                let timestamp_len = if !timestamp_str.is_empty() {
                    TIMESTAMP_COL_WIDTH
                } else {
                    0
                };
                let spacing = if !size_str.is_empty() && !timestamp_str.is_empty() {
                    MIN_SPACING
                } else {
                    0
                };
                size_len + spacing + timestamp_len + MIN_SPACING
            } else {
                0
            };

            if content_width > name_and_prefix_len + metadata_len {
                let padding = content_width - name_and_prefix_len - metadata_len;
                spans.push(Span::raw(" ".repeat(padding)));

                // Add right-aligned size
                if !size_str.is_empty() {
                    let size_padding = SIZE_COL_WIDTH.saturating_sub(size_str.len());
                    spans.push(Span::styled(
                        format!("{}{}", " ".repeat(size_padding), size_str),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                // Add timestamp
                if !timestamp_str.is_empty() {
                    if !size_str.is_empty() {
                        spans.push(Span::raw("  "));
                    }
                    spans.push(Span::styled(
                        timestamp_str,
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            }

            let line = Line::from(spans);
            let item = ListItem::new(line);

            if is_selected {
                item.style(Style::default().bg(Color::DarkGray))
            } else {
                item
            }
        })
        .collect();

    // Show empty state or "load more" hints
    let items = if items.is_empty() && !app.tree.any_loading() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "  (empty)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )]))]
    } else {
        // Check if we need to add "load more" hints for expanded directories
        add_load_more_hints(app, items, app.scroll_offset, visible_height)
    };

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Add "load more" hints after expanded directories that have more items
fn add_load_more_hints(
    app: &App,
    items: Vec<ListItem<'static>>,
    scroll_offset: usize,
    _visible_height: usize,
) -> Vec<ListItem<'static>> {
    let visible_nodes = app.tree.visible_nodes();

    if visible_nodes.is_empty() {
        return items;
    }

    let mut result = Vec::with_capacity(items.len() + 10);

    for (idx, item) in items.into_iter().enumerate() {
        result.push(item);

        // Calculate the actual index in the full visible_nodes list
        let visible_idx = idx + scroll_offset;

        if visible_idx >= visible_nodes.len() {
            continue;
        }

        let current_node = &visible_nodes[visible_idx];
        let current_parent = &current_node.1.parent_key;

        // Check if the next node is still a descendant of the current parent
        // We only want to show "load more" after ALL descendants of the parent are shown
        let is_last_descendant_of_parent = if visible_idx + 1 < visible_nodes.len() {
            let next_key = &visible_nodes[visible_idx + 1].0;
            // Next node is NOT a descendant if its key doesn't start with the parent prefix
            !next_key.starts_with(current_parent.as_str())
        } else {
            // Last item overall
            true
        };

        // If this is the last descendant and parent has more children, add hint
        if is_last_descendant_of_parent
            && !current_parent.is_empty()
            && let Some(parent_node) = app.tree.nodes.get(current_parent)
            && parent_node.has_more_children
        {
            let is_loading = app.tree.is_loading(current_parent);
            let spinner = app.spinner_char();
            let hint = create_load_more_hint(
                parent_node.depth + 1,
                parent_node.child_count,
                is_loading,
                spinner,
            );
            result.push(hint);
        }
    }

    result
}

fn create_load_more_hint(
    depth: usize,
    child_count: Option<usize>,
    is_loading: bool,
    spinner: char,
) -> ListItem<'static> {
    // Create indentation matching the tree depth
    let tree_prefix = if depth > 0 {
        format!("{}   ", "   ".repeat(depth - 1))
    } else {
        String::new()
    };

    let count_text = if is_loading {
        let count_info = child_count
            .map(|c| format!(" ({} loaded)", c))
            .unwrap_or_default();
        format!("{}└─ {} loading more...", tree_prefix, spinner) + &count_info
    } else if let Some(count) = child_count {
        format!("{}└─ ⋯ {} shown, more available", tree_prefix, count)
    } else {
        format!("{}└─ ⋯ more available", tree_prefix)
    };

    ListItem::new(Line::from(Span::styled(
        count_text,
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )))
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

/// Format timestamp into abbreviated form like "Jan 12" or "2d ago"
fn format_timestamp(timestamp_str: &str) -> String {
    // Try to parse the ISO 8601 timestamp from AWS
    if let Ok(dt) = timestamp_str.parse::<DateTime<Utc>>() {
        let now = Utc::now();
        let duration = now.signed_duration_since(dt);

        // If within the last 7 days, show relative time
        if duration.num_days() < 7 && duration.num_days() >= 0 {
            if duration.num_days() == 0 {
                if duration.num_hours() == 0 {
                    if duration.num_minutes() == 0 {
                        return "now".to_string();
                    }
                    return format!("{}m ago", duration.num_minutes());
                }
                return format!("{}h ago", duration.num_hours());
            }
            return format!("{}d ago", duration.num_days());
        }

        // Otherwise show abbreviated date
        dt.format("%b %d").to_string()
    } else {
        // Fallback if parsing fails
        String::new()
    }
}
