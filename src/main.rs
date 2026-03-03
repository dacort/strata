//! strata - A context-aware terminal UI for exploring object stores.
//!
//! This is not a shell. It's a view-first exploration tool where safety
//! is a feature, not a limitation.

mod app;
mod event;
mod mock_provider;
mod preview;
mod provider;
mod registry;
mod s3_provider;
mod tree;
mod ui;

use std::io::{self, stdout};

use clap::Parser;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use tokio::sync::mpsc;
use tracing::info;

use app::{App, StatusMessage};
use event::{AppEvent, KeyResult, spawn_event_reader};
use mock_provider::MockProvider;
use preview::PreviewMode;
use provider::{Provider, ProviderContext};
use registry::{ParsedUri, get_available_providers, parse_uri};
use s3_provider::S3Provider;

/// A context-aware terminal UI for exploring object stores
#[derive(Parser, Debug)]
#[command(name = "strata", version, about)]
struct Cli {
    /// URI to open (e.g., s3://bucket-name)
    uri: Option<String>,

    /// Use mock/dev provider
    #[arg(long)]
    dev: bool,

    /// Log level (e.g., debug, info, warn, error). Enables logging when set.
    #[arg(short = 'l', long = "log-level")]
    log_level: Option<String>,

    /// Log file path
    #[arg(long = "log-file", default_value = "./data-shell.log")]
    log_file: String,
}

/// Set up file-based logging with tracing
fn setup_logging(log_file: &str, level: &str) {
    use std::path::Path;
    use tracing_subscriber::EnvFilter;

    let path = Path::new(log_file);
    let dir = path.parent().unwrap_or(Path::new("."));
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("data-shell.log");

    let file_appender = tracing_appender::rolling::never(dir, filename);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_writer(file_appender)
        .with_ansi(false)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize error handling
    color_eyre::install().ok();

    // Parse command line arguments
    let cli = Cli::parse();

    // Set up logging if -l is specified
    if let Some(ref level) = cli.log_level {
        setup_logging(&cli.log_file, level);
        info!(
            "Logging initialized at level={} to file={}",
            level, cli.log_file
        );
    }

    let use_dev_mode = cli.dev;
    let uri = cli.uri;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    info!(
        "Starting data-shell (dev_mode={}, uri={:?})",
        use_dev_mode, uri
    );

    // Run the app based on arguments
    let result = if use_dev_mode {
        // --dev flag: use mock provider
        run_app_with_mock(&mut terminal).await
    } else if let Some(uri_str) = uri {
        // URI provided: parse and run with appropriate provider
        run_app_with_uri(&mut terminal, uri_str).await
    } else {
        // No args: show provider selector
        run_app_with_selector(&mut terminal).await
    };

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// Run app with a parsed URI (e.g., s3://bucket-name)
async fn run_app_with_uri(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    uri_str: String,
) -> anyhow::Result<()> {
    match parse_uri(&uri_str) {
        Some(ParsedUri::S3 { bucket }) => run_app_with_s3(terminal, bucket).await,
        Some(ParsedUri::HuggingFace { .. }) => {
            anyhow::bail!("HuggingFace provider not yet implemented")
        }
        None => {
            anyhow::bail!("Invalid URI format. Expected s3://bucket-name or hf://datasets/path")
        }
    }
}

/// Run app starting with provider selector
async fn run_app_with_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> anyhow::Result<()> {
    let providers = get_available_providers();
    let mut app = App::new_with_provider_selector(providers);

    // Set up event channel
    let (tx, mut rx) = mpsc::channel::<AppEvent>(100);

    // Spawn event reader
    spawn_event_reader(tx.clone());

    // Main loop - handle provider and resource selection, then transition to browse mode
    loop {
        // Clear expired status messages
        app.clear_expired_status();

        // Render
        terminal.draw(|f| ui::render(f, &mut app))?;

        // Handle events
        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    match event::handle_key(&mut app, key) {
                        KeyResult::ProviderSelected(provider_id) => {
                            // Transition to resource selector
                            app.enter_resource_selector(provider_id.clone());

                            // Load resources for selected provider
                            if provider_id == "s3" {
                                spawn_s3_contexts_task(tx.clone());
                            }
                        }
                        KeyResult::SwitchContext(resource_name) => {
                            // User selected a resource, transition to browse mode
                            if let Some(provider_id) = &app.selected_provider_id
                                && provider_id == "s3"
                            {
                                // Initialize S3 provider and switch to browse mode
                                match S3Provider::new(&resource_name).await {
                                    Ok(provider) => {
                                        let context = ProviderContext {
                                            provider_name: "s3".to_string(),
                                            root: resource_name.clone(),
                                            current_prefix: String::new(),
                                        };
                                        app.enter_browse_mode(context);

                                        // Initial root load
                                        app.tree.set_loading("", true);
                                        spawn_list_task(provider, String::new(), None, tx.clone());
                                    }
                                    Err(e) => {
                                        app.set_status(StatusMessage::error(format!(
                                            "Failed to connect to S3: {}",
                                            e
                                        )));
                                    }
                                }
                            }
                        }
                        KeyResult::LoadContexts => {
                            // In browse mode, reload contexts
                            if let Some(context) = &app.context
                                && context.provider_name == "s3"
                            {
                                spawn_s3_contexts_task(tx.clone());
                            }
                        }
                        _ => {}
                    }
                }
                AppEvent::Tick => {
                    app.tick_spinner();
                }
                AppEvent::ContextsLoaded(contexts) => {
                    app.contexts = contexts;
                }
                AppEvent::LoadError(_prefix, err) => {
                    app.set_status(StatusMessage::error(err));
                }
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }

        // If we transitioned to browse mode with a provider, hand off to provider-specific loop
        if matches!(app.mode, app::AppMode::Browse)
            && let Some(context) = app.context.clone()
        {
            if context.provider_name == "s3" {
                // Get provider - need to recreate it
                if let Ok(provider) = S3Provider::new(&context.root).await {
                    return run_app_loop(terminal, provider, app, tx, rx).await;
                }
            } else if context.provider_name == "mock" {
                let provider = MockProvider::new();
                return run_app_loop(terminal, provider, app, tx, rx).await;
            }
        }
    }

    Ok(())
}

async fn run_app_with_s3(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bucket: String,
) -> anyhow::Result<()> {
    // Create S3 provider with default AWS credentials
    let provider = S3Provider::new(&bucket).await?;
    let context = ProviderContext {
        provider_name: "s3".to_string(),
        root: bucket,
        current_prefix: String::new(),
    };

    let mut app = App::new(context);

    // Set up event channel
    let (tx, rx) = mpsc::channel::<AppEvent>(100);

    // Spawn event reader
    spawn_event_reader(tx.clone());

    // Initial root load
    app.tree.set_loading("", true);
    spawn_list_task(provider.clone(), String::new(), None, tx.clone());

    run_app_loop(terminal, provider, app, tx, rx).await
}

async fn run_app_with_mock(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> anyhow::Result<()> {
    // Create mock provider
    let provider = MockProvider::new();
    let context = ProviderContext {
        provider_name: "mock".to_string(),
        root: "demo-bucket".to_string(),
        current_prefix: String::new(),
    };

    let mut app = App::new(context);

    // Set up event channel
    let (tx, rx) = mpsc::channel::<AppEvent>(100);

    // Spawn event reader
    spawn_event_reader(tx.clone());

    // Initial root load
    app.tree.set_loading("", true);
    spawn_list_task(provider.clone(), String::new(), None, tx.clone());

    run_app_loop(terminal, provider, app, tx, rx).await
}

async fn run_app_loop<P: Provider + Clone>(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut provider: P,
    mut app: App,
    tx: mpsc::Sender<AppEvent>,
    mut rx: mpsc::Receiver<AppEvent>,
) -> anyhow::Result<()> {
    // Main loop
    loop {
        // Clear expired status messages
        app.clear_expired_status();

        // Render
        terminal.draw(|f| ui::render(f, &mut app))?;

        // Handle events
        if let Some(event) = rx.recv().await {
            match event {
                AppEvent::Key(key) => {
                    match event::handle_key(&mut app, key) {
                        KeyResult::LoadChildren(prefix) => {
                            spawn_list_task(
                                provider.clone(),
                                prefix.clone(),
                                Some(prefix),
                                tx.clone(),
                            );
                        }
                        KeyResult::Refresh => {
                            // Clear and reload root
                            app.tree = tree::TreeState::new();
                            app.tree.set_loading("", true);
                            spawn_list_task(provider.clone(), String::new(), None, tx.clone());
                        }
                        KeyResult::LoadContexts => {
                            spawn_contexts_task(provider.clone(), tx.clone());
                        }
                        KeyResult::SwitchContext(new_context) => {
                            // For S3Provider, we need to create a new provider with the new bucket
                            // This is a bit of a hack, but it works for now
                            if let Some(ref context) = app.context {
                                if context.provider_name == "s3" {
                                    // Switch bucket for S3 (async to detect bucket region)
                                    provider = switch_s3_bucket(provider, &new_context).await;
                                }
                                // Update context and reload
                                let mut new_context_obj = context.clone();
                                new_context_obj.root = new_context;
                                new_context_obj.current_prefix = String::new();
                                app.context = Some(new_context_obj);
                                app.tree = tree::TreeState::new();
                                app.tree.set_loading("", true);
                                app.set_status(StatusMessage::info(format!(
                                    "Switched to {}",
                                    app.context.as_ref().unwrap().root
                                )));
                                spawn_list_task(provider.clone(), String::new(), None, tx.clone());
                            }
                        }
                        KeyResult::FetchPreviewHead(key, bytes) => {
                            spawn_preview_task(
                                provider.clone(),
                                key,
                                0,
                                bytes.saturating_sub(1),
                                PreviewMode::Head,
                                tx.clone(),
                            );
                        }
                        KeyResult::FetchPreviewTail(key, total_size, bytes) => {
                            let start = total_size.saturating_sub(bytes);
                            let end = total_size.saturating_sub(1);
                            spawn_preview_task(
                                provider.clone(),
                                key,
                                start,
                                end,
                                PreviewMode::Tail,
                                tx.clone(),
                            );
                        }
                        KeyResult::OpenInPager(key) => {
                            // Suspend TUI and open pager
                            if let Err(e) = open_in_pager(&provider, &key, terminal).await {
                                app.set_status(StatusMessage::error(format!("Pager error: {}", e)));
                            }
                            app.close_file_preview();
                        }
                        KeyResult::SaveToLocal(remote_key, local_path) => {
                            spawn_download_task(
                                provider.clone(),
                                remote_key,
                                local_path,
                                tx.clone(),
                            );
                        }
                        KeyResult::LoadMore(parent_key) => {
                            // Get the continuation token from the parent node
                            if let Some(token) = app.tree.get_continuation_token(&parent_key) {
                                // Get the prefix from the parent key
                                let prefix = parent_key.clone();
                                app.tree.set_loading(&parent_key, true);
                                spawn_load_more_task(
                                    provider.clone(),
                                    prefix,
                                    parent_key,
                                    token,
                                    tx.clone(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                AppEvent::Tick => {
                    app.tick_spinner();
                }
                AppEvent::RootLoaded(objects, has_more) => {
                    app.tree.set_loading("", false);
                    app.tree.set_root(objects, has_more);
                }
                AppEvent::ChildrenLoaded {
                    parent_key,
                    objects,
                    has_more,
                    continuation_token,
                } => {
                    app.tree.set_loading(&parent_key, false);
                    app.tree
                        .set_children(&parent_key, objects, has_more, continuation_token);
                }
                AppEvent::MoreChildrenLoaded {
                    parent_key,
                    objects,
                    has_more,
                    continuation_token,
                } => {
                    app.tree.set_loading(&parent_key, false);
                    app.tree
                        .append_children(&parent_key, objects, has_more, continuation_token);
                }
                AppEvent::LoadError(prefix, err) => {
                    app.tree.set_loading(&prefix, false);
                    app.set_status(StatusMessage::error(err));
                }
                AppEvent::ContextsLoaded(contexts) => {
                    app.contexts = contexts;
                }
                AppEvent::PreviewLoaded { key, content, mode } => {
                    if let Some(ref mut preview) = app.file_preview
                        && preview.key == key
                    {
                        preview.update_content(content, mode);
                    }
                }
                AppEvent::PagerExited => {
                    // TUI already restored, nothing to do
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

// Helper function to switch S3 bucket
// For S3, we use the with_bucket method. For other providers, this is a no-op.
// This works via trait specialization pattern using std::any::Any
async fn switch_s3_bucket<P: Provider + Clone + 'static>(provider: P, new_bucket: &str) -> P {
    use std::any::Any;

    // Try to downcast to S3Provider
    let provider_any: &dyn Any = &provider;
    if let Some(s3_provider) = provider_any.downcast_ref::<S3Provider>() {
        // We have an S3Provider, create a new one with the new bucket
        // This is async because it needs to detect the bucket's region
        match s3_provider.with_bucket(new_bucket).await {
            Ok(new_s3) => {
                // Convert S3Provider back to P
                // This is safe because when we successfully downcast &provider to &S3Provider,
                // it means P is S3Provider at the call site
                let boxed: Box<dyn Any> = Box::new(new_s3);
                if let Ok(result) = boxed.downcast::<P>() {
                    return *result;
                }
            }
            Err(e) => {
                // Log error but continue with original provider
                eprintln!("Failed to switch bucket: {}", e);
            }
        }
    }

    // For other providers or if downcast fails, return the original provider
    provider
}

fn spawn_list_task<P: Provider + Clone>(
    provider: P,
    prefix: String,
    parent_key: Option<String>, // None = root, Some = children of this key
    tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        match provider.list(&prefix, None, 1000).await {
            Ok(result) => {
                let event = if let Some(key) = parent_key {
                    AppEvent::ChildrenLoaded {
                        parent_key: key,
                        objects: result.objects,
                        has_more: result.is_truncated,
                        continuation_token: result.continuation_token,
                    }
                } else {
                    AppEvent::RootLoaded(result.objects, result.is_truncated)
                };
                let _ = tx.send(event).await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::LoadError(prefix, e.to_string())).await;
            }
        }
    });
}

fn spawn_load_more_task<P: Provider + Clone>(
    provider: P,
    prefix: String,
    parent_key: String,
    continuation_token: String,
    tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        match provider
            .list(&prefix, Some(&continuation_token), 1000)
            .await
        {
            Ok(result) => {
                let _ = tx
                    .send(AppEvent::MoreChildrenLoaded {
                        parent_key,
                        objects: result.objects,
                        has_more: result.is_truncated,
                        continuation_token: result.continuation_token,
                    })
                    .await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::LoadError(prefix, e.to_string())).await;
            }
        }
    });
}

fn spawn_contexts_task<P: Provider + Clone>(provider: P, tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        match provider.list_contexts().await {
            Ok(contexts) => {
                let _ = tx.send(AppEvent::ContextsLoaded(contexts)).await;
            }
            Err(e) => {
                let _ = tx
                    .send(AppEvent::LoadError(
                        "contexts".to_string(),
                        format!("Failed to load contexts: {}", e),
                    ))
                    .await;
            }
        }
    });
}

fn spawn_s3_contexts_task(tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        info!("Loading S3 bucket list");
        // Create an S3 provider without targeting a specific bucket
        match S3Provider::new_default().await {
            Ok(provider) => match provider.list_contexts().await {
                Ok(contexts) => {
                    info!(count = contexts.len(), "S3 buckets loaded");
                    let _ = tx.send(AppEvent::ContextsLoaded(contexts)).await;
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to load S3 buckets");
                    let _ = tx
                        .send(AppEvent::LoadError(
                            "contexts".to_string(),
                            format!("Failed to load S3 buckets: {}", e),
                        ))
                        .await;
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "Failed to initialize S3 client");
                let _ = tx
                    .send(AppEvent::LoadError(
                        "contexts".to_string(),
                        format!("Failed to initialize S3 client: {}", e),
                    ))
                    .await;
            }
        }
    });
}

fn spawn_preview_task<P: Provider + Clone>(
    provider: P,
    key: String,
    start: u64,
    end: u64,
    mode: PreviewMode,
    tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        match provider.get_range(&key, start, end).await {
            Ok(data) => {
                let _ = tx
                    .send(AppEvent::PreviewLoaded {
                        key,
                        content: data,
                        mode,
                    })
                    .await;
            }
            Err(e) => {
                let _ = tx
                    .send(AppEvent::LoadError(key, format!("Preview error: {}", e)))
                    .await;
            }
        }
    });
}

async fn open_in_pager<P: Provider>(
    provider: &P,
    key: &str,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> anyhow::Result<()> {
    use std::process::{Command, Stdio};

    // 1. Exit alternate screen to restore normal terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    // 2. Get pager command from environment
    let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -FRSX".to_string());
    let pager_parts: Vec<&str> = pager.split_whitespace().collect();
    let (pager_cmd, pager_args) = pager_parts
        .split_first()
        .map(|(cmd, args)| (*cmd, args.to_vec()))
        .unwrap_or(("less", vec!["-FRSX"]));

    // 3. Fetch content and pipe to pager
    // Note: For very large files, we might want chunked streaming
    // For now, use get_range with a reasonable limit
    let result = provider.get_range(key, 0, 10 * 1024 * 1024).await; // 10MB limit

    match result {
        Ok(data) => {
            let mut child = Command::new(pager_cmd)
                .args(&pager_args)
                .stdin(Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                let _ = stdin.write_all(&data);
            }
            let _ = child.wait();
        }
        Err(e) => {
            eprintln!("Error fetching file: {}", e);
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    // 4. Restore TUI
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;

    Ok(())
}

fn spawn_download_task<P: Provider + Clone>(
    provider: P,
    key: String,
    local_path: String,
    tx: mpsc::Sender<AppEvent>,
) {
    tokio::spawn(async move {
        use tokio::fs::File;
        use tokio::io::AsyncWriteExt;

        // Get file size first via head
        let total_bytes = match provider.head(&key).await {
            Ok(info) => info.size.unwrap_or(0),
            Err(e) => {
                let _ = tx
                    .send(AppEvent::LoadError(
                        key,
                        format!("Failed to get file info: {}", e),
                    ))
                    .await;
                return;
            }
        };

        // Create local file
        let file = match File::create(&local_path).await {
            Ok(f) => f,
            Err(e) => {
                let _ = tx
                    .send(AppEvent::LoadError(
                        key,
                        format!("Failed to create file: {}", e),
                    ))
                    .await;
                return;
            }
        };
        let mut file = file;

        // Stream in chunks
        const CHUNK_SIZE: u64 = 1024 * 1024; // 1MB chunks
        let mut bytes_downloaded: u64 = 0;

        while bytes_downloaded < total_bytes {
            let end = (bytes_downloaded + CHUNK_SIZE - 1).min(total_bytes - 1);

            match provider.get_range(&key, bytes_downloaded, end).await {
                Ok(chunk) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        let _ = tx
                            .send(AppEvent::LoadError(key, format!("Failed to write: {}", e)))
                            .await;
                        return;
                    }
                    bytes_downloaded += chunk.len() as u64;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::LoadError(key, format!("Download error: {}", e)))
                        .await;
                    return;
                }
            }
        }

        // Success - report via status message (we don't have a dedicated event for this)
        // The caller should show a status message
    });
}
