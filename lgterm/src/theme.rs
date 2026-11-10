//! Colors and sizes from the approved mockups.

pub const WINDOW: u32 = 0x191919;
pub const SIDEBAR: u32 = 0x141414;
pub const SIDEBAR_BORDER: u32 = 0x2a2a2a;
pub const ROW_SELECTED: u32 = 0x262626;
pub const ROW_HOVER: u32 = 0x1f1f1f;
pub const CARD: u32 = 0x1e1e1e;
pub const CARD_BORDER: u32 = 0x3a3a3a;
pub const FOCUS: u32 = 0x4f8cc9;
pub const TAB_STRIP_BORDER: u32 = 0x2e2e2e;
pub const TAB_ACTIVE: u32 = 0x313131;

pub const TEXT: u32 = 0xd9d9da;
pub const TEXT_STRONG: u32 = 0xeeeeef;
pub const TEXT_MUTED: u32 = 0x9b9b9b;
pub const TEXT_SPACE: u32 = 0xb8b8b8;
pub const ICON: u32 = 0x8f8f8f;
pub const BADGE_TEXT: u32 = 0xffffff;
pub const CLAUDE: u32 = 0xd97757;

pub const TERM_FG: u32 = 0xd4d4d4;
pub const TERM_BG: u32 = CARD;
pub const SELECTION: u32 = 0x3a5a7a;

pub const SIDEBAR_WIDTH: f32 = 264.;
/// The sidebar with only layout badges: wide enough for the traffic lights.
pub const SIDEBAR_ICONS: f32 = 76.;
pub const SIDEBAR_MIN: f32 = 150.;
pub const SIDEBAR_MAX: f32 = 420.;
pub const GAP: f32 = 8.;
pub const TITLEBAR: f32 = 46.;
pub const TERM_PAD_X: f32 = 14.;
pub const TERM_PAD_Y: f32 = 12.;

pub const UI_FONT: &str = if cfg!(target_os = "macos") {
    ".SystemUIFont"
} else {
    "DejaVu Sans"
};
pub const MONO_FONT: &str = if cfg!(target_os = "macos") {
    "Menlo"
} else {
    "DejaVu Sans Mono"
};
pub const DEFAULT_FONT_SIZE: f32 = 13.;

pub const BLOCKED: u32 = 0xe3b341;
pub const DONE: u32 = 0x3fb950;
pub const WORKING: u32 = 0x58a6ff;
pub const IDLE: u32 = 0x7a7a7a;
pub const DIALOG: u32 = 0x272727;
pub const MATCH: u32 = 0x79b8ff;
pub const CODE: u32 = 0x39c5cf;
pub const ROW_RULE: u32 = 0x2a2a2a;
