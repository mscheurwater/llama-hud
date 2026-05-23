//! llama-hud — btop-style terminal dashboard for llama-server.

mod app;
mod config;
mod parser;
mod prometheus;
mod slots_poller;
mod theme;
mod widgets;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use app::App;
use config::load_config;

#[derive(Parser, Debug)]
#[command(name = "llama-hud", about = "btop-style dashboard for llama-server")]
struct Cli {
    /// Override server URL
    #[arg(long)]
    url: Option<String>,

    /// Override tmux session for log tailing
    #[arg(long)]
    tmux_session: Option<String>,
}

fn setup_terminal() -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    Ok(())
}

fn cleanup_terminal() {
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
}

fn merge_config(cli: &Cli) -> config::AppConfig {
    let mut cfg = load_config();

    if let Some(ref url) = cli.url {
        cfg.url = url.clone();
    }
    if let Some(ref session) = cli.tmux_session {
        cfg.tmux_session = Some(session.clone());
    }

    cfg
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let cfg = merge_config(&cli);

    if setup_terminal().is_err() {
        eprintln!("Failed to setup terminal");
        std::process::exit(1);
    }

    // Panic hook — restore terminal on crash
    let panic_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup_terminal();
        panic_hook(info);
    }));

    // Graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();
    let _ = ctrlc::set_handler(move || {
        running_clone.store(false, Ordering::SeqCst);
    });

    let result = run(&cfg, running.clone()).await;
    cleanup_terminal();
    running.store(false, Ordering::SeqCst);

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(
    cfg: &config::AppConfig,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cfg.clone());

    let base_url = cfg.base_url().to_string();

    // Slots poller
    let mut slots_rx = slots_poller::start_slots_poller(base_url.clone(), cfg.slots_poll_ms);

    // Prometheus poller
    let mut prom_rx = prometheus::start_prometheus_poller(base_url.clone(), cfg.metrics_poll_ms);

    // Model name fetch — run once in background, set result via channel
    let (model_tx, mut model_rx) = tokio::sync::mpsc::channel(1);
    {
        let url = base_url.clone();
        tokio::spawn(async move {
            let client = reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default();
            let name = slots_poller::fetch_model_name(&url, &client).await;
            let _ = model_tx.send(name).await;
        });
    }

    // Render loop at 100ms
    let tick_rate = Duration::from_millis(100);

    loop {
        if !running.load(Ordering::SeqCst) || !app.running {
            break;
        }

        // Drain key events
        while let Ok(true) = crossterm::event::poll(Duration::ZERO) {
            if let Ok(Event::Key(key)) = event::read() {
                if key.code == KeyCode::Esc {
                    app.selected_slot = None;
                    for slot in app.slots.values_mut() {
                        slot.selected = false;
                    }
                } else {
                    app.handle_key(key.code);
                }
            }
        }

        // Process slot updates (non-blocking)
        while let Ok(snapshots) = slots_rx.try_recv() {
            for snapshot in snapshots {
                app.update_slot(snapshot);
            }
            // First successful poll = server is running
            if app.server_state == app::ServerState::Unknown {
                app.server_state = app::ServerState::Running;
                app.start_time = Some(Instant::now());
            }
        }

        // Process Prometheus updates (non-blocking)
        while let Ok(metrics) = prom_rx.try_recv() {
            app.update_prometheus(metrics);
        }

        // Check for model name result (non-blocking)
        if app.model_name.is_empty()
            && let Ok(name) = model_rx.try_recv()
            && !name.is_empty()
        {
            app.model_name = name;
        }

        // Render
        terminal.draw(|f| widgets::render(f, &app))?;
        tokio::time::sleep(tick_rate).await;
    }

    Ok(())
}
