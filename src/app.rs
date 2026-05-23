//! Application state — slots, metrics history, server info.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use crate::config::AppConfig;
use crate::prometheus::PrometheusMetrics;
use crate::slots_poller::SlotSnapshot;

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

#[derive(Debug, Clone, Default)]
pub struct Slot {
    pub id: u32,
    pub phase: SlotPhase,
    pub progress: f64, // 0.0 to 1.0
    pub prompt_tps: f64,
    pub gen_tps: f64,
    pub n_prompt_tokens: u64,           // current context size (loaded prompt)
    pub n_prompt_tokens_processed: u64,
    pub n_decoded: u64,
    pub max_tokens: u64,
    pub n_ctx: u64,
    pub expected_prompt_total: Option<u64>,
    pub n_prompt_tokens_cache: u64,
    pub last_task: Option<CompletedTask>,
    pub current_task_start: Option<Instant>,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Split, // both panels
    Stats, // stats only
    Slots, // slots only
    Logs,  // log stream only
}

pub struct App {
    pub config: AppConfig,
    pub running: bool,
    pub view_mode: ViewMode,
    pub server_state: ServerState,
    pub model_name: String,
    pub start_time: Option<Instant>,

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
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let history = config.chart_history;
        Self {
            config,
            running: true,
            view_mode: ViewMode::Split,
            server_state: ServerState::Unknown,
            model_name: String::new(),
            start_time: None,
            prometheus: None,
            prompt_tokens_total_history: VecDeque::with_capacity(history),
            predicted_tokens_total_history: VecDeque::with_capacity(history),
            prompt_tps_history: VecDeque::with_capacity(history),
            predicted_tps_history: VecDeque::with_capacity(history),
            slots: HashMap::new(),
            selected_slot: None,
            logs: VecDeque::with_capacity(1000),
        }
    }

    pub fn uptime_str(&self) -> String {
        if let Some(start) = self.start_time {
            format_duration(start.elapsed())
        } else {
            "N/A".to_string()
        }
    }

    /// Process a /slots snapshot update.
    pub fn update_slot(&mut self, snapshot: SlotSnapshot) {
        let prev = self.slots.get(&snapshot.id).cloned().unwrap_or_default();

        // Phase detection from deltas

        // Detect phase and compute TPS from deltas
        let prompt_delta = snapshot
            .n_prompt_tokens_processed
            .saturating_sub(prev.n_prompt_tokens_processed);
        let gen_delta = snapshot.n_decoded.saturating_sub(prev.n_decoded);

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
                let useful = snapshot.n_prompt_tokens.saturating_sub(snapshot.n_prompt_tokens_cache) as f64;
                let done =
                    snapshot.n_prompt_tokens_processed.saturating_sub(snapshot.n_prompt_tokens_cache) as f64;
                if useful > 0.0 { done / useful } else { 1.0 }
            }
            SlotPhase::Generation if snapshot.max_tokens > 0 => {
                snapshot.n_decoded as f64 / snapshot.max_tokens as f64
            }
            _ => 0.0,
        };

        let slot = Slot {
            id: snapshot.id,
            phase,
            progress: progress.min(1.0),
            prompt_tps: if prompt_delta > 0 {
                prompt_delta as f64 / 0.5
            } else {
                0.0
            },
            gen_tps: if gen_delta > 0 {
                gen_delta as f64 / 0.5
            } else {
                0.0
            },
            n_prompt_tokens: snapshot.n_prompt_tokens,
            n_prompt_tokens_processed: snapshot.n_prompt_tokens_processed,
            n_decoded: snapshot.n_decoded,
            max_tokens: snapshot.max_tokens,
            n_ctx: snapshot.n_ctx,
            expected_prompt_total: prev.expected_prompt_total,
            n_prompt_tokens_cache: snapshot.n_prompt_tokens_cache,
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
        self.logs.push_back(line);
        if self.logs.len() > 1000 {
            self.logs.pop_front();
        }
    }

    pub fn handle_key(&mut self, code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => self.running = false,
            KeyCode::Char('1') => self.view_mode = ViewMode::Stats,
            KeyCode::Char('2') => self.view_mode = ViewMode::Slots,
            KeyCode::Char('3') => self.view_mode = ViewMode::Split,
            KeyCode::Char('4') => self.view_mode = ViewMode::Logs,
            KeyCode::Down => self.select_next_slot(),
            KeyCode::Up => self.select_prev_slot(),
            _ => {}
        }
    }

    fn select_next_slot(&mut self) {
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
