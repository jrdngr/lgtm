//! Typing into a one-line text field.

use gpui::Keystroke;

#[derive(Debug, PartialEq, Eq)]
pub enum Key {
    /// The text changed.
    Edited,
    Enter,
    /// Shift-enter.
    ShiftEnter,
    Escape,
    Up,
    Down,
    /// Nothing a field handles, like a key with ⌘.
    Other,
}

/// Applies a keystroke to `text`.
pub fn key(text: &mut String, k: &Keystroke) -> Key {
    if k.modifiers.platform {
        return Key::Other;
    }
    match k.key.as_str() {
        "enter" if k.modifiers.shift => Key::ShiftEnter,
        "enter" => Key::Enter,
        "escape" => Key::Escape,
        "up" => Key::Up,
        "down" => Key::Down,
        "backspace" => {
            if k.modifiers.alt {
                let trimmed = text.trim_end().len();
                let cut = text[..trimmed]
                    .rfind(char::is_whitespace)
                    .map_or(0, |i| i + 1);
                text.truncate(cut);
            } else {
                text.pop();
            }
            Key::Edited
        }
        _ => match k
            .key_char
            .as_deref()
            .filter(|t| !t.chars().any(char::is_control))
        {
            Some(t) if !k.modifiers.control => {
                text.push_str(t);
                Key::Edited
            }
            _ => Key::Other,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(text: &mut String, key: &'static str) -> Key {
        let mut k = Keystroke::parse(key).unwrap();
        if k.key.chars().count() == 1 && !k.modifiers.control && !k.modifiers.platform {
            k.key_char = Some(k.key.clone());
        }
        super::key(text, &k)
    }

    #[test]
    fn typing_edits_and_special_keys_report() {
        let mut t = String::from("split ri");
        assert_eq!(press(&mut t, "g"), Key::Edited);
        assert_eq!(t, "split rig");
        assert_eq!(press(&mut t, "alt-backspace"), Key::Edited);
        assert_eq!(t, "split ");
        assert_eq!(press(&mut t, "backspace"), Key::Edited);
        assert_eq!(t, "split");
        assert_eq!(press(&mut t, "shift-enter"), Key::ShiftEnter);
        assert_eq!(press(&mut t, "cmd-a"), Key::Other);
        assert_eq!(press(&mut t, "ctrl-a"), Key::Other);
        assert_eq!(t, "split");
    }
}
