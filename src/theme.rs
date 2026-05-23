//! Color palettes — named themes selectable via config.

use ratatui::style::{Color, Style};

use crate::app::SlotPhase;

/// A complete color palette for the dashboard.
#[derive(Debug, Clone)]
pub struct Theme {
    pub border: Color,
    pub prompt: Color,
    pub generation: Color,
    pub idle: Color,
    pub error: Color,
    pub warning: Color,
    pub text: Color,
    pub dim: Color,
    pub highlight: Color,
}

impl Theme {
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.dim)
    }

    pub fn phase_style(&self, phase: &SlotPhase) -> Style {
        match phase {
            SlotPhase::Idle => Style::default().fg(self.idle),
            SlotPhase::Prompt => Style::default().fg(self.prompt).bold(),
            SlotPhase::Generation => Style::default().fg(self.generation).bold(),
            SlotPhase::Done => Style::default().fg(self.text).bold(),
        }
    }
}

/// Built-in themes. Add more here.
pub fn get_theme(name: &str) -> Theme {
    match name {
        "default" | "" => default_theme(),
        "nord" => nord_theme(),
        "gruvbox" => gruvbox_theme(),
        "catppuccin" => catppuccin_theme(),
        "dracula" => dracula_theme(),
        "tokyo-night" => tokyo_night_theme(),
        "kanagawa" => kanagawa_wave_theme(),
        "onedark" => onedark_theme(),
        "horizon" => horizon_theme(),
        "flexoki" => flexoki_dark_theme(),
        _ => default_theme(),
    }
}

/// Default — uses terminal ANSI colors, adapts to user's terminal theme.
fn default_theme() -> Theme {
    Theme {
        border: Color::Blue,
        prompt: Color::Green,
        generation: Color::Blue,
        idle: Color::DarkGray,
        error: Color::Red,
        warning: Color::Yellow,
        text: Color::White,
        dim: Color::DarkGray,
        highlight: Color::LightBlue,
    }
}

/// Nord — arctic, north-blue color palette.
fn nord_theme() -> Theme {
    Theme {
        border: Color::Rgb(134, 166, 231),   // nord9
        prompt: Color::Rgb(163, 190, 140),   // nord13
        generation: Color::Rgb(134, 166, 231),      // nord9
        idle: Color::Rgb(104, 117, 142),     // nord4
        error: Color::Rgb(191, 97, 106),     // nord11
        warning: Color::Rgb(235, 203, 139),  // nord16
        text: Color::Rgb(216, 222, 233),     // nord0
        dim: Color::Rgb(129, 142, 174),      // nord5
        highlight: Color::Rgb(235, 203, 139), // nord16
    }
}

/// Gruvbox — retro groove, warm tones.
fn gruvbox_theme() -> Theme {
    Theme {
        border: Color::Rgb(132, 164, 197),   // blue
        prompt: Color::Rgb(184, 174, 123),   // yellow
        generation: Color::Rgb(132, 164, 197),      // blue
        idle: Color::Rgb(146, 131, 116),     // grey
        error: Color::Rgb(204, 90, 90),      // red
        warning: Color::Rgb(212, 142, 114),  // orange
        text: Color::Rgb(235, 219, 178),     // fg
        dim: Color::Rgb(165, 142, 114),      // grey
        highlight: Color::Rgb(212, 142, 114), // orange
    }
}

/// Catppuccin Mocha — soft purples and pastels.
fn catppuccin_theme() -> Theme {
    Theme {
        border: Color::Rgb(137, 180, 250),   // blue
        prompt: Color::Rgb(166, 227, 161),   // green
        generation: Color::Rgb(137, 180, 250),      // blue
        idle: Color::Rgb(109, 115, 141),     // overlay0
        error: Color::Rgb(243, 139, 168),    // pink
        warning: Color::Rgb(249, 226, 175),  // peach
        text: Color::Rgb(205, 214, 244),     // text
        dim: Color::Rgb(125, 131, 164),      // overlay1
        highlight: Color::Rgb(249, 226, 175), // peach
    }
}

/// Dracula — purple/green, iconic dark palette.
fn dracula_theme() -> Theme {
    Theme {
        border: Color::Rgb(189, 147, 249),   // purple
        prompt: Color::Rgb(80, 250, 123),    // green
        generation: Color::Rgb(139, 233, 253), // cyan
        idle: Color::Rgb(68, 71, 90),        // gutter
        error: Color::Rgb(255, 85, 85),      // red
        warning: Color::Rgb(255, 184, 108),  // orange
        text: Color::Rgb(248, 248, 242),     // fg
        dim: Color::Rgb(98, 114, 164),       // comment
        highlight: Color::Rgb(255, 121, 198), // pink
    }
}

/// Tokyo Night — modern blue-heavy, clean and crisp.
fn tokyo_night_theme() -> Theme {
    Theme {
        border: Color::Rgb(86, 95, 137),     // blue0
        prompt: Color::Rgb(158, 206, 106),   // green
        generation: Color::Rgb(125, 207, 255), // lightblue
        idle: Color::Rgb(86, 95, 137),       // blue0
        error: Color::Rgb(247, 118, 142),    // red
        warning: Color::Rgb(224, 175, 104),  // yellow
        text: Color::Rgb(207, 201, 194),     // fg
        dim: Color::Rgb(65, 72, 104),        // blue1
        highlight: Color::Rgb(125, 207, 255), // lightblue
    }
}

/// Kanagawa Wave — muted, warm, Japanese-inspired.
fn kanagawa_wave_theme() -> Theme {
    Theme {
        border: Color::Rgb(114, 113, 105),   // darkrock
        prompt: Color::Rgb(152, 187, 108),   // olive
        generation: Color::Rgb(127, 180, 202), // waveblue
        idle: Color::Rgb(114, 113, 105),     // darkrock
        error: Color::Rgb(232, 36, 36),      // red
        warning: Color::Rgb(220, 165, 97),   // amber
        text: Color::Rgb(220, 215, 186),     // main
        dim: Color::Rgb(84, 84, 96),         // grey1
        highlight: Color::Rgb(195, 64, 67),  // darkred
    }
}

/// One Dark — VS Code default, familiar and balanced.
fn onedark_theme() -> Theme {
    Theme {
        border: Color::Rgb(92, 99, 112),     // dark
        prompt: Color::Rgb(152, 195, 121),   // green
        generation: Color::Rgb(97, 175, 239), // blue
        idle: Color::Rgb(92, 99, 112),       // dark
        error: Color::Rgb(224, 108, 117),    // red
        warning: Color::Rgb(229, 192, 123),  // yellow
        text: Color::Rgb(171, 178, 191),     // fg
        dim: Color::Rgb(75, 82, 99),         // darker
        highlight: Color::Rgb(97, 175, 239), // blue
    }
}

/// Horizon — purple/teal, sunset-inspired.
fn horizon_theme() -> Theme {
    Theme {
        border: Color::Rgb(184, 119, 219),   // purple
        prompt: Color::Rgb(39, 215, 150),    // green
        generation: Color::Rgb(37, 178, 188), // teal
        idle: Color::Rgb(39, 46, 51),        // dark
        error: Color::Rgb(233, 86, 120),     // red
        warning: Color::Rgb(250, 194, 154),  // peach
        text: Color::Rgb(248, 248, 242),     // fg
        dim: Color::Rgb(184, 119, 219),      // purple
        highlight: Color::Rgb(184, 119, 219), // purple
    }
}

/// Flexoki Dark — accessibility-focused, warm earth tones.
fn flexoki_dark_theme() -> Theme {
    Theme {
        border: Color::Rgb(67, 133, 190),    // blue
        prompt: Color::Rgb(135, 154, 57),    // green
        generation: Color::Rgb(58, 169, 159), // cyan
        idle: Color::Rgb(87, 86, 83),        // grey4
        error: Color::Rgb(209, 77, 65),      // red
        warning: Color::Rgb(218, 112, 44),   // orange
        text: Color::Rgb(206, 205, 195),     // grey10
        dim: Color::Rgb(135, 133, 128),      // grey7
        highlight: Color::Rgb(218, 112, 44), // orange
    }
}

// Bar characters
pub const BAR_FILL: &str = "█";
pub const BAR_EMPTY: &str = "░";
