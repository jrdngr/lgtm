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
        Self {
            terminal,
            enhanced: enhance(),
        }
    }

    /// Gives the terminal back for a program to run in it, then takes it again.
    pub fn suspended<T>(&mut self, run: impl FnOnce() -> T) -> T {
        self.release();
        let out = run();
        self.terminal = ratatui::init();
        self.enhanced = enhance();
        out
    }

    fn release(&self) {
        if self.enhanced {
            let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
        }
        ratatui::restore();
    }

    pub fn stop(self) {
        self.release();
    }
}

/// Turns on the kitty keyboard protocol where the terminal has it. Returns whether it did.
fn enhance() -> bool {
    supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
        )
        .is_ok()
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
