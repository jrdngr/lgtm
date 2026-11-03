//! The one key table: which keys run which command in which modes. Keys are names joined by
//! spaces, like `g g`, `space f` or `C-d`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Left,
    Right,
    Down,
    Up,
    HalfDown,
    HalfUp,
    Insert,
    Append,
    InsertLineStart,
    AppendLineEnd,
    OpenBelow,
    OpenAbove,
    Undo,
    Redo,
    Command,
    Escape,
}

/// The modes a binding works in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum In {
    Select,
    Both,
}

pub struct Bind {
    pub keys: &'static str,
    pub cmd: Cmd,
    pub modes: In,
    /// What it does, for the `?` list and the prefix menus.
    pub label: &'static str,
}

const fn b(keys: &'static str, cmd: Cmd, modes: In, label: &'static str) -> Bind {
    Bind {
        keys,
        cmd,
        modes,
        label,
    }
}

use Cmd::*;
use In::{Both, Select};

pub const BINDS: &[Bind] = &[
    b("h", Left, Both, "left"),
    b("left", Left, Both, "left"),
    b("l", Right, Both, "right"),
    b("right", Right, Both, "right"),
    b("j", Down, Both, "down"),
    b("down", Down, Both, "down"),
    b("k", Up, Both, "up"),
    b("up", Up, Both, "up"),
    b("C-d", HalfDown, Both, "half page down"),
    b("C-u", HalfUp, Both, "half page up"),
    b("i", Insert, Both, "insert"),
    b("a", Append, Both, "append"),
    b("I", InsertLineStart, Both, "insert at line start"),
    b("A", AppendLineEnd, Both, "append at line end"),
    b("o", OpenBelow, Both, "line below"),
    b("O", OpenAbove, Both, "line above"),
    b("u", Undo, Both, "undo"),
    b("U", Redo, Both, "redo"),
    b(":", Command, Both, "command"),
    b("esc", Escape, Select, "normal mode"),
];

/// The name `key` has in the table.
pub fn name(key: &KeyEvent) -> Option<String> {
    let base = match key.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Enter => "ret".into(),
        KeyCode::Backspace => "bs".into(),
        KeyCode::Delete => "del".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pgup".into(),
        KeyCode::PageDown => "pgdn".into(),
        _ => return None,
    };
    let mut out = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("C-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        out.push_str("A-");
    }
    out.push_str(&base);
    Some(out)
}

/// What the keys typed so far come to in `select` mode or not.
pub enum Found {
    Run(Cmd),
    /// More keys could finish a binding.
    Partial,
    None,
}

fn fits(bind: &Bind, select: bool) -> bool {
    match bind.modes {
        In::Both => true,
        In::Select => select,
    }
}

pub fn find(seq: &str, select: bool) -> Found {
    let mut partial = false;
    for bind in BINDS.iter().filter(|b| fits(b, select)) {
        if bind.keys == seq {
            return Found::Run(bind.cmd);
        }
        partial |= bind
            .keys
            .strip_prefix(seq)
            .is_some_and(|rest| rest.starts_with(' '));
    }
    if partial { Found::Partial } else { Found::None }
}

/// The bindings after `prefix`, each with its next key and label, for a prefix's menu.
pub fn after(prefix: &str, select: bool) -> Vec<(&'static str, &'static str)> {
    let start = format!("{prefix} ");
    BINDS
        .iter()
        .filter(|b| fits(b, select))
        .filter_map(|b| Some((b.keys.strip_prefix(start.as_str())?, b.label)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_binding_is_unique_in_its_modes_and_no_binding_hides_another() {
        for (i, a) in BINDS.iter().enumerate() {
            for z in &BINDS[i + 1..] {
                let shared = [false, true]
                    .into_iter()
                    .any(|select| fits(a, select) && fits(z, select));
                if !shared {
                    continue;
                }
                assert_ne!(a.keys, z.keys, "{} is bound twice", a.keys);
                let hides = |x: &str, y: &str| y.starts_with(&format!("{x} "));
                assert!(
                    !hides(a.keys, z.keys) && !hides(z.keys, a.keys),
                    "{} and {} overlap",
                    a.keys,
                    z.keys
                );
            }
        }
    }
}
