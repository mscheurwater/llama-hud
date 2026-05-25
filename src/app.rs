//! Application state — slots, metrics history, server info.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use crate::config::AppConfig;
use crate::prometheus::PrometheusMetrics;
use crate::slots_poller::{SlotParams, SlotSnapshot};
use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum ServerState {
    #[default]
    Unknown,
    Running,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum SlotPhase {
    #[default]
    Idle,
    Prompt,     // prompt processing (THINK)
    Generation, // token generation (WRITE)
    Done,       // task just completed
}

impl std::fmt::Display for SlotPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlotPhase::Idle => write!(f, "IDLE "),
            SlotPhase::Prompt => write!(f, "THINK"),
            SlotPhase::Generation => write!(f, "WRITE"),
            SlotPhase::Done => write!(f, "DONE "),
        }
    }
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct CompletedTask {
    pub n_prompt_tokens: u64,
    pub n_decoded: u64,
    pub duration: Duration,
    pub avg_prompt_tps: f64,
    pub avg_gen_tps: f64,
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub id: u32,
    pub phase: SlotPhase,
    pub progress: f64, // 0.0 to 1.0
    pub prompt_tps: f64,
    pub gen_tps: f64,
    // Windowed TPS: track cumulative delta and window start
    pub prompt_tps_window_start: Option<Instant>,
    pub prompt_tps_window_tokens: u64,
    pub gen_tps_window_start: Option<Instant>,
    pub gen_tps_window_tokens: u64,
    pub n_prompt_tokens: u64,           // current context size (loaded prompt)
    pub n_prompt_tokens_processed: u64,
    pub n_decoded: u64,
    pub max_tokens: u64,
    pub n_ctx: u64,
    pub expected_prompt_total: Option<u64>,
    pub n_prompt_tokens_cache: u64,
    pub id_task: i64,
    pub params: SlotParams,
    pub last_task: Option<CompletedTask>,
    pub current_task_start: Option<Instant>,
    pub selected: bool,
}

impl Default for Slot {
    fn default() -> Self {
        Self {
            id: 0,
            phase: SlotPhase::Idle,
            progress: 0.0,
            prompt_tps: 0.0,
            gen_tps: 0.0,
            prompt_tps_window_start: None,
            prompt_tps_window_tokens: 0,
            gen_tps_window_start: None,
            gen_tps_window_tokens: 0,
            n_prompt_tokens: 0,
            n_prompt_tokens_processed: 0,
            n_decoded: 0,
            max_tokens: 0,
            n_ctx: 0,
            expected_prompt_total: None,
            n_prompt_tokens_cache: 0,
            id_task: 0,
            params: SlotParams::default(),
            last_task: None,
            current_task_start: None,
            selected: false,
        }
    }
}

pub struct App {
    pub config: AppConfig,
    pub running: bool,
    pub shared_url: Option<std::sync::Arc<std::sync::Mutex<String>>>,
    // Toggle views independently
    pub show_stats: bool,
    pub show_slots: bool,
    pub show_slot_detail: bool,
    pub show_logs: bool,
    pub server_state: ServerState,
    pub error_message: Option<String>,
    pub model_name: String,
    pub start_time: Option<Instant>,
    pub last_metrics_time: Option<Instant>,

    // Prometheus cumulative
    pub prometheus: Option<PrometheusMetrics>,

    // Chart history
    pub prompt_tokens_total_history: VecDeque<u64>,
    pub predicted_tokens_total_history: VecDeque<u64>,
    pub prompt_tps_history: VecDeque<f64>,
    pub predicted_tps_history: VecDeque<f64>,

    // Slots
    pub slots: HashMap<u32, Slot>,
    pub selected_slot: Option<u32>,

    // Log buffer (for optional log view)
    pub logs: VecDeque<String>,
    // Dedup: (comparison key, display line, repeat_count)
    pub log_dedup: (String, String, usize),

    // Active theme
    pub theme: Theme,

    // Config popup
    pub show_config: bool,
    pub config_field: ConfigField,
    pub config_edits: ConfigEdits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigField {
    #[default]
    Url,
    TmuxSession,
    SlotsPollMs,
    MetricsPollMs,
    ChartHistory,
    Theme,
}

#[derive(Debug, Clone)]
pub struct ConfigEdits {
    pub url: String,
    pub tmux_session: String,
    pub slots_poll_ms: String,
    pub metrics_poll_ms: String,
    pub chart_history: String,
    pub theme: String,
}

impl ConfigEdits {
    pub fn from_config(cfg: &AppConfig) -> Self {
        Self {
            url: cfg.url.clone(),
            tmux_session: cfg.tmux_session.clone().unwrap_or_default(),
            slots_poll_ms: cfg.slots_poll_ms.to_string(),
            metrics_poll_ms: cfg.metrics_poll_ms.to_string(),
            chart_history: cfg.chart_history.to_string(),
            theme: cfg.theme.clone(),
        }
    }
}

impl App {
    pub fn new(config: AppConfig, config_exists: bool) -> Self {
        let history = config.chart_history;
        let theme = crate::theme::get_theme(&config.theme);
        Self {
            show_config: !config_exists,
            config_field: ConfigField::Url,
            config_edits: ConfigEdits::from_config(&config),
            config,
            running: true,
            shared_url: None,
            show_stats: true,
            show_slots: true,
            show_slot_detail: false,
            show_logs: true,
            server_state: ServerState::Unknown,
            error_message: None,
            model_name: String::new(),
            start_time: None,
            last_metrics_time: None,
            prometheus: None,
            prompt_tokens_total_history: VecDeque::with_capacity(history),
            predicted_tokens_total_history: VecDeque::with_capacity(history),
            prompt_tps_history: VecDeque::with_capacity(history),
            predicted_tps_history: VecDeque::with_capacity(history),
            slots: HashMap::new(),
            selected_slot: None,
            logs: VecDeque::with_capacity(1000),
            log_dedup: (String::new(), String::new(), 0),
            theme,
        }
    }

    pub fn uptime_str(&self) -> String {
        if let Some(start) = self.start_time {
            format_duration(start.elapsed())
        } else {
            "N/A".to_string()
        }
    }

    /// Process a batch of /slots snapshots, removing slots that no longer exist.
    pub fn update_slots(&mut self, snapshots: Vec<SlotSnapshot>) {
                let current_ids: HashSet<u32> = snapshots.iter().map(|s| s.id).collect();
        let removed: Vec<u32> = self.slots.keys().filter(|id| !current_ids.contains(id)).cloned().collect();
        if !removed.is_empty() {
            for id in &removed {
                self.slots.remove(id);
            }
            // Reset selection (and detail panel) if selected slot was removed
            if let Some(ref sel) = self.selected_slot
                && removed.contains(sel)
            {
                self.selected_slot = None;
                self.show_slot_detail = false;
            }
        }
        for snapshot in snapshots {
            self.update_slot(snapshot);
        }
    }

    /// Process a single /slots snapshot update (called by update_slots).
    pub fn update_slot(&mut self, snapshot: SlotSnapshot) {
        let prev = self.slots.get(&snapshot.id).cloned().unwrap_or_default();

        // Detect task boundary — n_decoded is stale across tasks.
        // Reset prev.n_decoded so we don't diff stale data.
        let prev_decoded = if snapshot.id_task != prev.id_task && prev.id_task != 0 {
            snapshot.n_decoded // new task: treat current as baseline
        } else {
            prev.n_decoded
        };

        // Detect phase and compute TPS from deltas
        let prompt_delta = snapshot
            .n_prompt_tokens_processed
            .saturating_sub(prev.n_prompt_tokens_processed);
        let gen_delta = snapshot.n_decoded.saturating_sub(prev_decoded);

        let phase = if !snapshot.is_processing {
            SlotPhase::Idle
        } else if gen_delta > 0 {
            SlotPhase::Generation
        } else {
            SlotPhase::Prompt
        };

        // Compute progress
        let progress = match phase {
            SlotPhase::Prompt => {
                let done =
                    snapshot.n_prompt_tokens_processed.saturating_sub(snapshot.n_prompt_tokens_cache) as f64;
                // Prefer log-derived expected total for a stable goal.
                // Fallback to n_ctx (worst-case, bar undershoots but stays stable).
                let goal = if let Some(total) = prev.expected_prompt_total {
                    total.saturating_sub(snapshot.n_prompt_tokens_cache) as f64
                } else {
                    snapshot.n_ctx.saturating_sub(snapshot.n_prompt_tokens_cache) as f64
                };
                if goal > 0.0 { done / goal } else { 1.0 }
            }
            SlotPhase::Generation if snapshot.max_tokens > 0 => {
                snapshot.n_decoded as f64 / snapshot.max_tokens as f64
            }
            _ => 0.0,
        };

        // Windowed TPS: accumulate deltas over actual elapsed time.
        // This smooths out GPU upload chunking (n_prompt_tokens_processed jumps in ~1024 steps).
        // Reset window only on phase change, not on 0-delta ticks between GPU chunks.
        let now = Instant::now();

        let (prompt_tps, prompt_window_start, prompt_window_tokens) = if phase == SlotPhase::Prompt {
            let window_start = prev.prompt_tps_window_start.unwrap_or(now);
            let accumulated = prev.prompt_tps_window_tokens + prompt_delta;
            let elapsed = now.duration_since(window_start).as_secs_f64();
            let tps = if elapsed > 0.0 { accumulated as f64 / elapsed } else { 0.0 };
            (tps, Some(window_start), accumulated)
        } else {
            // Phase changed — reset window
            (0.0, None, 0)
        };

        let (gen_tps, gen_window_start, gen_window_tokens) = if phase == SlotPhase::Generation {
            let window_start = prev.gen_tps_window_start.unwrap_or(now);
            let accumulated = prev.gen_tps_window_tokens + gen_delta;
            let elapsed = now.duration_since(window_start).as_secs_f64();
            let tps = if elapsed > 0.0 { accumulated as f64 / elapsed } else { 0.0 };
            (tps, Some(window_start), accumulated)
        } else {
            // Phase changed — reset window
            (0.0, None, 0)
        };

        let slot = Slot {
            id: snapshot.id,
            phase,
            progress: progress.min(1.0),
            prompt_tps,
            gen_tps,
            prompt_tps_window_start: prompt_window_start,
            prompt_tps_window_tokens: prompt_window_tokens,
            gen_tps_window_start: gen_window_start,
            gen_tps_window_tokens: gen_window_tokens,
            n_prompt_tokens: snapshot.n_prompt_tokens,
            n_prompt_tokens_processed: snapshot.n_prompt_tokens_processed,
            n_decoded: snapshot.n_decoded,
            max_tokens: snapshot.max_tokens,
            n_ctx: snapshot.n_ctx,
            // Reset expected_prompt_total on task boundary
            expected_prompt_total: if snapshot.id_task != prev.id_task && prev.id_task != 0 {
                None
            } else {
                prev.expected_prompt_total
            },
            n_prompt_tokens_cache: snapshot.n_prompt_tokens_cache,
            id_task: snapshot.id_task,
            params: snapshot.params,
            last_task: prev.last_task,
            current_task_start: prev.current_task_start,
            selected: self.selected_slot == Some(snapshot.id),
        };

        self.slots.insert(snapshot.id, slot);
    }

    /// Apply expected prompt total from log regex.
    #[allow(dead_code)]
    pub fn apply_expected_total(&mut self, slot_id: u32, total: u64) {
        if let Some(slot) = self.slots.get_mut(&slot_id) {
            slot.expected_prompt_total = Some(total);
        }
    }

    /// Process Prometheus metrics update.
    pub fn update_prometheus(&mut self, metrics: PrometheusMetrics) {
        self.last_metrics_time = Some(Instant::now());
        self.prometheus = Some(metrics.clone());
        let max_len = self.config.chart_history;

        self.prompt_tokens_total_history
            .push_back(metrics.prompt_tokens_total);
        while self.prompt_tokens_total_history.len() > max_len {
            self.prompt_tokens_total_history.pop_front();
        }
        self.predicted_tokens_total_history
            .push_back(metrics.predicted_tokens_total);
        while self.predicted_tokens_total_history.len() > max_len {
            self.predicted_tokens_total_history.pop_front();
        }
        self.prompt_tps_history.push_back(metrics.prompt_tps);
        while self.prompt_tps_history.len() > max_len {
            self.prompt_tps_history.pop_front();
        }
        self.predicted_tps_history.push_back(metrics.predicted_tps);
        while self.predicted_tps_history.len() > max_len {
            self.predicted_tps_history.pop_front();
        }
    }

    #[allow(dead_code)]
    pub fn add_log(&mut self, line: String) {
        // Strip timestamp prefix (first token) for comparison
        let key = line.find(' ').map(|i| &line[i + 1..]).unwrap_or(&line);
        let key = key.trim().to_string();

        if self.log_dedup.0 == key && !key.is_empty() {
            self.log_dedup.2 += 1;
            if let Some(last) = self.logs.back_mut() {
                *last = format!("{}  (×{})", self.log_dedup.1, self.log_dedup.2);
            }
        } else {
            // Flush previous: remove suffix if count was 1
            if self.log_dedup.2 == 1
                && let Some(last) = self.logs.back_mut() {
                    *last = self.log_dedup.1.clone();
                }
            self.log_dedup = (key.clone(), line.clone(), 1);
            self.logs.push_back(line);
            if self.logs.len() > 1000 {
                self.logs.pop_front();
            }
        }
    }

    pub fn handle_key(&mut self, code: crossterm::event::KeyCode, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        // Config popup has priority
        if self.show_config {
            self.handle_config_key(code, &key);
            return;
        }

        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.running = false,
            KeyCode::Char('1') => self.show_stats = !self.show_stats,
            KeyCode::Char('2') => self.show_slots = !self.show_slots,
            KeyCode::Char('3') => {
                self.show_slot_detail = !self.show_slot_detail;
                if self.show_slot_detail && self.selected_slot.is_none()
                    && let Some(first_id) = self.slots.keys().cloned().min() {
                        self.selected_slot = Some(first_id);
                        self.update_selection();
                    }
            }
            KeyCode::Char('4') => self.show_logs = !self.show_logs,
            KeyCode::Down => self.select_next_slot(),
            KeyCode::Up => self.select_prev_slot(),
            KeyCode::Esc => {
                // Open config popup
                self.show_config = true;
                self.config_field = ConfigField::Url;
                self.config_edits = ConfigEdits::from_config(&self.config);
                // Clear selection
                self.selected_slot = None;
                for slot in self.slots.values_mut() {
                    slot.selected = false;
                }
            }
            _ => {}
        }
    }

    fn handle_config_key(&mut self, code: crossterm::event::KeyCode, _key: &crossterm::event::KeyEvent) {
        use crossterm::event::KeyCode;

        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                // Discard changes and restore theme
                self.theme = crate::theme::get_theme(&self.config.theme);
                self.show_config = false;
            }
            KeyCode::Enter => {
                // Save config
                self.save_config();
                self.show_config = false;
            }
            KeyCode::Left | KeyCode::Right
                // Cycle theme when on Theme field
                if self.config_field == ConfigField::Theme => {
                    let themes = [
                        "default", "nord", "gruvbox", "catppuccin",
                        "dracula", "tokyo-night", "kanagawa", "onedark",
                        "horizon", "flexoki",
                    ];
                    let len = themes.len();
                    if let Some(pos) = themes.iter().position(|t| *t == self.config_edits.theme) {
                        let next = if code == KeyCode::Right {
                            (pos + 1) % len
                        } else {
                            pos.checked_sub(1).unwrap_or(len - 1)
                        };
                        self.config_edits.theme = themes[next].to_string();
                        // Apply immediately for instant feedback
                        self.theme = crate::theme::get_theme(&self.config_edits.theme);
                    }
                }
            KeyCode::Down | KeyCode::Tab => {
                // Next field
                self.config_field = match self.config_field {
                    ConfigField::Url => ConfigField::TmuxSession,
                    ConfigField::TmuxSession => ConfigField::SlotsPollMs,
                    ConfigField::SlotsPollMs => ConfigField::MetricsPollMs,
                    ConfigField::MetricsPollMs => ConfigField::ChartHistory,
                    ConfigField::ChartHistory => ConfigField::Theme,
                    ConfigField::Theme => ConfigField::Url,
                };
            }
            KeyCode::Up | KeyCode::BackTab => {
                // Prev field
                self.config_field = match self.config_field {
                    ConfigField::Url => ConfigField::Theme,
                    ConfigField::TmuxSession => ConfigField::Url,
                    ConfigField::SlotsPollMs => ConfigField::TmuxSession,
                    ConfigField::MetricsPollMs => ConfigField::SlotsPollMs,
                    ConfigField::ChartHistory => ConfigField::MetricsPollMs,
                    ConfigField::Theme => ConfigField::ChartHistory,
                };
            }
            KeyCode::Backspace => {
                self.active_field_buf().pop();
            }
            KeyCode::Char(c) => {
                self.active_field_buf().push(c);
            }
            _ => {}
        }
    }

    fn active_field_buf(&mut self) -> &mut String {
        match self.config_field {
            ConfigField::Url => &mut self.config_edits.url,
            ConfigField::TmuxSession => &mut self.config_edits.tmux_session,
            ConfigField::SlotsPollMs => &mut self.config_edits.slots_poll_ms,
            ConfigField::MetricsPollMs => &mut self.config_edits.metrics_poll_ms,
            ConfigField::ChartHistory => &mut self.config_edits.chart_history,
            ConfigField::Theme => &mut self.config_edits.theme,
        }
    }

    fn save_config(&mut self) {
        let mut cfg = self.config.clone();
        let url_changed = cfg.url != self.config_edits.url;
        cfg.url = self.config_edits.url.clone();
        cfg.tmux_session = if self.config_edits.tmux_session.is_empty() {
            None
        } else {
            Some(self.config_edits.tmux_session.clone())
        };
        if let Ok(v) = self.config_edits.slots_poll_ms.parse() {
            cfg.slots_poll_ms = v;
        }
        if let Ok(v) = self.config_edits.metrics_poll_ms.parse() {
            cfg.metrics_poll_ms = v;
        }
        if let Ok(v) = self.config_edits.chart_history.parse() {
            cfg.chart_history = v;
        }
        cfg.theme = self.config_edits.theme.clone();

        // Update shared URL for live poller updates
        if url_changed
            && let Some(ref shared) = self.shared_url {
                let mut url = shared.lock().unwrap();
                *url = cfg.base_url().to_string();
            }

        // Apply theme immediately
        self.theme = crate::theme::get_theme(&cfg.theme);

        self.config = cfg;

        // Write to disk
        if let Err(e) = self.write_config() {
            eprintln!("Failed to save config: {}", e);
        }
    }

    fn write_config(&self) -> std::io::Result<()> {
        use std::io::Write;
        let mut path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default());
        path.push(".config/llama-hud/config.json");

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(&self.config)?;
        let mut file = std::fs::File::create(&path)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    fn select_next_slot(&mut self) {
        self.show_slot_detail = true;
        let mut ids: Vec<u32> = self.slots.keys().cloned().collect();
        ids.sort();
        if ids.is_empty() {
            return;
        }
        let current = self.selected_slot.unwrap_or(ids[0]);
        if let Some(pos) = ids.iter().position(|&id| id == current) {
            let next = (pos + 1) % ids.len();
            self.selected_slot = Some(ids[next]);
        }
        self.update_selection();
    }

    fn select_prev_slot(&mut self) {
        self.show_slot_detail = true;
        let mut ids: Vec<u32> = self.slots.keys().cloned().collect();
        ids.sort();
        if ids.is_empty() {
            return;
        }
        let current = self.selected_slot.unwrap_or(ids[0]);
        let pos = ids.iter().position(|&id| id == current).unwrap_or(0);
        let prev = if pos == 0 { ids.len() - 1 } else { pos - 1 };
        self.selected_slot = Some(ids[prev]);
        self.update_selection();
    }

    fn update_selection(&mut self) {
        for (id, slot) in self.slots.iter_mut() {
            slot.selected = self.selected_slot == Some(*id);
        }
    }
}

pub fn format_duration(dur: Duration) -> String {
    let secs = dur.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

pub fn format_tokens(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        format!("{}", count)
    }
}

pub fn format_tps(tps: f64) -> String {
    if tps >= 100.0 {
        format!("{:.0} t/s", tps)
    } else {
        format!("{:.1} t/s", tps)
    }
}

pub fn format_eta(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
