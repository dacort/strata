//! s3sh - A context-aware terminal UI for exploring object stores.
//!
//! This is not a shell. It's a view-first exploration tool where safety
//! is a feature, not a limitation.

mod app;
mod event;
mod mock_provider;
mod provider;
mod ui;

use std::io::{self, stdout};

use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;

use app::{App, StatusMessage};
use event::{spawn_event_reader, AppEvent};
use mock_provider::MockProvider;
use provider::{Provider, ProviderContext};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize error handling
    color_eyre::install().ok();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the app
    let result = run_app(&mut terminal).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    // Create provider and context
    let provider = MockProvider::new();
    let context = ProviderContext {
        provider_name: "s3".to_string(),
        root: "my-demo-bucket".to_string(),
        current_prefix: String::new(),
    };

    let mut app = App::new(context);

    // Set up event channel
    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);

    // Spawn event reader
    spawn_event_reader(tx.clone());

    // Initial listing load
    app.listing.is_loading = true;
    spawn_list_task(provider.clone(), app.context.current_prefix.clone(), tx.clone());

    // Main loop
    loop {
        // Clear expired status messages
        app.clear_expired_status();

        // Render
        terminal.draw(|f| ui::render(f, &app))?;

        // Handle events
        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    let needs_refresh = event::handle_key(&mut app, key);

                    // If navigation changed, trigger a new listing
                    if needs_refresh && app.listing.is_loading {
                        spawn_list_task(
                            provider.clone(),
                            app.context.current_prefix.clone(),
                            tx.clone(),
                        );
                    }
                }
                AppEvent::Tick => {
                    app.tick_spinner();
                }
                AppEvent::ListingLoaded(objects, continuation_token, is_truncated) => {
                    app.listing.objects = objects;
                    app.listing.continuation_token = continuation_token;
                    app.listing.has_more = is_truncated;
                    app.listing.is_loading = false;
                    app.listing.selected_index = 0;
                }
                AppEvent::ListingError(err) => {
                    app.listing.is_loading = false;
                    app.set_status(StatusMessage::error(err));
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn spawn_list_task<P: Provider + Clone>(
    provider: P,
    prefix: String,
    tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        match provider.list(&prefix, None, 1000).await {
            Ok(result) => {
                let _ = tx
                    .send(AppEvent::ListingLoaded(
                        result.objects,
                        result.continuation_token,
                        result.is_truncated,
                    ))
                    .await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::ListingError(e.to_string())).await;
            }
        }
    });
}
