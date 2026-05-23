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

#[derive(Debug, Clone, Deserialize)]
pub struct SlotParams {
    #[serde(default)]
    pub max_tokens: u64,
    #[serde(default)]
    pub n_predict: u64,
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
}

impl SlotApiResponse {
    pub fn to_snapshot(&self) -> SlotSnapshot {
        let max_tokens = self
            .params
            .as_ref()
            .map_or(0, |p| p.max_tokens.max(p.n_predict));
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
    base_url: String,
    poll_ms: u64,
) -> tokio::sync::mpsc::Receiver<Vec<SlotSnapshot>> {
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;

            let url = format!("{}/slots", base_url);
            if let Ok(resp) = client.get(&url).send().await
                && let Ok(slots) = resp.json::<Vec<SlotApiResponse>>().await
            {
                let snapshots: Vec<SlotSnapshot> = slots.iter().map(|s| s.to_snapshot()).collect();
                if tx.send(snapshots).await.is_err() {
                    break;
                }
            }
        }
    });

    rx
}
