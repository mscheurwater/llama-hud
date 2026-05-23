//! Polls /slots endpoint every 500ms, sends SlotSnapshot updates.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SlotApiResponse {
    pub id: u32,
    #[allow(dead_code)]
    pub speculative: Option<bool>,
    #[serde(default)]
    pub is_processing: bool,
    #[serde(default)]
    pub id_task: i64,
    #[serde(default)]
    pub n_prompt_tokens: u64,
    #[serde(default)]
    pub n_prompt_tokens_processed: u64,
    #[serde(default)]
    pub n_prompt_tokens_cache: u64,
    #[serde(default)]
    pub n_ctx: u64,
    #[serde(default)]
    pub params: Option<SlotParams>,
    #[serde(default)]
    pub next_token: Option<Vec<NextTokenInfo>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SlotParams {
    #[serde(default)]
    pub max_tokens: i64,
    #[serde(default)]
    pub n_predict: i64,
    #[serde(default)]
    pub temperature: f64,
    #[serde(default)]
    pub top_p: f64,
    #[serde(default)]
    pub top_k: i64,
    #[serde(default)]
    pub min_p: f64,
    #[serde(default)]
    pub repeat_penalty: f64,
    #[serde(default)]
    pub repeat_last_n: i64,
    #[serde(default)]
    pub presence_penalty: f64,
    #[serde(default)]
    pub frequency_penalty: f64,
    #[serde(default)]
    #[allow(dead_code)]
    pub mirostat_tau: f64,
    #[serde(default)]
    #[allow(dead_code)]
    pub stop: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct NextTokenInfo {
    #[serde(default)]
    pub has_next_token: bool,
    #[serde(default)]
    pub n_remain: i64,
    #[serde(default)]
    pub n_decoded: u64,
}

/// Flattened snapshot for state diffing.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SlotSnapshot {
    pub id: u32,
    pub is_processing: bool,
    pub id_task: i64,
    pub n_prompt_tokens: u64,
    pub n_prompt_tokens_processed: u64,
    pub n_prompt_tokens_cache: u64,
    pub n_decoded: u64,
    pub n_remain: i64,
    pub max_tokens: u64,
    pub n_ctx: u64,
    pub params: SlotParams,
}

impl SlotApiResponse {
    pub fn to_snapshot(&self) -> SlotSnapshot {
        let max_tokens = self
            .params
            .as_ref()
            .map_or(0i64, |p| p.max_tokens.max(p.n_predict)).max(0) as u64;
        let next = self.next_token.as_ref().and_then(|v| v.first());
        SlotSnapshot {
            id: self.id,
            is_processing: self.is_processing,
            id_task: self.id_task,
            n_prompt_tokens: self.n_prompt_tokens,
            n_prompt_tokens_processed: self.n_prompt_tokens_processed,
            n_prompt_tokens_cache: self.n_prompt_tokens_cache,
            n_decoded: next.map_or(0, |n| n.n_decoded),
            n_remain: next.map_or(-1, |n| n.n_remain),
            max_tokens,
            n_ctx: self.n_ctx,
            params: self.params.clone().unwrap_or_default(),
        }
    }
}

/// Fetches model name from /v1/models endpoint.
pub async fn fetch_model_name(base_url: &str, client: &reqwest::Client) -> String {
    let url = format!("{}/v1/models", base_url);
    if let Ok(resp) = client.get(&url).send().await
        && let Ok(data) = resp.json::<serde_json::Value>().await
        && let Some(name) = data["models"][0]["name"].as_str()
    {
        return name.to_string();
    }
    String::new()
}

/// Background poller — sends snapshots via channel.
pub fn start_slots_poller(
    poll_ms: u64,
    error_tx: tokio::sync::mpsc::Sender<String>,
    shared_url: std::sync::Arc<std::sync::Mutex<String>>,
) -> tokio::sync::mpsc::Receiver<Vec<SlotSnapshot>> {
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;

            let base_url = shared_url.lock().unwrap().clone();
            let url = format!("{}/slots", base_url);
            match client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if !status.is_success() {
                        let _ = error_tx.send(format!("Server returned {}", status)).await;
                        continue;
                    }
                    match resp.json::<Vec<SlotApiResponse>>().await {
                        Ok(slots) => {
                            let snapshots: Vec<SlotSnapshot> = slots.iter().map(|s| s.to_snapshot()).collect();
                            if tx.send(snapshots).await.is_err() {
                                break;
                            }
                            let _ = error_tx.send(String::new()).await;
                        }
                        Err(e) => {
                            let _ = error_tx.send(format!("Bad response from /slots: {}", e)).await;
                        }
                    }
                }
                Err(e) => {
                    let msg = if e.is_connect() {
                        "Connection refused — is llama-server running?".to_string()
                    } else if e.is_timeout() {
                        "Request timed out".to_string()
                    } else {
                        format!("Cannot reach server: {}", e)
                    };
                    let _ = error_tx.send(msg).await;
                }
            }
        }
    });

    rx
}
