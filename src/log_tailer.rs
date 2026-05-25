//! Tails a tmux session's output by polling capture-pane.

use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

pub fn start_tailing(
    session: String,
    running: Arc<AtomicBool>,
) -> (mpsc::Receiver<String>, tokio::task::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel(256);

    let handle = tokio::spawn(async move {
        let mut last_lines: Vec<String> = Vec::new();

        while running.load(Ordering::SeqCst) {
            let output = match tokio::task::spawn_blocking({
                let session = session.clone();
                move || {
                    Command::new("tmux")
                        .args(["capture-pane", "-t", &session, "-p"])
                        .output()
                }
            })
            .await
            {
                Ok(Ok(o)) => String::from_utf8_lossy(&o.stdout).to_string(),
                _ => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            let all: Vec<String> = output.lines().map(|l| l.trim().to_string()).collect();
            if all.is_empty() {
                tokio::time::sleep(Duration::from_millis(500)).await;
                continue;
            }

            // Find where we left off: scan for the last 3 lines we sent
            let start = if last_lines.len() >= 3 {
                let needle = &last_lines[last_lines.len() - 3..];
                let found = all.windows(3).position(|w| {
                    let wtrimmed: Vec<&str> = w.iter().map(|s| s.as_str()).collect();
                    needle.iter().zip(wtrimmed.iter()).all(|(a, b)| a == b)
                });
                found
                    .map(|i| i + 3)
                    .unwrap_or(all.len().saturating_mul(9) / 10)
            } else {
                all.len().saturating_mul(9) / 10
            };

            let new_lines: Vec<String> = all
                .iter()
                .skip(start)
                .filter(|l| !l.is_empty())
                .cloned()
                .collect();

            for line in &new_lines {
                let _ = tx.send(line.clone()).await;
            }

            // Keep last 10 lines for tracking
            last_lines = all
                .iter()
                .skip(all.len().saturating_sub(10))
                .cloned()
                .collect();

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    (rx, handle)
}
