//! llama-hud — btop-style terminal dashboard for llama-server.

mod app;
mod config;
mod log_tailer;
mod parser;
mod prometheus;
mod slots_poller;
mod theme;
mod widgets;

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event};
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

    /// Show version and exit
    #[arg(long)]
    version: bool,
}

fn setup_terminal() -> std::io::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    )?;
    Ok(())
}

fn cleanup_terminal() {
    let mut stdout = std::io::stdout();
    let _ = crossterm::terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        stdout,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::cursor::Show,
    );
    let _ = stdout.flush();
}

fn merge_config(cli: &Cli) -> (config::AppConfig, bool, Option<String>) {
    let (mut cfg, config_exists, cfg_error) = load_config();

    if let Some(ref url) = cli.url {
        cfg.url = url.clone();
    }
    if let Some(ref session) = cli.tmux_session {
        cfg.tmux_session = Some(session.clone());
    }

    (cfg, config_exists, cfg_error)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if cli.version {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let (cfg, config_exists, cfg_error) = merge_config(&cli);

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

    let result = run(&cfg, config_exists, cfg_error, running.clone()).await;
    running.store(false, Ordering::SeqCst);
    cleanup_terminal();

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

async fn run(
    cfg: &config::AppConfig,
    config_exists: bool,
    cfg_error: Option<String>,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(cfg.clone(), config_exists);
    if let Some(err) = cfg_error {
        app.error_message = Some(err);
    }

    // Shared URL — pollers read this, updated when config changes
    let shared_url = std::sync::Arc::new(std::sync::Mutex::new(cfg.base_url().to_string()));
    app.shared_url = Some(shared_url.clone());

    // Shared error channel
    let (error_tx, mut error_rx) = tokio::sync::mpsc::channel(8);

    // Poll state — track config values to detect changes and restart pollers
    let mut current_slots_poll_ms = cfg.slots_poll_ms;
    let mut current_metrics_poll_ms = cfg.metrics_poll_ms;
    let mut current_tmux_session = cfg.tmux_session.clone();

    // Slots poller (receiver, join_handle) — abort old handle on config change
    let (slots_rx, mut slots_handle) =
        slots_poller::start_slots_poller(cfg.slots_poll_ms, error_tx.clone(), shared_url.clone());
    let mut slots_rx = slots_rx;

    // Prometheus poller
    let (prom_rx, mut prom_handle) = prometheus::start_prometheus_poller(
        cfg.metrics_poll_ms,
        error_tx.clone(),
        shared_url.clone(),
    );
    let mut prom_rx = prom_rx;

    // Log tailer (optional tmux session)
    let mut logs_rx = cfg
        .tmux_session
        .as_ref()
        .map(|session| log_tailer::start_tailing(session.clone(), running.clone()));

    // Model name fetch — run once in background, set result via channel
    let (model_tx, mut model_rx) = tokio::sync::mpsc::channel(1);
    {
        let url = cfg.base_url().to_string();
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
    // Consecutive-success counter for error clearing (clear after 3 clean ticks)
    let mut consecutive_ok: u32 = 0;

    loop {
        if !running.load(Ordering::SeqCst) || !app.running {
            break;
        }

        // Drain key events
        while let Ok(true) = crossterm::event::poll(Duration::ZERO) {
            if let Ok(Event::Key(key)) = event::read() {
                app.handle_key(key.code, key);
            }
        }

        // Detect config changes and restart pollers if needed
        let config = &app.config;
        if config.slots_poll_ms != current_slots_poll_ms
            || config.metrics_poll_ms != current_metrics_poll_ms
        {
            current_slots_poll_ms = config.slots_poll_ms;
            current_metrics_poll_ms = config.metrics_poll_ms;
            slots_handle.abort();
            prom_handle.abort();
            let (new_slots_rx, new_slots_h) = slots_poller::start_slots_poller(
                config.slots_poll_ms,
                error_tx.clone(),
                shared_url.clone(),
            );
            slots_rx = new_slots_rx;
            slots_handle = new_slots_h;
            let (new_prom_rx, new_prom_h) = prometheus::start_prometheus_poller(
                config.metrics_poll_ms,
                error_tx.clone(),
                shared_url.clone(),
            );
            prom_rx = new_prom_rx;
            prom_handle = new_prom_h;
        }
        if config.tmux_session != current_tmux_session {
            current_tmux_session = config.tmux_session.clone();
            if let Some(ref mut rx) = logs_rx {
                rx.1.abort();
            }
            logs_rx = config
                .tmux_session
                .as_ref()
                .map(|session| log_tailer::start_tailing(session.clone(), running.clone()));
        }

        // Process slot updates (non-blocking)
        let mut slots_ok = false;
        while let Ok(snapshots) = slots_rx.try_recv() {
            app.update_slots(snapshots);
            slots_ok = true;
            // First successful poll = server is running
            if app.server_state == app::ServerState::Unknown {
                app.server_state = app::ServerState::Running;
                app.start_time = Some(Instant::now());
            }
        }

        // Process Prometheus updates (non-blocking)
        let mut prom_ok = false;
        while let Ok(metrics) = prom_rx.try_recv() {
            app.update_prometheus(metrics);
            prom_ok = true;
        }

        // Error handling: set on error, clear only after 3 consecutive ticks where both pollers succeed
        while let Ok(msg) = error_rx.try_recv() {
            if !msg.is_empty() {
                app.error_message = Some(msg);
                consecutive_ok = 0;
            }
        }
        if slots_ok && prom_ok {
            consecutive_ok += 1;
            if consecutive_ok >= 3 {
                app.error_message = None;
            }
        } else {
            consecutive_ok = 0;
        }

        // Process log lines (non-blocking)
        if let Some(ref mut rx) = logs_rx {
            while let Ok(line) = rx.0.try_recv() {
                // Check for expected prompt total
                if let Some(total) = parser::parse_prompt_expected_total(&line) {
                    // Apply to first active slot
                    for (_, slot) in app.slots.iter() {
                        if slot.phase == app::SlotPhase::Prompt {
                            app.apply_expected_total(slot.id, total);
                            break;
                        }
                    }
                }
                app.add_log(line);
            }
        }

        // Check for model name result (non-blocking)
        if app.model_name.is_empty()
            && let Ok(name) = model_rx.try_recv()
            && !name.is_empty()
        {
            app.model_name = name;
        }

        // Render
        app.frame_count += 1;
        terminal.draw(|f| widgets::render(f, &app))?;
        tokio::time::sleep(tick_rate).await;
    }

    Ok(())
}
