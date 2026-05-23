//! Minimal config — JSON at ~/.config/llama-hud/config.json.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const CONFIG_FILE: &str = "~/.config/llama-hud/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_url")]
    pub url: String,
    #[serde(default = "default_tmux_session")]
    pub tmux_session: Option<String>,
    #[serde(default = "default_slots_poll_ms")]
    pub slots_poll_ms: u64,
    #[serde(default = "default_metrics_poll_ms")]
    pub metrics_poll_ms: u64,
    #[serde(default = "default_chart_history")]
    pub chart_history: usize,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_url() -> String {
    "http://127.0.0.1:8080".to_string()
}
fn default_tmux_session() -> Option<String> {
    Some("llama".to_string())
}
fn default_slots_poll_ms() -> u64 {
    200
}
fn default_metrics_poll_ms() -> u64 {
    2000
}
fn default_chart_history() -> usize {
    600
}
fn default_theme() -> String {
    "default".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            url: default_url(),
            tmux_session: default_tmux_session(),
            slots_poll_ms: default_slots_poll_ms(),
            metrics_poll_ms: default_metrics_poll_ms(),
            chart_history: default_chart_history(),
            theme: default_theme(),
        }
    }
}

impl AppConfig {
    pub fn base_url(&self) -> &str {
        self.url.trim_end_matches('/')
    }
}

pub fn load_config() -> (AppConfig, bool) {
    let path = expand_tilde(CONFIG_FILE);
    let exists = path.exists();

    if !exists {
        return (AppConfig::default(), false);
    }

    let cfg = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str::<AppConfig>(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    };
    (cfg, true)
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        let mut p = PathBuf::from(home);
        p.push(rest);
        return p;
    }
    PathBuf::from(path)
}
