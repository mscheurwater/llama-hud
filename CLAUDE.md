# CLAUDE.md

## Project

`llama-hud` — btop-style terminal dashboard for monitoring `llama-server`.

**Built with:** Rust, ratatui 0.29, crossterm 0.28, tokio 1, reqwest 0.12.

## Rules

**NEVER commit or push without explicit user permission.** Always ask first.

## Commands

```bash
cargo build
cargo run
cargo check
cargo clippy
cargo fmt
```

## Architecture

**Monitor-only.** No server spawn/stop. Polls the llama-server HTTP API.

### Data sources

| Source | Poll | Purpose |
|--------|------|---------|
| `GET /slots` | 500ms | Per-slot state, progress, TPS, params |
| `GET /metrics` (Prometheus) | 2s | Server-wide cumulative stats, gauge TPS |
| Log tail (optional tmux) | line-by-line | Single regex for expected prompt total |
| `GET /v1/models` | once | Model name |

### Config

Minimal JSON at `~/.config/llama-hud/config.json`:

```json
{
  "url": "http://127.0.0.1:8080",
  "tmux_session": null,
  "slots_poll_ms": 500,
  "metrics_poll_ms": 2000,
  "chart_history": 600
}
```

URL is the only required field. Config editor via ESC key.

### Slot state machine

**Core rule:** State is determined by **diffing two snapshots** 500ms apart, not from a single snapshot.

```
delta_processed = n_prompt_tokens_processed - prev.n_prompt_tokens_processed
delta_decoded   = n_decoded - prev.n_decoded

delta_processed > 0  → PROMPT phase
delta_decoded > 0    → GEN phase
neither changing     → IDLE
```

**TPS:** Windowed accumulation over actual elapsed time. Resets on phase change.
(Smooths GPU upload chunking — `n_prompt_tokens_processed` jumps in ~1024 steps.)

**Progress (cache-corrected):**
- Prompt: `(n_prompt_tokens_processed - n_prompt_tokens_cache) / expected_total`
- Gen: `n_decoded / max_tokens` (from `params.max_tokens`)

**Task boundary:** `id_task` changed → reset prev.n_decoded baseline.

### Key insight about n_prompt_tokens

`n_prompt_tokens` is NOT the final total prompt token count. It's the count allocated so far and grows during processing. The actual final count is only known after prompt processing completes. The log regex (`n_tokens / progress`) gives an estimate.

### Key insight about n_prompt_tokens_processed

`n_prompt_tokens_processed == n_prompt_tokens` does NOT mean prompt processing is done. It just means all tokens currently known to the server are processed. The GPU uploads tokens in ~1024-token chunks, and more chunks may still be in-flight. Use `is_processing` + phase diffing to determine actual completion.

### Layout (btop-style)

Views: 1=stats, 2=slots, 3=detail, 4=logs

- Stats: Prometheus totals + 2 charts (total tokens, throughput)
- Slots: fixed 12 lines, `#N  PHASE ▓▓▓▓ 10K  75%  853 t/s`
- Detail: slot params panel (press 3 or ↑↓ to select)
- Logs: fills remaining space, optional tmux tail

ESC opens config editor.

### File structure

```
src/
  main.rs          — CLI (clap), terminal setup, event loop, poller spawns, ctrlc handler
  app.rs           — App struct, Slot, SlotPhase, state update logic, config editor
  slots_poller.rs  — GET /slots poller, SlotSnapshot, JSON parsing, model name fetch
  prometheus.rs    — GET /metrics poller, PrometheusMetrics struct
  parser.rs        — Minimal: only print_timing regex for expected prompt total
  widgets.rs       — All ratatui rendering (header, stats, charts, slots, footer, detail panel)
  theme.rs         — Color palette constants (btop-inspired blue/green)
  config.rs        — AppConfig, load from JSON
```

### Dependencies

- ratatui 0.29 — TUI framework
- crossterm 0.28 — terminal control
- tokio 1 (rt-multi-thread, time, macros, sync) — async runtime
- serde + serde_json — config serialization
- regex 1 — log parsing (single pattern)
- chrono 0.4 (clock) — timestamps
- reqwest 0.12 (json) — HTTP client for API polling
- clap 4 (derive) — CLI args
- ctrlc 3 — graceful shutdown


