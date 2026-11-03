//! The desktop app's dark palette, so both look alike.

use ratatui::style::{Color, Modifier, Style};

const fn rgb(v: u32) -> Color {
    Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

/// `c` at `pct` percent over the background.
const fn tint(c: (u8, u8, u8), pct: u16) -> Color {
    const fn mix(base: u8, c: u8, pct: u16) -> u8 {
        ((base as u16 * (100 - pct) + c as u16 * pct) / 100) as u8
    }
    Color::Rgb(mix(30, c.0, pct), mix(30, c.1, pct), mix(30, c.2, pct))
}

pub const BG: Color = rgb(0x1e1e1e);
pub const PANEL: Color = rgb(0x252526);
pub const FG: Color = rgb(0xd4d4d4);
pub const MUTED: Color = rgb(0x8a8a8a);
pub const FAINT: Color = rgb(0x5f6368);
pub const HAIR: Color = rgb(0x303030);
pub const ACCENT: Color = rgb(0x4f8cc9);
pub const GREEN: Color = rgb(0x3fb950);
pub const RED: Color = rgb(0xf85149);
pub const CYAN: Color = rgb(0x39c5cf);
pub const FUCHSIA: Color = rgb(0xd670d6);
pub const AMBER: Color = rgb(0xd29922);
pub const UNIQUE: Color = rgb(0xc586c0);

pub const ADD: Color = tint((46, 160, 67), 15);
pub const DEL: Color = tint((248, 81, 73), 15);
pub const SEL: Color = tint((79, 140, 201), 28);
/// Behind a comment for the agent.
pub const NOTE: Color = tint((79, 140, 201), 12);
pub const BLANK: Color = tint((60, 60, 60), 35);

pub fn fg(c: Color) -> Style {
    Style::default().fg(c)
}

pub fn bold(c: Color) -> Style {
    fg(c).add_modifier(Modifier::BOLD)
}

/// The color of a highlight capture, by its name in `syntax::HIGHLIGHT_NAMES`.
pub fn syntax(name: &str) -> Color {
    match name {
        "attribute" | "keyword" => rgb(0xc586c0),
        "comment" => rgb(0x6a9955),
        "constant" | "number" => rgb(0xb5cea8),
        "constructor" | "function" => rgb(0xdcdcaa),
        "escape" => rgb(0xd7ba7d),
        "label" | "property" | "variable.parameter" => rgb(0x9cdcfe),
        "module" | "tag" | "type" => rgb(0x4ec9b0),
        "string" => rgb(0xce9178),
        "variable.builtin" => rgb(0x569cd6),
        _ => FG,
    }
}
