//! Polls /metrics (Prometheus) endpoint, parses server-wide stats.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct PrometheusMetrics {
    pub prompt_tps: f64,
    pub predicted_tps: f64,
    pub prompt_tokens_total: u64,
    pub predicted_tokens_total: u64,
    pub n_decode_total: u64,
    pub n_tokens_max: u64,
    pub requests_processing: u64,
    pub requests_deferred: u64,
    pub n_busy_slots_per_decode: f64,
}

pub fn parse_prometheus(body: &str) -> PrometheusMetrics {
    let mut values: HashMap<String, f64> = HashMap::new();

    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() == 2
            && let Ok(val) = parts[1].parse::<f64>()
        {
            values.insert(parts[0].to_string(), val);
        }
    }

    let pt = *values.get("llamacpp:prompt_tokens_total").unwrap_or(&0.0);
    let ps = *values.get("llamacpp:prompt_seconds_total").unwrap_or(&0.0);
    let gt = *values
        .get("llamacpp:tokens_predicted_total")
        .unwrap_or(&0.0);
    let gs = *values
        .get("llamacpp:tokens_predicted_seconds_total")
        .unwrap_or(&0.0);

    PrometheusMetrics {
        prompt_tps: values
            .get("llamacpp:prompt_tokens_seconds")
            .copied()
            .unwrap_or_else(|| if ps > 0.0 { pt / ps } else { 0.0 }),
        predicted_tps: values
            .get("llamacpp:predicted_tokens_seconds")
            .copied()
            .unwrap_or_else(|| if gs > 0.0 { gt / gs } else { 0.0 }),
        prompt_tokens_total: pt as u64,
        predicted_tokens_total: gt as u64,
        n_decode_total: *values.get("llamacpp:n_decode_total").unwrap_or(&0.0) as u64,
        n_tokens_max: *values.get("llamacpp:n_tokens_max").unwrap_or(&0.0) as u64,
        requests_processing: *values.get("llamacpp:requests_processing").unwrap_or(&0.0) as u64,
        requests_deferred: *values.get("llamacpp:requests_deferred").unwrap_or(&0.0) as u64,
        n_busy_slots_per_decode: *values
            .get("llamacpp:n_busy_slots_per_decode")
            .unwrap_or(&0.0),
    }
}

pub fn start_prometheus_poller(
    base_url: String,
    poll_ms: u64,
) -> tokio::sync::mpsc::Receiver<PrometheusMetrics> {
    let (tx, rx) = tokio::sync::mpsc::channel(8);

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;

            let url = format!("{}/metrics", base_url);
            let body = match client.get(&url).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(text) => text,
                    Err(_) => continue,
                },
                Err(_) => continue,
            };

            let metrics = parse_prometheus(&body);
            if tx.send(metrics).await.is_err() {
                break;
            }
        }
    });

    rx
}
