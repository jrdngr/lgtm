//! GPUI keystrokes as Ghostty key events.

use ghostty_vt::key::{Key, Mods};
use gpui::{Keystroke, Modifiers};

pub fn key(name: &str) -> Key {
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return char_key(c.to_ascii_lowercase());
    }
    match name {
        "space" => Key::Space,
        "enter" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "escape" => Key::Escape,
        "delete" => Key::Delete,
        "insert" => Key::Insert,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::ArrowUp,
        "down" => Key::ArrowDown,
        "left" => Key::ArrowLeft,
        "right" => Key::ArrowRight,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "f13" => Key::F13,
        "f14" => Key::F14,
        "f15" => Key::F15,
        "f16" => Key::F16,
        "f17" => Key::F17,
        "f18" => Key::F18,
        "f19" => Key::F19,
        "f20" => Key::F20,
        "f21" => Key::F21,
        "f22" => Key::F22,
        "f23" => Key::F23,
        "f24" => Key::F24,
        "f25" => Key::F25,
        _ => Key::Unidentified,
    }
}

fn char_key(c: char) -> Key {
    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    const DIGITS: [Key; 10] = [
        Key::Digit0,
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
        Key::Digit6,
        Key::Digit7,
        Key::Digit8,
        Key::Digit9,
    ];
    match c {
        'a'..='z' => LETTERS[c as usize - 'a' as usize],
        '0'..='9' => DIGITS[c as usize - '0' as usize],
        ' ' => Key::Space,
        '-' => Key::Minus,
        '=' => Key::Equal,
        '[' => Key::BracketLeft,
        ']' => Key::BracketRight,
        '\\' => Key::Backslash,
        ';' => Key::Semicolon,
        '\'' => Key::Quote,
        ',' => Key::Comma,
        '.' => Key::Period,
        '/' => Key::Slash,
        '`' => Key::Backquote,
        _ => Key::Unidentified,
    }
}

pub fn mods(m: &Modifiers) -> Mods {
    let mut out = Mods::empty();
    out.set(Mods::SHIFT, m.shift);
    out.set(Mods::ALT, m.alt);
    out.set(Mods::CTRL, m.control);
    out.set(Mods::SUPER, m.platform);
    out
}

/// The text a keystroke types, if it types any.
pub fn text(k: &Keystroke) -> Option<&str> {
    let text = k.key_char.as_deref()?;
    (!k.modifiers.control && !text.chars().any(char::is_control)).then_some(text)
}

/// Whether a keystroke adds to a prompt being typed (true) or sends or drops it (false).
/// Escape leaves a prompt as it is: agents keep the text when it closes a menu or stops a turn.
pub fn drafts(k: &Keystroke) -> Option<bool> {
    let m = &k.modifiers;
    match k.key.as_str() {
        "enter" if m.shift || m.alt => Some(true),
        "enter" => Some(false),
        "c" if m.control => Some(false),
        "backspace" | "delete" => Some(true),
        _ => text(k).is_some().then_some(true),
    }
}

/// The modifiers spent producing `text`, like shift on `A`.
pub fn consumed(k: &Keystroke) -> Mods {
    let mut out = Mods::empty();
    if text(k).is_some() {
        out.set(Mods::SHIFT, k.modifiers.shift);
        out.set(Mods::ALT, k.modifiers.alt);
    }
    out
}

/// The key's character with no modifiers held.
pub fn unshifted(name: &str) -> Option<char> {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c.to_ascii_lowercase()),
        _ if name == "space" => Some(' '),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghostty_vt::key::{Action, Encoder, Event};

    fn encode(keystroke: &'static str) -> Vec<u8> {
        let k = Keystroke::parse(keystroke).unwrap();
        let mut event = Event::new().unwrap();
        event
            .set_action(Action::Press)
            .set_key(key(&k.key))
            .set_mods(mods(&k.modifiers))
            .set_consumed_mods(consumed(&k))
            .set_utf8(text(&k).map(str::to_owned));
        if let Some(c) = unshifted(&k.key) {
            event.set_unshifted_codepoint(c);
        }
        let mut out = Vec::new();
        Encoder::new()
            .unwrap()
            .encode_to_vec(&event, &mut out)
            .unwrap();
        out
    }

    #[test]
    fn keystrokes_encode_to_what_a_shell_expects() {
        assert_eq!(encode("ctrl-c"), b"\x03");
        assert_eq!(encode("enter"), b"\r");
        assert_eq!(encode("up"), b"\x1b[A");
        assert_eq!(encode("backspace"), b"\x7f");
        assert_eq!(encode("shift-tab"), b"\x1b[Z");
    }

    #[test]
    fn typing_starts_a_draft_and_sending_or_cancelling_ends_it_but_escape_keeps_it() {
        let d = |s| drafts(&Keystroke::parse(s).unwrap());
        assert_eq!(d("backspace"), Some(true));
        assert_eq!(d("shift-enter"), Some(true));
        assert_eq!(d("enter"), Some(false));
        assert_eq!(d("escape"), None);
        assert_eq!(d("ctrl-c"), Some(false));
        assert_eq!(d("up"), None);
        let mut a = Keystroke::parse("a").unwrap();
        a.key_char = Some("a".into());
        assert_eq!(drafts(&a), Some(true));
    }
}
