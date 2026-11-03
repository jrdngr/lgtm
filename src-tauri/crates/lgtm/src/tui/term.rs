//! Terminal setup. Where the terminal speaks the kitty keyboard protocol, Shift's own presses
//! and releases come through, so the key bar can show what Shift changes while it's held.

use std::io::stdout;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::supports_keyboard_enhancement;

pub struct Term {
    pub terminal: DefaultTerminal,
    enhanced: bool,
}

impl Term {
    pub fn start() -> Self {
        let terminal = ratatui::init();
        let enhanced = supports_keyboard_enhancement().unwrap_or(false)
            && execute!(
                stdout(),
                PushKeyboardEnhancementFlags(
                    KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                        | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                )
            )
            .is_ok();
        Self { terminal, enhanced }
    }

    pub fn stop(self) {
        if self.enhanced {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        }
        ratatui::restore();
    }
}

/// `key` as a terminal without the protocol sends it: a shifted letter as its capital.
pub fn plain(mut key: KeyEvent) -> KeyEvent {
    if let KeyCode::Char(c) = key.code
        && key.modifiers.contains(KeyModifiers::SHIFT)
        && c.is_ascii_lowercase()
    {
        key.code = KeyCode::Char(c.to_ascii_uppercase());
    }
    key
}

/// Whether `key` is Shift itself.
pub fn is_shift(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Modifier(
            ratatui::crossterm::event::ModifierKeyCode::LeftShift
                | ratatui::crossterm::event::ModifierKeyCode::RightShift
        )
    )
}
