//! Ratatui widgets — btop-style rendering.

use chrono::Timelike;
use ratatui::prelude::*;
use ratatui::widgets::Padding;
use ratatui::symbols;
use ratatui::widgets::BorderType;
use ratatui::widgets::{Axis, Block, Borders, Chart, Dataset, GraphType, Paragraph, Wrap};

use crate::app::{App, ConfigField, ServerState, SlotPhase};
use crate::theme::{Theme, BAR_EMPTY, BAR_FILL};

// --- Main render ---

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1), // header
        Constraint::Min(0),    // body
        Constraint::Length(1), // footer
    ])
    .split(area);

    render_header(frame, app, chunks[0]);

    // Build layout from toggled views
    let mut visible = vec![];
    if app.show_stats {
        visible.push("stats");
    }
    if app.show_slots {
        visible.push("slots");
    }
    if app.show_logs {
        visible.push("logs");
    }

    // Ensure at least one view is visible
    if visible.is_empty() {
        render_stats(frame, app, chunks[1]);
    } else {
        let constraints: Vec<ratatui::layout::Constraint> = visible
            .iter()
            .map(|v| match *v {
                "slots" => Constraint::Length(12),
                "stats" => Constraint::Min(14),
                "logs" => Constraint::Min(4),
                _ => Constraint::Min(10),
            })
            .collect();
        let inner = Layout::vertical(constraints).split(chunks[1]);
        for (i, view) in visible.iter().enumerate() {
            match *view {
                "stats" => render_stats(frame, app, inner[i]),
                "slots" => render_slots_panel(frame, app, inner[i]),
                "logs" => render_logs(frame, app, inner[i]),
                _ => {}
            }
        }
    }

    render_footer(frame, app, chunks[2]);

    // Config popup overlay
    if app.show_config {
        render_config_popup(frame, app, area);
    }
}

// --- Header bar ---

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let (state_color, state_label) = match app.server_state {
        ServerState::Unknown => (app.theme.dim, "Connecting"),
        ServerState::Running => (app.theme.prompt, "Running"),
        ServerState::Error(ref msg) => (app.theme.error, msg.as_str()),
    };

    let model = if app.model_name.is_empty() {
        "N/A"
    } else {
        &app.model_name
    };

    let now = chrono::Local::now();
    let time = format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second());

    let pulse = match app.last_metrics_time {
        Some(t) if t.elapsed() < std::time::Duration::from_secs(1) => "◉",
        _ => "●",
    };

    let left = Line::from(vec![
        Span::styled("llama-hud", Style::default().fg(app.theme.border).bold()),
        Span::raw("  "),
        Span::raw(model),
        Span::raw("  "),
        Span::styled(pulse, Style::default().fg(state_color)),
        Span::styled(" ", Style::default().fg(state_color)),
        Span::styled(state_label, Style::default().fg(state_color)),
        Span::raw("  "),
        Span::styled(app.config.base_url(), Style::default().fg(app.theme.dim)),
    ]);

    let right = Line::from(vec![
        Span::styled("Uptime: ", Style::default().fg(app.theme.dim)),
        Span::raw(app.uptime_str()),
        Span::raw("  "),
        Span::styled(time, Style::default().fg(app.theme.dim)),
    ]);

    let chunks =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(20)]).split(area);
    frame.render_widget(Paragraph::new(left).style(Style::default()), chunks[0]);
    frame.render_widget(
        Paragraph::new(right).alignment(ratatui::layout::Alignment::Right),
        chunks[1],
    );
}

// --- Stats panel (Prometheus-driven) ---

fn render_stats(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(vec![Span::styled(
            "¹ STATS",
            Style::default().fg(app.theme.border).bold(),
        )])
        .border_style(app.theme.border_style());

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
    // Show error if present
    if let Some(ref err) = app.error_message {
        let msg = Paragraph::new(Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(app.theme.error)),
            Span::styled(err.as_str(), Style::default().fg(app.theme.error).bold()),
        ])).alignment(ratatui::layout::Alignment::Center);
        let center = Layout::vertical([
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area)[1];
        frame.render_widget(msg, center);
        return;
    }

    let prom = match &app.prometheus {
        Some(p) => p,
        None => {
            let msg = Paragraph::new(Span::styled(
                "Waiting for metrics...",
                Style::default().fg(app.theme.dim),
            ));
            frame.render_widget(msg, area);
            return;
        }
    };

    // Two columns: Totals | Throughput
    let col_chunks =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);

    let total_lines = vec![
        Line::from(vec![
            Span::styled("Prompt:  ", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tokens(prom.prompt_tokens_total),
                Style::default().fg(app.theme.prompt),
            ),
            Span::styled(" tokens", Style::default().fg(app.theme.dim)),
        ]),
        Line::from(vec![
            Span::styled("Gen:     ", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tokens(prom.predicted_tokens_total),
                Style::default().fg(app.theme.generation),
            ),
            Span::styled(" tokens", Style::default().fg(app.theme.dim)),
        ]),
        Line::from(vec![
            Span::styled("Peak ctx:", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tokens(prom.n_tokens_max),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("Active:  ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{}", prom.requests_processing)),
            Span::styled("  Queued: ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{}", prom.requests_deferred)),
        ]),
    ];

    let throughput_lines = vec![
        Line::from(vec![
            Span::styled("Prompt:  ", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tps(prom.prompt_tps),
                Style::default().fg(app.theme.prompt),
            ),
        ]),
        Line::from(vec![
            Span::styled("Gen:     ", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tps(prom.predicted_tps),
                Style::default().fg(app.theme.generation),
            ),
        ]),
        Line::from(vec![
            Span::styled("Decodes: ", Style::default().fg(app.theme.dim)),
            Span::styled(
                format!("{}", prom.n_decode_total),
                Style::default().fg(app.theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("Busy/slot:", Style::default().fg(app.theme.dim)),
            Span::styled(
                format!("{:.2}", prom.n_busy_slots_per_decode),
                Style::default().fg(app.theme.text),
            ),
        ]),
    ];

    frame.render_widget(Paragraph::new(total_lines), col_chunks[0]);
    frame.render_widget(Paragraph::new(throughput_lines), col_chunks[1]);
}

fn render_charts(frame: &mut Frame, app: &App, area: Rect) {
    let col_chunks =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(area);

    render_total_tokens_chart(frame, app, col_chunks[0]);
    render_throughput_chart(frame, app, col_chunks[1]);
}

fn render_total_tokens_chart(frame: &mut Frame, app: &App, area: Rect) {
    let prompt_hist: Vec<f64> =
        app.prompt_tokens_total_history.iter().map(|&v| v as f64).collect();
    let gen_hist: Vec<f64> =
        app.predicted_tokens_total_history.iter().map(|&v| v as f64).collect();

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

    render_dual_chart(frame, area, &prompt_hist, &gen_hist, &fmt, &app.theme, "TOTAL TOKENS");
}

fn render_throughput_chart(frame: &mut Frame, app: &App, area: Rect) {
    let prompt_hist: Vec<f64> = app.prompt_tps_history.iter().copied().collect();
    let gen_hist: Vec<f64> = app.predicted_tps_history.iter().copied().collect();

    let fmt = |v: f64| -> String {
        let s = if v >= 1000.0 {
            format!("{:.0}K", v / 1000.0)
        } else {
            format!("{:.0}", v)
        };
        format!("{:>5}", s)
    };

    render_dual_chart(frame, area, &prompt_hist, &gen_hist, &fmt, &app.theme, "THROUGHPUT (t/s)");
}

/// Single chart: prompt grows up from bottom, decoded grows down from top.
/// Each has its own independent scale.
fn render_dual_chart<'a, F>(
    frame: &mut Frame<'a>,
    area: Rect,
    prompt: &[f64],
    decoded: &[f64],
    fmt_y: &F,
    theme: &Theme,
    title: &str,
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

    // Top chart: prompt (grows up from bottom)
    if prompt_max >= 1.0 {
        let y_scale = nice_max(prompt_max);
        let data: Vec<(f64, f64)> = std::iter::repeat_n(0.0, pad)
            .chain(prompt_recent.iter().copied())
            .enumerate()
            .map(|(i, v)| (i as f64, v))
            .collect();

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.prompt))
            .data(&data);

        // Zero line at bottom
        let center_data: Vec<(f64, f64)> = (0..width).map(|i| (i as f64, 0.0)).collect();
        let center_dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.dim))
            .data(&center_data);

        let x_axis = Axis::default().style(theme.dim_style()).bounds([0.0, width as f64]);
        let y_axis = Axis::default()
            .style(theme.dim_style())
            .bounds([0.0, y_scale])
            .labels(["     ".to_string(), fmt_y(y_scale)]);

        let chart = Chart::new(vec![center_dataset, dataset])
            .x_axis(x_axis)
            .y_axis(y_axis)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::TOP)
                    .title(Span::styled(title, Style::default().fg(theme.border)))
                    .border_style(theme.dim_style()),
            );
        frame.render_widget(chart, chunks[0]);
    }

    // Bottom chart: decoded (inverted, grows down from top)
    if decoded_max >= 1.0 {
        let y_scale = nice_max(decoded_max);
        let data: Vec<(f64, f64)> = std::iter::repeat_n(y_scale, pad)
            .chain(decoded_recent.iter().map(|v| y_scale - v))
            .enumerate()
            .map(|(i, v)| (i as f64, v))
            .collect();

        let dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.generation))
            .data(&data);

        // Zero line at top (inverted chart)
        let center_data: Vec<(f64, f64)> = (0..width).map(|i| (i as f64, y_scale)).collect();
        let center_dataset = Dataset::default()
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(theme.dim))
            .data(&center_data);

        let x_axis = Axis::default().style(theme.dim_style()).bounds([0.0, width as f64]);
        let y_axis = Axis::default()
            .style(theme.dim_style())
            .bounds([0.0, y_scale])
            .labels([fmt_y(y_scale), "     ".to_string()]);

        let chart = Chart::new(vec![center_dataset, dataset])
            .x_axis(x_axis)
            .y_axis(y_axis)
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::BOTTOM)
                    .border_style(theme.dim_style()),
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
            Style::default().fg(app.theme.border).bold(),
        )])
        .border_style(app.theme.border_style());

    let inner = block.inner(area);

    // Show detail panel if toggled on or a slot is selected
    let show_detail = app.show_slot_detail && !app.slots.is_empty();

    if show_detail && inner.width > 60 {
        let chunks =
            Layout::horizontal([Constraint::Min(0), Constraint::Length(36)]).split(inner);
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
            Style::default().fg(app.theme.dim),
        ));
        frame.render_widget(msg, area);
        return;
    }

    let bar_width = std::cmp::max(10, (area.width as usize).saturating_sub(45));
    let mut lines: Vec<Line> = Vec::with_capacity(slots.len());

    for slot in &slots {
        lines.push(render_slot_line(slot, bar_width, &app.theme));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn render_slot_line<'a>(slot: &'a crate::app::Slot, bar_width: usize, theme: &'a Theme) -> Line<'a> {
    let mut parts: Vec<Span> = Vec::new();

    // ID
    let id_style = if slot.selected {
        Style::default().fg(theme.highlight).bold()
    } else {
        Style::default().fg(theme.dim)
    };
    parts.push(Span::styled(format!("#{}  ", slot.id), id_style));

    match slot.phase {
        SlotPhase::Prompt => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Prompt),
                theme.phase_style(&slot.phase),
            ));
            let fill = (slot.progress * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(fill);
            parts.push(Span::styled(
                BAR_FILL.repeat(fill),
                Style::default().fg(theme.prompt),
            ));
            parts.push(Span::styled(
                BAR_EMPTY.repeat(empty),
                Style::default().fg(theme.idle),
            ));
            parts.push(Span::styled(
                format!(" {}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(theme.dim),
            ));
            parts.push(Span::raw(format!(" {:>4.0}%", slot.progress * 100.0)));
            parts.push(Span::styled(
                format!(" {}", crate::app::format_tps(slot.prompt_tps)),
                Style::default().fg(theme.prompt),
            ));
            if slot.n_prompt_tokens_cache > 0 {
                parts.push(Span::styled(
                    format!(
                        " ⚡{}",
                        crate::app::format_tokens(slot.n_prompt_tokens_cache)
                    ),
                    Style::default().fg(theme.warning),
                ));
            }
        }
        SlotPhase::Generation => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Generation),
                theme.phase_style(&slot.phase),
            ));
            let fill = (slot.progress * bar_width as f64) as usize;
            let empty = bar_width.saturating_sub(fill);
            parts.push(Span::styled(
                BAR_FILL.repeat(fill),
                Style::default().fg(theme.generation),
            ));
            parts.push(Span::styled(
                BAR_EMPTY.repeat(empty),
                Style::default().fg(theme.idle),
            ));
            parts.push(Span::styled(
                format!(" {}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(theme.dim),
            ));
            parts.push(Span::raw(format!(" {:>4.0}%", slot.progress * 100.0)));
            parts.push(Span::styled(
                format!(" {}", crate::app::format_tps(slot.gen_tps)),
                Style::default().fg(theme.generation),
            ));
        }
        SlotPhase::Done => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Done),
                theme.phase_style(&slot.phase),
            ));
            parts.push(Span::styled(
                BAR_FILL.repeat(bar_width),
                Style::default().fg(theme.idle),
            ));
            if let Some(ref task) = slot.last_task {
                parts.push(Span::styled(
                    format!(
                        " {} tok / {} t/s",
                        crate::app::format_tokens(task.n_decoded),
                        crate::app::format_tps(task.avg_gen_tps),
                    ),
                    Style::default().fg(theme.dim),
                ));
            }
            parts.push(Span::styled(
                format!(" {}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(theme.dim),
            ));
        }
        SlotPhase::Idle => {
            parts.push(Span::styled(
                format!("{}", SlotPhase::Idle),
                theme.phase_style(&slot.phase),
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
                Style::default().fg(theme.dim),
            ));
            parts.push(Span::styled(
                BAR_EMPTY.repeat(empty),
                Style::default().fg(theme.idle),
            ));
            if let Some(ref task) = slot.last_task {
                parts.push(Span::styled(
                    format!(" last: {} tok", crate::app::format_tokens(task.n_decoded)),
                    Style::default().fg(theme.dim),
                ));
            }
            parts.push(Span::styled(
                format!(" {}", crate::app::format_tokens(slot.n_prompt_tokens)),
                Style::default().fg(theme.dim),
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
            Style::default().fg(app.theme.border),
        ))
        .border_style(app.theme.dim_style());

    let inner = block.inner(area);

    let col_chunks =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(inner);

    let status_lines = vec![
        Line::from(vec![
            Span::styled("Phase:   ", Style::default().fg(app.theme.dim)),
            Span::styled(format!("{}", slot.phase), app.theme.phase_style(&slot.phase)),
        ]),
        Line::from(vec![
            Span::styled("Prog:    ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{:.0}%", slot.progress * 100.0)),
        ]),
        Line::from(vec![
            Span::styled("Prompt:  ", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tps(slot.prompt_tps),
                Style::default().fg(app.theme.prompt),
            ),
        ]),
        Line::from(vec![
            Span::styled("Gen:     ", Style::default().fg(app.theme.dim)),
            Span::styled(
                crate::app::format_tps(slot.gen_tps),
                Style::default().fg(app.theme.generation),
            ),
        ]),
        Line::from(vec![
            Span::styled("Cur ctx: ", Style::default().fg(app.theme.dim)),
            Span::raw(crate::app::format_tokens(slot.n_prompt_tokens)),
        ]),
        Line::from(vec![
            Span::styled("Max ctx: ", Style::default().fg(app.theme.dim)),
            Span::raw(crate::app::format_tokens(slot.n_ctx)),
        ]),
        Line::from(vec![
            Span::styled("Dec:     ", Style::default().fg(app.theme.dim)),
            Span::raw(crate::app::format_tokens(slot.n_decoded)),
        ]),
        Line::from(vec![
            Span::styled("Cache:   ", Style::default().fg(app.theme.dim)),
            Span::raw(crate::app::format_tokens(slot.n_prompt_tokens_cache)),
        ]),
    ];

    let param_lines = vec![
        Line::from(vec![
            Span::styled("Temp:   ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{:.2}", slot.params.temperature)),
        ]),
        Line::from(vec![
            Span::styled("TopP:   ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{:.2}", slot.params.top_p)),
        ]),
        Line::from(vec![
            Span::styled("TopK:   ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{}", slot.params.top_k)),
        ]),
        Line::from(vec![
            Span::styled("MinP:   ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{:.3}", slot.params.min_p)),
        ]),
        Line::from(vec![
            Span::styled("Rep:    ", Style::default().fg(app.theme.dim)),
            Span::raw(format!(
                "{:.2}-{}n",
                slot.params.repeat_penalty, slot.params.repeat_last_n
            )),
        ]),
        Line::from(vec![
            Span::styled("Pres:   ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{:.2}", slot.params.presence_penalty)),
        ]),
        Line::from(vec![
            Span::styled("Freq:   ", Style::default().fg(app.theme.dim)),
            Span::raw(format!("{:.2}", slot.params.frequency_penalty)),
        ]),
        Line::from(vec![
            Span::styled("Gen lim:", Style::default().fg(app.theme.dim)),
            Span::raw(crate::app::format_tokens(slot.max_tokens)),
        ]),
    ];

    frame.render_widget(Paragraph::new(status_lines), col_chunks[0]);
    frame.render_widget(Paragraph::new(param_lines), col_chunks[1]);
    frame.render_widget(block, area);
}

// --- Logs view ---

fn render_logs(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(vec![Span::styled(
            "³ LOG",
            Style::default().fg(app.theme.border).bold(),
        )])
        .border_style(app.theme.border_style());

    let inner = block.inner(area);

    if app.logs.is_empty() {
        let msg = Paragraph::new(Span::styled(
            "No logs — configure tmux_session in config to enable log tailing.",
            Style::default().fg(app.theme.dim),
        ));
        frame.render_widget(msg, inner);
        frame.render_widget(block, area);
        return;
    }

    let visible_count = inner.height as usize;
    let lines: Vec<Line> = app
        .logs
        .iter()
        .map(|entry| Line::from(vec![Span::styled(entry.as_str(), Style::default().fg(app.theme.dim))]))
        .collect();

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

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let st = if app.show_stats { "on" } else { "off" };
    let sl = if app.show_slots { "on" } else { "off" };
    let dt = if app.show_slot_detail { "on" } else { "off" };
    let lg = if app.show_logs { "on" } else { "off" };
    let text = Line::raw(format!(
        " 1=stats({})  2=slots({})  3=detail({})  4=logs({})  ↑↓=select  q=quit",
        st, sl, dt, lg
    ));
    let para = Paragraph::new(text).style(app.theme.dim_style());
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

// --- Config popup ---

fn render_config_popup(frame: &mut Frame, app: &App, area: Rect) {
    // Center the popup
    let popup_width = 52;
    let popup_height = 14;
    let popup_area = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(popup_width),
        Constraint::Min(0),
    ])
    .split(area)[1];
    let popup_area = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(popup_height),
        Constraint::Min(0),
    ])
    .split(popup_area)[1];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(
            " Config (~/.config/llama-hud/config.json)",
            Style::default().fg(app.theme.border).bold(),
        ))
        .border_style(Style::default().fg(app.theme.border))
        .padding(Padding::horizontal(1));

    use ratatui::widgets::Clear;
    frame.render_widget(Clear, popup_area);
    frame.render_widget(&block, popup_area);

    let inner = block.inner(popup_area);

    // Split: content area on top, help line fixed at bottom
    let content_area = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(inner);

    let fields = [
        ("Server URL", &app.config_edits.url, ConfigField::Url),
        ("Tmux session", &app.config_edits.tmux_session, ConfigField::TmuxSession),
        ("Slots poll (ms)", &app.config_edits.slots_poll_ms, ConfigField::SlotsPollMs),
        ("Metrics poll (ms)", &app.config_edits.metrics_poll_ms, ConfigField::MetricsPollMs),
        ("Chart history", &app.config_edits.chart_history, ConfigField::ChartHistory),
        ("Theme", &app.config_edits.theme, ConfigField::Theme),
    ];

    let blink = (chrono::Utc::now().nanosecond() / 500_000_000).is_multiple_of(2);
    let cursor = if blink { Span::styled("█", Style::default().fg(app.theme.prompt)) } else { Span::raw(" ") };
    let theme_cursor = Span::styled(" ← →", Style::default().fg(app.theme.prompt));

    let lines: Vec<Line> = fields
        .iter()
        .map(|(label, value, field)| {
            let is_active = *field == app.config_field;
            let value_color = if is_active { app.theme.prompt } else { app.theme.dim };
            let prefix = if is_active { "> " } else { "  " };
            let mut spans = vec![
                Span::styled(format!("{}{}: ", prefix, label), Style::default().fg(app.theme.text)),
                Span::styled(value.as_str(), Style::default().fg(value_color)),
            ];
            if is_active {
                spans.push(if *field == ConfigField::Theme { theme_cursor.clone() } else { cursor.clone() });
            }
            Line::from(spans)
        })
        .collect();

    let tooltip = match app.config_field {
        ConfigField::Url => "Base URL of the llama-server HTTP API (e.g. http://127.0.0.1:8080)",
        ConfigField::TmuxSession => "Tmux session name to tail logs from (leave empty to disable)",
        ConfigField::SlotsPollMs => "How often to poll /slots endpoint in milliseconds",
        ConfigField::MetricsPollMs => "How often to poll /metrics endpoint in milliseconds",
        ConfigField::ChartHistory => "Number of data points to keep in chart history",
        ConfigField::Theme => "← → cycle: default, nord, gruvbox, catppuccin, dracula, tokyo-night, kanagawa, onedark, horizon, flexoki",
    };

    let content: Vec<Line> = lines
        .into_iter()
        .chain(std::iter::once(Line::default()))
        .chain(std::iter::once(Line::from(Span::styled(
            tooltip,
            Style::default().fg(app.theme.idle),
        ))))
        .collect();

    let paragraph = Paragraph::new(content).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, content_area[0]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("  ↑↓ navigate  ", Style::default().fg(app.theme.border)),
        Span::styled("Enter save  ", Style::default().fg(app.theme.border)),
        Span::styled("Esc cancel", Style::default().fg(app.theme.border)),
    ]));
    frame.render_widget(help, content_area[1]);
    frame.render_widget(block, popup_area);
}

