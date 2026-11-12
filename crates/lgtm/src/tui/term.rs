//! Terminal setup. Where the terminal speaks the kitty keyboard protocol, Shift's own presses
//! and releases come through, so the key bar can show what Shift changes while it's held. The
//! mouse is captured for clicks and the wheel, and pastes come as one event.

use std::io::stdout;

use ratatui::DefaultTerminal;
use ratatui::crossterm::cursor::SetCursorStyle;
use ratatui::crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, KeyCode,
    KeyEvent, KeyModifiers, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
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
        let _ = execute!(stdout(), EnableMouseCapture, EnableBracketedPaste);
        // The cursor shows only in insert mode, as a bar in the text's color.
        let _ = execute!(stdout(), SetCursorStyle::SteadyBar);
        print!("\x1b]12;#d4d4d4\x07");
        Self {
            terminal,
            enhanced: enhance(),
        }
    }

    fn release(&self) {
        let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
        let _ = execute!(stdout(), SetCursorStyle::DefaultUserShape);
        print!("\x1b]112\x07");
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
