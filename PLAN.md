# llama-hud — Design Doc

**New project:** `/home/mark/Projects/llama-hud/` (fresh start, not a rewrite)

**Goal:** btop-style terminal dashboard for monitoring `llama-server`, driven by the `/slots` API + Prometheus. Monitor-only (no server spawn/stop).

**Reference:** Run `btop` to see the target aesthetic.

## Decisions

| Question | Decision |
|----------|----------|
| Log tailer | Optional, 4th view mode (logs). Core is 100% API-driven. |
| Server management | Monitor-only. No start/stop/restart. |
| Config | Minimal JSON: `url` (required), `tmux_session`, poll intervals, chart history. Hardcode defaults for v1. |
| Slot selection | Arrow keys select slot, detail panel appears on right with params. |
| Replay mode | Dropped. |
| Themes | Inherit terminal colors initially. Theme system later. |
| Name | `llama-hud` |

## Minimal Config

```json
{
  "url": "http://127.0.0.1:8080",
  "tmux_session": null,
  "slots_poll_ms": 500,
  "metrics_poll_ms": 2000,
  "chart_history": 600
}
```

URL is the only required field. Everything else has sensible defaults.

## Design Patterns (from btop)

- Rounded borders (`╭─┐╰┘`), section labels with view-number shortcuts (`¹`, `²`)
- Metric cards with `Label: Value` alignment
- Braille sparkline charts (⣀⣀⣀⣀⢀)
- Block progress bars (▓░)
- Footer with action hints separated by `└┘`
- Cohesive color theme, not per-widget random colors
- ~100ms render tick

---

## Target Layout

```
╭─┐¹llamaRtui┌───────────────────────────────────────────────────────────────────────────────────────┐12:17:35┌─╮
│ Qwen3.6-27B  ● Running  2h 14m  127.0.0.1:1234  4 slots                                                            │
├─Prompt───────────────────────────────────────────┬─Gen────────────────────────────────────────────────┤
│ Total:          52.8K tokens                     │ Total:          3.7K tokens                        │
│ Avg:            542 t/s                          │ Avg:            39 t/s                             │
│ Peak ctx:       59.4K                            │ Decodes:        1,667                              │
│ Active: 0      Queued: 0                         │                                                      │
├─Total Tokens───────────────────────┬─Throughput──────────────────────────────────────────────────────┤
│ ↑ ─────────────────────────────────╮│ ↑ ─────────────────────────────────────────────────────╮       │
│    ╭────────╮                      ││    ╭────╮  ╭────╮  ╭────╮  ╭────╮  ╭────╮             │       │
│   ╱          ╰─────╮               ││   ╱    ╰╮  ╱    ╰╮  ╱    ╰╮  ╱    ╰╮  ╱    ╰────╮      │       │
│  ╱                  ╰────╮         ││  ╱      ╰╮        ╰╮      ╰╮      ╰╮      ╰────╮     │       │
│ ╱                        ╰────╮    ││ ╱        ╰─────────╯      ╰───────╯             ╰────╮     │       │
│ ↓ ──────╮                     ╰──╮ ││ ↓ ──────╯                                           ╰────╯       │
│        ╰────────────╮             ╰─╯│                                                                  │
└─────────────────────╯└───────────────┴────────────────────────────────────────────────────────────────╯
╭─┐²slots┌────────────────────────────────────────────────────────────────────────────────────────────┤
│ #0  THINK ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  75%  1.2K t/s  47K/60K tok│
│ #1  WRITE ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓  95%  45 t/s    121/20K tok│
│ #2  IDLE  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0%             last: 356 tok│
│ #3  DONE  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ 100%  2.8s      412 tok / 38 t/s│
╰┘↑ select ↓└┘1=stats└┘2=slots└┘3=split└┘Enter=start/stop└┘Esc=config└┘q=quit└────────────────────────┘
```

**Views (keys 1/2/3):**
- `1` = stats-only (top panel fills screen)
- `2` = slots-only (slots panel fills screen)
- `3` = split (both panels, 50/50)

---

## Data Architecture

### Sources

| Source | Poll interval | Purpose |
|--------|--------------|---------|
| `GET /slots` | 500ms | Per-slot state, progress, TPS, params |
| `GET /metrics` (Prometheus) | 2s | Server-wide cumulative stats, gauge TPS |
| Log tail (tmux/script/inline) | line-by-line | Single regex to detect prompt expected total |
| `GET /v1/models` | once at start | Model name |

### `/slots` endpoint — per-slot data

```json
{
  "id": 0,
  "n_ctx": 158720,
  "speculative": true,
  "is_processing": true,
  "id_task": 11,
  "n_prompt_tokens": 47104,
  "n_prompt_tokens_processed": 47104,
  "n_prompt_tokens_cache": 0,
  "params": {
    "max_tokens": 20000,
    "n_predict": 20000,
    "temperature": 0.6,
    "top_p": 0.95,
    ...
  },
  "next_token": [{
    "has_next_token": true,
    "n_remain": -1,
    "n_decoded": 0
  }]
}
```

### Prometheus metrics

| Metric | Type | Use |
|--------|------|-----|
| `llamacpp:prompt_tokens_total` | counter | Lifetime prompt tokens |
| `llamacpp:prompt_seconds_total` | counter | Lifetime prompt time |
| `llamacpp:tokens_predicted_total` | counter | Lifetime gen tokens |
| `llamacpp:tokens_predicted_seconds_total` | counter | Lifetime gen time |
| `llamacpp:n_decode_total` | counter | Total decode calls |
| `llamacpp:n_tokens_max` | counter | Peak context usage |
| `llamacpp:prompt_tokens_seconds` | **gauge** | Current prompt TPS (server-reported) |
| `llamacpp:predicted_tokens_seconds` | **gauge** | Current gen TPS (server-reported) |
| `llamacpp:requests_processing` | gauge | Active requests |
| `llamacpp:requests_deferred` | gauge | Queued requests |
| `llamacpp:n_busy_slots_per_decode` | gauge | Avg busy slots per decode |

### Log regex — single pattern

Only one regex needed, to seed the expected prompt total:

```
prompt processing, n_tokens = (\d+), progress = ([\d.]+), .*?([\d.]+) tokens per second
```

Example match:
```
slot print_timing: id  1 | task 162 | prompt processing, n_tokens =  44032, progress = 0.91, t =  73.95 s / 595.46 tokens per second
```

Extract: `n_tokens = 44032`, `progress = 0.91`
Compute: `expected_total = n_tokens / progress = 44032 / 0.91 ≈ 48387`

Store `expected_total` per slot. If no log line fires, fall back to `n_ctx`.

---

## Slot State Machine

**Core insight:** You can't determine state from a single snapshot. You must diff two snapshots 500ms apart and see what's changing.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotPhase {
    Idle,       // No active task
    Prompt,     // Prompt processing (THINK)
    Generation, // Token generation (WRITE)
    Done,       // Task just completed
}

#[derive(Debug, Clone)]
pub struct SlotSnapshot {
    pub id_task: i64,
    pub is_processing: bool,
    pub n_prompt_tokens: u64,
    pub n_prompt_tokens_processed: u64,
    pub n_prompt_tokens_cache: u64,
    pub n_decoded: u64,
    pub n_remain: i64,
    pub max_tokens: u64,       // from params
    pub n_ctx: u64,
}
```

### Phase detection

```rust
fn detect_phase(prev: &SlotSnapshot, curr: &SlotSnapshot) -> SlotPhase {
    if !curr.is_processing {
        return SlotPhase::Idle;
    }

    let prompt_delta = curr.n_prompt_tokens_processed - prev.n_prompt_tokens_processed;
    let gen_delta = curr.n_decoded - prev.n_decoded;

    if prompt_delta > 0 && curr.n_prompt_tokens_processed < curr.n_prompt_tokens {
        SlotPhase::Prompt
    } else if gen_delta > 0 {
        SlotPhase::Generation
    } else {
        SlotPhase::Idle // processing flag set but nothing moving yet
    }
}
```

### TPS from deltas (500ms poll interval)

```rust
let elapsed = (curr.timestamp - prev.timestamp).as_secs_f64();
let prompt_tps = prompt_delta as f64 / elapsed;
let gen_tps = gen_delta as f64 / elapsed;
```

### Progress calculation

**Prompt phase** (cache-corrected):

```rust
let useful = (curr.n_prompt_tokens - curr.n_prompt_tokens_cache) as f64;
let done = (curr.n_prompt_tokens_processed - curr.n_prompt_tokens_cache) as f64;
let progress = if useful > 0.0 { done / useful } else { 1.0 };
```

Without expected total from logs, fall back to `n_ctx`:

```rust
let progress = curr.n_prompt_tokens_processed as f64 / curr.n_ctx as f64;
```

**Generation phase**:

```rust
let progress = curr.n_decoded as f64 / curr.max_tokens as f64;
```

### Task boundary detection

`curr.id_task != prev.id_task` → new task started. Capture previous task's final numbers as "last task" stats (total tokens, duration, avg TPS).

---

## Dashboard State Model

```rust
pub struct DashboardState {
    pub server: ServerState,        // Idle, Loading, Running, Stopped, Error
    pub model_name: String,         // from /v1/models
    pub start_time: Option<Instant>,
    pub port: u16,

    // Prometheus cumulative (updated every 2s)
    pub prometheus: Option<PrometheusMetrics>,

    // History for charts (pushed each Prometheus poll)
    pub prompt_tokens_total_history: VecDeque<u64>,  // cumulative totals
    pub predicted_tokens_total_history: VecDeque<u64>,
    pub prompt_tps_history: VecDeque<f64>,           // throughput over time
    pub predicted_tps_history: VecDeque<f64>,

    // Slots (updated every 500ms from /slots)
    pub slots: HashMap<u32, Slot>,
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub id: u32,
    pub phase: SlotPhase,
    pub progress: f64,              // 0.0 to 1.0
    pub prompt_tps: f64,            // current from deltas
    pub gen_tps: f64,               // current from deltas
    pub n_prompt_tokens_processed: u64,
    pub n_decoded: u64,
    pub max_tokens: u64,            // generation limit from params
    pub n_ctx: u64,
    pub expected_prompt_total: Option<u64>,  // from log regex, fallback n_ctx
    pub n_prompt_tokens_cache: u64,
    pub last_task: Option<CompletedTask>,
    pub current_task_start: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct CompletedTask {
    pub n_prompt_tokens: u64,
    pub n_decoded: u64,
    pub duration: Duration,
    pub avg_prompt_tps: f64,
    pub avg_gen_tps: f64,
}
```

---

## Implementation Plan

### Phase 1: Foundation

1. **Event loop rewrite** (`main.rs`):
   - 100ms render tick (smooth feel)
   - 500ms `/slots` poll tick
   - 2s Prometheus poll tick (keep existing)
   - Add `ctrlc` handler for graceful terminal cleanup
   - Remove `EnableMouseCapture` (unused)

2. **Slots poller** (new `src/slots_poller.rs`):
   - `GET /slots` every 500ms via reqwest
   - Parse JSON into `SlotSnapshot`
   - Diff against previous snapshot → detect phase, TPS, progress
   - Send updates via `tokio::sync::mpsc` channel
   - Handle connection errors gracefully (server not ready yet)

3. **Model name fetch** (new or in poller):
   - `GET /v1/models` once at startup
   - Extract `models[0].name`

4. **Minimal log parser** (shrink `src/parser.rs`):
   - Keep only the `print_timing` regex for expected total detection
   - Remove all other regex patterns
   - Log lines still fed from tmux/script/inline modes, but only for this one pattern

### Phase 2: App state

5. **Rewrite `src/app.rs`**:
   - New `DashboardState` with slot HashMap, chart history VecDeques
   - `process_slot_update(slot_id, snapshot, prev_snapshot)` → updates phase/TPS/progress
   - `process_prometheus(metrics)` → updates cumulative stats + pushes to history
   - `process_log_line(line)` → only checks for print_timing regex
   - Task boundary detection on `id_task` change
   - History trimming: `if len > max { pop_front() }` (not while loop)

### Phase 3: Widgets

6. **Rewrite `src/widgets.rs`** — btop-style rendering:

   **Header bar:**
   ```
   ╭─┐¹llamaRtui┌───────────────────────────────────────────────┐12:17:35┌─╮
   │ Model Name  ● State  Uptime  Host:Port  N slots            │
   └─────────────┴───────────────────────────────────────────────┘
   ```
   - Use `BorderType::Rounded`
   - Truncate model name to fit
   - Colored state dot (● green/yellow/red)

   **Stats panel (Prometheus-driven):**
   - Two columns: Prompt stats | Gen stats
   - Label: Value format, right-aligned values
   - Humanized numbers: `52.8K`, `3.7K`, `1.2M`

   **Total tokens chart:**
   - Braille line chart, two datasets
   - ↑ Prompt (green) — cumulative `prompt_tokens_total` over time
   - ↓ Gen (blue) — cumulative `tokens_predicted_total` over time
   - Always rising lines, slope = speed
   - Y-axis: humanized (K, M)

   **Throughput chart:**
   - Braille line chart, two datasets
   - ↑ Prompt TPS (green) — `prompt_tps` gauge over time
   - ↓ Gen TPS (blue) — `predicted_tps` gauge over time
   - Spiky/fluctuating lines
   - Y-axis: tokens/s

   **Slots panel:**
   - One line per slot, sorted by ID
   - Format: `#N  PHASE ▓▓▓▓▓░░░░░░░░  XX%  TPS  tokens_info`
   - Phase colors: THINK=green, WRITE=blue, IDLE=dark gray, DONE=white
   - Progress bar: block chars (▓░), width fills available space
   - Prompt phase: shows `processed/expected` tokens
   - Gen phase: shows `decoded/max_tokens`
   - Idle with last task: shows last task summary
   - Cache indicator: ⚡ if `n_prompt_tokens_cache > 0`

   **Footer:**
   ```
   ╰┘↑ select ↓└┘1=stats└┘2=slots└┘3=split└┘Enter=start/stop└┘Esc=config└┘q=quit└─────────────────┘
   ```

7. **Color theme** (new `src/theme.rs`):
   - Define a cohesive palette (inspired by btop's default blue theme):
     - Background: default terminal
     - Borders: `Color::Rgb(100, 180, 255)` (light blue)
     - Prompt/THINK: `Color::Rgb(80, 250, 120)` (green)
     - Gen/WRITE: `Color::Rgb(100, 180, 255)` (blue)
     - Idle: `Color::DarkGray`
     - Error/Stopped: `Color::Rgb(255, 80, 80)` (red)
     - Warning/Loading: `Color::Rgb(255, 200, 60)` (yellow)
     - Text: `Color::White`
     - Dim text: `Color::Rgb(140, 140, 160)`
   - Centralize all color references here

### Phase 4: Polish

8. **Config editor** (`src/config_editor.rs`):
   - Add Up/Down arrow key navigation
   - Add Home/End for first/last field
   - Clamp dialog size to terminal dimensions
   - Keep overlay style (dim background + centered dialog)

9. **Graceful shutdown**:
   - Add `ctrlc` crate
   - Handler: set `app.running = false`, cleanup terminal
   - Also add `std::panic::set_hook` as backup terminal restore

10. **Empty states**:
    - No server: "Press Enter to start server" or "Configure with Esc"
    - No slots detected: "Waiting for server..."
    - No metrics: "Prometheus not available"

11. **Responsive layout**:
    - Slots panel scrolls if more slots than terminal height
    - Charts shrink gracefully on narrow terminals
    - Min terminal size warning if too small

### Phase 5: Cleanup

12. **Remove dead code**:
    - Old `parser.rs` regex patterns (keep only print_timing)
    - Old log event enum variants no longer used
    - Old `ServerState::Loading` if not needed
    - `#[allow(dead_code)]` annotations — either use or remove

13. **Add clippy + fmt**:
    ```toml
    [workspace.lints.clippy]
    all = "warn"
    pedantic = "warn"
    ```
    Run `cargo fmt` and `cargo clippy --fix`

14. **Narrow tokio features**:
    ```toml
    tokio = { version = "1", features = ["rt-multi-thread", "time", "macros", "sync"] }
    ```

15. **Add tests**:
    - Slot phase detection (idle → prompt → gen → done transitions)
    - Progress calculation (with and without cache)
    - TPS delta calculation
    - Task boundary detection

---

## File Structure (after rewrite)

```
src/
  main.rs          — CLI, event loop, poller spawns, terminal setup/cleanup
  app.rs           — App struct, DashboardState, Slot, state update logic
  slots_poller.rs  — GET /slots poller, snapshot diffing, phase detection
  prometheus.rs    — GET /metrics poller (move from main.rs)
  parser.rs        — Minimal: only print_timing regex for expected total
  widgets.rs       — All ratatui rendering (header, stats, charts, slots, footer)
  theme.rs         — Color palette constants
  config.rs        — AppConfig, load/save (keep as-is)
  config_editor.rs — Config modal (add arrow key nav)
  server.rs        — ServerHandle, tmux tailer (keep as-is)
```

---

## Key Decisions

1. **API-first, log-second**: `/slots` is ground truth. Logs only used for expected prompt total heuristic.
2. **Deltas over absolutes**: TPS and phase come from comparing consecutive snapshots, not parsing log lines.
3. **Cache-aware progress**: Subtract `n_prompt_tokens_cache` from both numerator and denominator.
4. **Fallback to n_ctx**: If no log line fires, use slot's `n_ctx` as the prompt progress denominator (will overshoot but gives visual feedback).
5. **100ms render, 500ms data**: Smooth feel without hammering the server.
6. **btop aesthetic**: Rounded borders, cohesive colors, metric cards, braille charts, block progress bars.
