//! Ratatui widgets — btop-style rendering.

use chrono::Timelike;
use ratatui::prelude::*;
use ratatui::symbols;
use ratatui::widgets::BorderType;
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Wrap};

use crate::app::{App, ServerState, SlotPhase, ViewMode};
use crate::theme::*;

// --- Main render ---

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(2), // header
        Constraint::Min(10),   // body
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_header(frame, app, chunks[0]);

    match app.view_mode {
        ViewMode::Stats => render_stats(frame, app, chunks[1]),
        ViewMode::Slots => render_slots_panel(frame, app, chunks[1]),
        ViewMode::Split => {
            let inner = Layout::vertical([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)])
                .split(chunks[1]);
            render_stats(frame, app, inner[0]);
            render_slots_panel(frame, app, inner[1]);
        }
        ViewMode::Logs => render_logs(frame, app, chunks[1]),
    }

    render_footer(frame, app, chunks[2]);
}

// --- Header bar ---

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let (state_color, state_label) = match app.server_state {
        ServerState::Unknown => (DIM, "Connecting"),
        ServerState::Running => (PROMPT, "Running"),
        ServerState::Error(ref msg) => (ERROR, msg.as_str()),
    };

    let model = if app.model_name.is_empty() {
        "N/A"
    } else {
        &app.model_name
    };

    let title = Line::from(vec![
        Span::styled("llama-hud", Style::default().fg(BORDER).bold()),
        Span::raw("  "),
        Span::raw(model),
        Span::raw("  "),
        Span::styled("●", Style::default().fg(state_color)),
        Span::styled(" ", Style::default().fg(state_color)),
        Span::styled(state_label, Style::default().fg(state_color)),
        Span::raw("  "),
        Span::styled("Uptime: ", Style::default().fg(DIM)),
        Span::raw(app.uptime_str()),
    ]);

    let slot_count = app.slots.len();
    let url = app.config.base_url();

    let info = Line::from(vec![
        Span::styled(url, Style::default().fg(DIM)),
        Span::raw("  "),
        Span::styled(format!("{} slots", slot_count), Style::default().fg(DIM)),
    ]);

    let now = chrono::Local::now();
    let time = Line::from(vec![Span::styled(
        format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second()),
        Style::default().fg(DIM),
    )]);

    let para_title = Paragraph::new(title).style(Style::default());
    let para_info = Paragraph::new(format!("{}  {}", info, " ".repeat(40)));

    let header_chunks =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);
    frame.render_widget(para_title, header_chunks[0]);

    let info_chunks =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(8)]).split(header_chunks[1]);
    frame.render_widget(para_info, info_chunks[0]);
    frame.render_widget(
        Paragraph::new(time).alignment(ratatui::layout::Alignment::Right),
        info_chunks[1],
    );
}

// --- Stats panel (Prometheus-driven) ---

fn render_stats(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(vec![Span::styled(
            "¹ STATS",
            Style::default().fg(BORDER).bold(),
        )])
        .border_style(border_style());

    let inner = block.inner(area);

    let chunks = Layout::vertical([
        Constraint::Length(4), // stats rows
        Constraint::Min(4),    // charts
    ])
    .split(inner);

    render_stats_rows(frame, app, chunks[0]);
    render_charts(frame, app, chunks[1]);

    frame.render_widget(block, area);
}

fn render_stats_rows(frame: &mut Frame, app: &App, area: Rect) {
    let prom = match &app.prometheus {
        Some(p) => p,
        None => {
            let msg = Paragraph::new(Span::styled(
                "Waiting for metrics...",
                Style::default().fg(DIM),
            ));
            frame.render_widget(msg, area);
            return;
        }
    };

    // Two columns: Prompt | Gen
    let col_chunks =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);

    let prompt_lines = vec![
        Line::from(vec![
            Span::styled("Total:   ", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tokens(prom.prompt_tokens_total),
                Style::default().fg(PROMPT),
            ),
            Span::styled(" tokens", Style::default().fg(DIM)),
        ]),
        Line::from(vec![
            Span::styled("Avg:     ", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tps(prom.prompt_tps),
                Style::default().fg(PROMPT),
            ),
        ]),
        Line::from(vec![
            Span::styled("Peak ctx:", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tokens(prom.n_tokens_max),
                Style::default().fg(TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("Active:  ", Style::default().fg(DIM)),
            Span::raw(format!("{}", prom.requests_processing)),
            Span::styled("  Queued: ", Style::default().fg(DIM)),
            Span::raw(format!("{}", prom.requests_deferred)),
        ]),
    ];

    let gen_lines = vec![
        Line::from(vec![
            Span::styled("Total:   ", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tokens(prom.predicted_tokens_total),
                Style::default().fg(GEN),
            ),
            Span::styled(" tokens", Style::default().fg(DIM)),
        ]),
        Line::from(vec![
            Span::styled("Avg:     ", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tps(prom.predicted_tps),
                Style::default().fg(GEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("Decodes: ", Style::default().fg(DIM)),
            Span::styled(
                format!("{}", prom.n_decode_total),
                Style::default().fg(TEXT),
            ),
        ]),
        Line::from(vec![
            Span::styled("Busy/slot:", Style::default().fg(DIM)),
            Span::styled(
                format!("{:.2}", prom.n_busy_slots_per_decode),
                Style::default().fg(TEXT),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(prompt_lines), col_chunks[0]);
    frame.render_widget(Paragraph::new(gen_lines), col_chunks[1]);
}

fn render_charts(frame: &mut Frame, app: &App, area: Rect) {
    let col_chunks =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);

    render_total_tokens_chart(frame, app, col_chunks[0]);
    render_throughput_chart(frame, app, col_chunks[1]);
}

fn render_total_tokens_chart(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled("↑↓ TOTAL TOKENS", Style::default().fg(BORDER)))
        .border_style(dim_style());

    let inner = block.inner(area);

    let prompt_hist: Vec<f64> =
        app.prompt_tokens_total_history.iter().map(|&v| v as f64).collect();
    let gen_hist: Vec<f64> =
        app.predicted_tokens_total_history.iter().map(|&v| v as f64).collect();

    if prompt_hist.len() < 2 || gen_hist.len() < 2 {
        frame.render_widget(block, area);
        return;
    }

    let fmt = |v: f64| -> String {
        let s = if v >= 1_000_000.0 {
            format!("{:.0}M", v / 1_000_000.0)
        } else if v >= 1_000.0 {
            format!("{:.0}K", v / 1_000.0)
        } else {
            format!("{:.0}", v)
        };
        format!("{:>5}", s)
    };

    render_dual_chart(frame, inner, &prompt_hist, &gen_hist, &fmt);
    frame.render_widget(block, area);
}

fn render_throughput_chart(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            "↑↓ THROUGHPUT (t/s)",
            Style::default().fg(BORDER),
        ))
        .border_style(dim_style());

    let inner = block.inner(area);

    let prompt_hist: Vec<f64> = app.prompt_tps_history.iter().copied().collect();
    let gen_hist: Vec<f64> = app.predicted_tps_history.iter().copied().collect();

    if prompt_hist.len() < 2 || gen_hist.len() < 2 {
        frame.render_widget(block, area);
        return;
    }

    let fmt = |v: f64| -> String {
        let s = if v >= 1000.0 {
            format!("{:.0}K", v / 1000.0)
        } else {
            format!("{:.0}", v)
        };
        format!("{:>5}", s)
    };

    render_dual_chart(frame, inner, &prompt_hist, &gen_hist, &fmt);
    frame.render_widget(block, area);
}

/// Single chart: prompt grows up from bottom, decoded grows down from top.
/// Each has its own independent scale.
fn render_dual_chart<'a, F>(
    frame: &mut Frame<'a>,
    area: Rect,
    prompt: &[f64],
    decoded: &[f64],
    fmt_y: &F,
) where
    F: Fn(f64) -> String,
{
    // Two charts stacked: decoded on top (inverted), prompt on bottom (normal)
    let chunks = Layout::vertical([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);

    let width = area.width as usize * 2;
    let n = std::cmp::min(std::cmp::min(width, prompt.len()), decoded.len());
    let pad = width - n;

    let prompt_recent: Vec<f64> = prompt.iter().skip(prompt.len() - n).copied().collect();
    let decoded_recent: Vec<f64> = decoded.iter().skip(decoded.len() - n).copied().collect();

    let prompt_max = prompt_recent.iter().copied().fold(0.0_f64, f64::max);
    let decoded_max = decoded_recent.iter().copied().fold(0.0_f64, f64::max);

    // Top chart: prompt (normal, grows up from center)
    if prompt_max >= 0.5 {
        let y_scale = nice_max(prompt_max);
        let data: Vec<(f64, f64)> = std::iter::repeat_n(0.0, pad)
            .chain(prompt_recent.iter().map(|v| v / prompt_max * y_scale))
            .enumerate()
            .map(|(i, v)| (i as f64, v))
            .collect();

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(PROMPT))
            .data(&data);

        let x_axis = Axis::default().style(dim_style()).bounds([0.0, width as f64]);
        let y_axis = Axis::default()
            .style(dim_style())
            .bounds([0.0, y_scale])
            .labels(["     ".to_string(), fmt_y(y_scale)]);

        let chart = Chart::new(vec![dataset])
            .x_axis(x_axis)
            .y_axis(y_axis)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::TOP)
                    .border_style(dim_style()),
            );
        frame.render_widget(chart, chunks[0]);
    }

    // Bottom chart: decoded (inverted, grows down from center)
    if decoded_max >= 0.5 {
        let y_scale = nice_max(decoded_max);
        let data: Vec<(f64, f64)> = std::iter::repeat_n(y_scale, pad)
            .chain(decoded_recent.iter().map(|v| y_scale - v / decoded_max * y_scale))
            .enumerate()
            .map(|(i, v)| (i as f64, v))
            .collect();

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(GEN))
            .data(&data);

        let x_axis = Axis::default().style(dim_style()).bounds([0.0, width as f64]);
        let y_axis = Axis::default()
            .style(dim_style())
            .bounds([0.0, y_scale])
            .labels([fmt_y(y_scale), "     ".to_string()]);

        let chart = Chart::new(vec![dataset])
            .x_axis(x_axis)
            .y_axis(y_axis)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_style(dim_style()),
            );
        frame.render_widget(chart, chunks[1]);
    }
}

// --- Slots panel ---

fn render_slots_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(vec![Span::styled(
            "² SLOTS",
            Style::default().fg(BORDER).bold(),
        )])
        .border_style(border_style());

    let inner = block.inner(area);

    // If a slot is selected, split for detail panel
    let has_selection = app.selected_slot.is_some() && app.slots.values().any(|s| s.selected);

    if has_selection && inner.width > 60 {
        let chunks =
            Layout::horizontal([Constraint::Ratio(2, 3), Constraint::Ratio(1, 3)]).split(inner);
        render_slots_list(frame, app, chunks[0]);
        render_slot_detail(frame, app, chunks[1]);
    } else {
        render_slots_list(frame, app, inner);
    }

    frame.render_widget(block, area);
}

fn render_slots_list(frame: &mut Frame, app: &App, area: Rect) {
    let mut slots: Vec<_> = app.slots.values().collect();
    slots.sort_by_key(|s| s.id);

    if slots.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "No slots detected — waiting for server...",
            Style::default().fg(DIM),
        ));
        frame.render_widget(msg, area);
        return;
    }

    let bar_width = std::cmp::max(10, (area.width as usize).saturating_sub(45));
    let mut lines: Vec<Line> = Vec::with_capacity(slots.len());

    for slot in &slots {
        lines.push(render_slot_line(slot, bar_width));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn render_slot_line(slot: &crate::app::Slot, bar_width: usize) -> Line<'_> {
    let mut parts: Vec<Span> = Vec::new();

    // ID
    let id_style = if slot.selected {
        Style::default().fg(HIGHLIGHT).bold()
    } else {
        Style::default().fg(DIM)
    };
    parts.push(Span::styled(format!("#{}  ", slot.id), id_style));

    match slot.phase {
        SlotPhase::Prompt => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Prompt),
                phase_style(&slot.phase),
            ));
            let fill = (slot.progress * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(fill);
            parts.push(Span::styled(
                BAR_FILL.repeat(fill),
                Style::default().fg(PROMPT),
            ));
            parts.push(Span::styled(
                BAR_EMPTY.repeat(empty),
                Style::default().fg(IDLE),
            ));
            parts.push(Span::raw(format!(" {:>5.0}%", slot.progress * 100.0)));
            parts.push(Span::styled(
                format!("  {}", crate::app::format_tps(slot.prompt_tps)),
                Style::default().fg(PROMPT),
            ));
            parts.push(Span::styled(
                format!("  ctx:{}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(DIM),
            ));
            if slot.n_prompt_tokens_cache > 0 {
                parts.push(Span::styled(
                    format!(
                        " ⚡{}",
                        crate::app::format_tokens(slot.n_prompt_tokens_cache)
                    ),
                    Style::default().fg(WARNING),
                ));
            }
        }
        SlotPhase::Generation => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Generation),
                phase_style(&slot.phase),
            ));
            let fill = (slot.progress * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(fill);
            parts.push(Span::styled(
                BAR_FILL.repeat(fill),
                Style::default().fg(GEN),
            ));
            parts.push(Span::styled(
                BAR_EMPTY.repeat(empty),
                Style::default().fg(IDLE),
            ));
            parts.push(Span::raw(format!(" {:>5.0}%", slot.progress * 100.0)));
            parts.push(Span::styled(
                format!("  {}", crate::app::format_tps(slot.gen_tps)),
                Style::default().fg(GEN),
            ));
            if slot.max_tokens > 0 {
                parts.push(Span::styled(
                    format!("  {}/{}", slot.n_decoded, slot.max_tokens),
                    Style::default().fg(DIM),
                ));
            }
            parts.push(Span::styled(
                format!("  ctx:{}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(DIM),
            ));
        }
        SlotPhase::Done => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Done),
                phase_style(&slot.phase),
            ));
            parts.push(Span::styled(
                BAR_FILL.repeat(bar_width),
                Style::default().fg(IDLE),
            ));
            if let Some(ref task) = slot.last_task {
                parts.push(Span::styled(
                    format!(
                        "  {} tok / {} t/s",
                        crate::app::format_tokens(task.n_decoded),
                        crate::app::format_tps(task.avg_gen_tps),
                    ),
                    Style::default().fg(DIM),
                ));
            }
            parts.push(Span::styled(
                format!("  ctx:{}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(DIM),
            ));
        }
        SlotPhase::Idle => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Idle),
                phase_style(&slot.phase),
            ));
            let ctx_ratio = if slot.n_ctx > 0 {
                slot.n_prompt_tokens as f64 / slot.n_ctx as f64
            } else {
                0.0
            };
            let fill = (ctx_ratio.min(1.0) * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(fill);
            parts.push(Span::styled(
                BAR_FILL.repeat(fill),
                Style::default().fg(DIM),
            ));
            parts.push(Span::styled(
                BAR_EMPTY.repeat(empty),
                Style::default().fg(IDLE),
            ));
            if let Some(ref task) = slot.last_task {
                parts.push(Span::styled(
                    format!("  last: {} tok", crate::app::format_tokens(task.n_decoded)),
                    Style::default().fg(DIM),
                ));
            }
            parts.push(Span::styled(
                format!("  ctx:{}/{}", crate::app::format_tokens(slot.n_prompt_tokens), crate::app::format_tokens(slot.n_ctx)),
                Style::default().fg(DIM),
            ));
        }
    }

    Line::from(parts)
}

// --- Slot detail panel ---

fn render_slot_detail(frame: &mut Frame, app: &App, area: Rect) {
    let slot = match app.selected_slot.and_then(|id| app.slots.get(&id)) {
        Some(s) => s,
        None => return,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            format!(" Slot #{}", slot.id),
            Style::default().fg(BORDER),
        ))
        .border_style(dim_style());

    let inner = block.inner(area);

    let lines = vec![
        Line::from(vec![
            Span::styled("Phase:    ", Style::default().fg(DIM)),
            Span::styled(format!("{}", slot.phase), phase_style(&slot.phase)),
        ]),
        Line::from(vec![
            Span::styled("Progress: ", Style::default().fg(DIM)),
            Span::raw(format!("{:.1}%", slot.progress * 100.0)),
        ]),
        Line::from(vec![
            Span::styled("Prompt:   ", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tps(slot.prompt_tps),
                Style::default().fg(PROMPT),
            ),
        ]),
        Line::from(vec![
            Span::styled("Gen:      ", Style::default().fg(DIM)),
            Span::styled(
                crate::app::format_tps(slot.gen_tps),
                Style::default().fg(GEN),
            ),
        ]),
        Line::from(vec![
            Span::styled("Ctx:      ", Style::default().fg(DIM)),
            Span::raw(crate::app::format_tokens(slot.n_ctx)),
        ]),
        Line::from(vec![
            Span::styled("Max gen:  ", Style::default().fg(DIM)),
            Span::raw(crate::app::format_tokens(slot.max_tokens)),
        ]),
        Line::from(vec![
            Span::styled("Decoded:  ", Style::default().fg(DIM)),
            Span::raw(crate::app::format_tokens(slot.n_decoded)),
        ]),
        Line::from(vec![
            Span::styled("Cache:    ", Style::default().fg(DIM)),
            Span::raw(crate::app::format_tokens(slot.n_prompt_tokens_cache)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "↑↓ select  Esc deselect",
            Style::default().fg(DIM),
        )]),
    ];

    let para = Paragraph::new(lines);
    frame.render_widget(para, inner);
    frame.render_widget(block, area);
}

// --- Logs view ---

fn render_logs(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(vec![Span::styled(
            "³ LOG",
            Style::default().fg(BORDER).bold(),
        )])
        .border_style(border_style());

    let inner = block.inner(area);

    if app.logs.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "No logs — configure tmux_session in config to enable log tailing.",
            Style::default().fg(DIM),
        ));
        frame.render_widget(msg, inner);
        frame.render_widget(block, area);
        return;
    }

    let lines: Vec<Line> = app
        .logs
        .iter()
        .map(|entry| Line::from(vec![Span::styled(entry.as_str(), Style::default().fg(DIM))]))
        .collect();

    let visible_count = inner.height as usize;
    let visible = if lines.len() > visible_count {
        let start = lines.len() - visible_count;
        lines.iter().skip(start).cloned().collect::<Vec<_>>()
    } else {
        lines
    };

    let para = Paragraph::new(visible).wrap(Wrap { trim: true });
    frame.render_widget(para, inner);
    frame.render_widget(block, area);
}

// --- Footer ---

fn render_footer(frame: &mut Frame, _app: &App, area: Rect) {
    let text =
        Line::raw(" 1=stats  2=slots  3=split  4=logs  ↑↓=select slot  Esc=deselect  q=quit");
    let para = Paragraph::new(text).style(dim_style());
    frame.render_widget(para, area);
}

// --- Helpers ---

fn nice_max(value: f64) -> f64 {
    if value <= 0.0 {
        return 1.0;
    }
    let magnitude = 10_f64.powf(value.log10().floor());
    let normalized = value / magnitude;
    let steps = [1.0, 2.0, 5.0, 10.0];
    let step = steps
        .iter()
        .find(|&&s| s >= normalized)
        .copied()
        .unwrap_or(10.0);
    step * magnitude
}

