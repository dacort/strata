//! Event handling - keyboard input and async events.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::app::{App, Focus, StatusMessage};
use crate::provider::ObjectType;

/// Application events
#[derive(Debug)]
pub enum AppEvent {
    /// Keyboard input
    Key(KeyEvent),
    /// Tick for animations/timeouts
    Tick,
    /// Listing loaded
    ListingLoaded(Vec<crate::provider::ObjectInfo>, Option<String>, bool),
    /// Listing error
    ListingError(String),
}

/// Spawn a task to read keyboard events
pub fn spawn_event_reader(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        loop {
            // Poll for events with timeout for tick
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if tx.send(AppEvent::Key(key)).await.is_err() {
                        break;
                    }
                }
            } else {
                // Send tick event
                if tx.send(AppEvent::Tick).await.is_err() {
                    break;
                }
            }
        }
    });
}

/// Handle a key event, returning true if the event was consumed
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    // Help overlay captures all input
    if app.show_help {
        app.show_help = false;
        return true;
    }

    // Global keybindings
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.quit();
            return true;
        }
        KeyCode::Char('?') => {
            app.toggle_help();
            return true;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.quit();
            return true;
        }
        KeyCode::Tab => {
            app.focus = match app.focus {
                Focus::Navigator => Focus::Preview,
                Focus::Preview => Focus::Navigator,
            };
            return true;
        }
        _ => {}
    }

    // Focus-specific handling
    match app.focus {
        Focus::Navigator => handle_navigator_key(app, key),
        Focus::Preview => handle_preview_key(app, key),
    }
}

fn handle_navigator_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.listing.select_prev();
            true
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.listing.select_next();
            true
        }
        KeyCode::Char('g') => {
            app.listing.select_first();
            true
        }
        KeyCode::Char('G') => {
            app.listing.select_last();
            true
        }
        KeyCode::Enter => {
            if let Some(obj) = app.listing.selected().cloned() {
                match obj.object_type {
                    ObjectType::Prefix => {
                        app.navigate_to(obj.key.clone());
                        app.set_status(StatusMessage::info(format!("Navigating to {}", obj.name)));
                    }
                    _ => {
                        app.set_status(StatusMessage::info(format!("Opening {}", obj.name)));
                        // TODO: Open preview/inspector based on type
                    }
                }
            }
            true
        }
        KeyCode::Backspace => {
            if app.navigate_back() {
                app.set_status(StatusMessage::info("Navigating back"));
            } else if !app.context.current_prefix.is_empty() {
                // Go up one level
                let parts: Vec<&str> = app.context.current_prefix.trim_end_matches('/').split('/').collect();
                if parts.len() > 1 {
                    let parent = parts[..parts.len() - 1].join("/") + "/";
                    app.navigate_to(parent);
                } else {
                    app.navigate_to(String::new());
                }
            }
            true
        }
        KeyCode::Char('r') => {
            app.listing.is_loading = true;
            app.set_status(StatusMessage::info("Refreshing..."));
            true
        }
        _ => false,
    }
}

fn handle_preview_key(app: &mut App, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            // TODO: Scroll preview up
            true
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // TODO: Scroll preview down
            true
        }
        _ => false,
    }
}
