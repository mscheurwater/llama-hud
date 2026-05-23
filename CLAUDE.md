# CLAUDE.md

## Project

`llama-hud` — btop-style terminal dashboard for monitoring `llama-server`. Fresh project, replacing the older `llamaRtui` at `/home/mark/Projects/llamaRtui/`.

**Design doc:** `PLAN.md` — read this before implementing anything.

**Built with:** Rust, ratatui 0.29, crossterm 0.28, tokio 1, reqwest 0.12.

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

URL is the only required field. Hardcoded defaults for v1, no config editor yet.

### Slot state machine

**Core rule:** State is determined by **diffing two snapshots** 500ms apart, not from a single snapshot.

```
delta_processed = n_prompt_tokens_processed - prev.n_prompt_tokens_processed
delta_decoded   = n_decoded - prev.n_decoded

delta_processed > 0  → PROMPT phase,  prompt_tps = delta_processed / 0.5
delta_decoded > 0    → GEN phase,     gen_tps = delta_decoded / 0.5
neither changing     → IDLE
```

**Progress (cache-corrected):**
- Prompt: `(n_prompt_tokens_processed - n_prompt_tokens_cache) / (n_prompt_tokens - n_prompt_tokens_cache)`
- Gen: `n_decoded / max_tokens` (from `params.max_tokens`)
- If no expected total from log regex, fallback to `n_ctx` as denominator (overshoots but gives visual feedback)

**Task boundary:** `id_task` changed → capture previous task stats, reset.

### Key insight about n_prompt_tokens

`n_prompt_tokens` is NOT the final total prompt token count. It's the count allocated so far and grows during processing. The actual final count is only known after prompt processing completes. The log regex (`n_tokens / progress`) gives an estimate.

### Layout (btop-style)

```
╭─┐¹llama-hud┌─────────────────────────────────────────────────────────────────────┐time┌─╮
│ Model Name  ● State  Uptime  Host:Port  N slots                                  │
├─Prompt───────────────────────────────────────────┬─Gen────────────────────────────┤
│ Total:  52.8K tokens                             │ Total:  3.7K tokens            │
│ Avg:    542 t/s                                  │ Avg:    39 t/s                 │
│ Peak ctx: 59.4K  Active: 0                       │ Decodes: 1,667                 │
├─Total Tokens───────────────────────┬─Throughput───────────────────────────────────┤
│ ↑ Prompt  ─────────────────────────╮│ ↑ Prompt TPS ─────────────────────────────╮│
│ ↓ Gen     ──────╮                  ╰│ ↓ Gen TPS   ──────╮                       ││
│                 ╰───────────────────╯│                   ╰───────────────────────╯│
└──────────────────────────────────────┴────────────────────────────────────────────┘
╭─┐²slots┌──────────────────────────────────────────────────────────────────────────┤
│ #0  THINK ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  75%  1.2K t/s  47K tok│
│ #1  WRITE ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  95%  45 t/s    121/20K │
│ #2  IDLE  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ │
╰┘↑↓ select└┘1=stats└┘2=slots└┘3=split└┘4=logs└┘q=quit└────────────────────────────┘
```

Views: 1=stats, 2=slots, 3=split, 4=logs (optional log stream).

Arrow keys select slots → detail panel appears on right with params (temp, top_p, cache hits, etc.).

### File structure

```
src/
  main.rs          — CLI (clap), terminal setup, event loop, poller spawns, ctrlc handler
  app.rs           — App struct, DashboardState, Slot, SlotPhase, state update logic
  slots_poller.rs  — GET /slots poller, SlotSnapshot, JSON parsing, model name fetch
  prometheus.rs    — GET /metrics poller, PrometheusMetrics struct
  parser.rs        — Minimal: only print_timing regex for expected prompt total
  widgets.rs       — All ratatui rendering (header, stats, charts, slots, footer, detail panel)
  theme.rs         — Color palette constants (btop-inspired blue/green)
  config.rs        — AppConfig, load from JSON
```

### What's done vs what's left

**Done (scaffolded):** Cargo.toml, all module files with core types, theme, config, parser, app state, slots_poller.

**Not yet implemented:**
- `prometheus.rs` — poller + metrics struct
- `widgets.rs` — all rendering (header bar, stats panel, 4 charts, slots panel, detail panel, footer)
- `main.rs` — event loop (100ms render, 500ms slots poll, 2s prom poll), ctrlc handler, terminal setup/cleanup
- Slot selection detail panel
- Log tailer (optional tmux mode, 4th view)
- Config editor

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

No linter/formatter in old project. This one has `clippy = "warn"` + `pedantic = "warn"` in Cargo.toml.

### Old project (reference only)

`/home/mark/Projects/llamaRtui/` — the original. Structural inspiration but being replaced. Key differences:
- Old: log-driven, regex-heavy, 1Hz refresh, server management, no slot selection
- New: API-driven, single regex, 100ms refresh, monitor-only, slot selection with detail panel
