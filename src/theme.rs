//! Color palette — btop-inspired defaults, inherits terminal background.

use ratatui::style::{Color, Style};

// Primary colors
pub const BORDER: Color = Color::Rgb(100, 180, 255); // light blue
pub const PROMPT: Color = Color::Rgb(80, 250, 120); // green (input/think)
pub const GEN: Color = Color::Rgb(100, 180, 255); // blue (output/write)
pub const IDLE: Color = Color::DarkGray;
pub const ERROR: Color = Color::Rgb(255, 80, 80); // red
pub const WARNING: Color = Color::Rgb(255, 200, 60); // yellow
pub const TEXT: Color = Color::White;
pub const DIM: Color = Color::Rgb(140, 140, 160);
pub const HIGHLIGHT: Color = Color::Rgb(255, 200, 60); // yellow (selected slot)

// Bar characters
pub const BAR_FILL: &str = "█";
pub const BAR_EMPTY: &str = "░";

// Helper styles
pub fn border_style() -> Style {
    Style::default().fg(BORDER)
}

pub fn dim_style() -> Style {
    Style::default().fg(DIM)
}

pub fn phase_style(phase: &crate::app::SlotPhase) -> Style {
    match phase {
        crate::app::SlotPhase::Idle => Style::default().fg(IDLE),
        crate::app::SlotPhase::Prompt => Style::default().fg(PROMPT).bold(),
        crate::app::SlotPhase::Generation => Style::default().fg(GEN).bold(),
        crate::app::SlotPhase::Done => Style::default().fg(TEXT).bold(),
    }
}
