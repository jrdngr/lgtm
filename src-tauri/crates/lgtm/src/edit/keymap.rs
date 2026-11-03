//! The one key table: which keys run which command in which modes. Keys are names joined by
//! spaces, like `g g`, `space f` or `C-d`.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::fold::Fold;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmd {
    Left,
    Right,
    Down,
    Up,
    HalfDown,
    HalfUp,
    /// `w` and `W`: whether it's WORDs, split only at spaces.
    NextWord(bool),
    WordEnd(bool),
    PrevWord(bool),
    /// `f` `t` `F` `T`, with the char that follows: forward, and whether to stop before it.
    Find {
        forward: bool,
        till: bool,
    },
    FileStart,
    FileEnd,
    LineStart,
    LineEnd,
    LineText,
    SelectLine,
    ToggleSelect,
    Collapse,
    SelectAll,
    Insert,
    Append,
    InsertLineStart,
    AppendLineEnd,
    OpenBelow,
    OpenAbove,
    Delete,
    Change,
    Yank,
    PasteAfter,
    PasteBefore,
    /// `r`, with the char that follows.
    Replace,
    Indent,
    Outdent,
    Join,
    ToggleCase,
    /// `mi` and `ma`, with the object's char after: whether it's around.
    Object(bool),
    /// `Alt-o` and `Alt-i`.
    Grow,
    Shrink,
    Fold(Fold),
    /// `ms`, `md`, `mr`, with the chars that follow.
    SurroundAdd,
    SurroundDelete,
    SurroundReplace,
    Undo,
    Redo,
    Search,
    SearchNext,
    SearchPrev,
    SearchSelection,
    ClipboardYank,
    ClipboardPaste,
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
    b("w", NextWord(false), Both, "word"),
    b("W", NextWord(true), Both, "WORD"),
    b("e", WordEnd(false), Both, "word end"),
    b("E", WordEnd(true), Both, "WORD end"),
    b("b", PrevWord(false), Both, "word back"),
    b("B", PrevWord(true), Both, "WORD back"),
    b(
        "f",
        Find {
            forward: true,
            till: false,
        },
        Both,
        "find char",
    ),
    b(
        "t",
        Find {
            forward: true,
            till: true,
        },
        Both,
        "till char",
    ),
    b(
        "F",
        Find {
            forward: false,
            till: false,
        },
        Both,
        "find char back",
    ),
    b(
        "T",
        Find {
            forward: false,
            till: true,
        },
        Both,
        "till char back",
    ),
    b("g g", FileStart, Both, "file start"),
    b("g e", FileEnd, Both, "file end"),
    b("g h", LineStart, Both, "line start"),
    b("g l", LineEnd, Both, "line end"),
    b("g s", LineText, Both, "first non-blank"),
    b("x", SelectLine, Both, "select line"),
    b("v", ToggleSelect, Both, "select mode"),
    b(";", Collapse, Both, "collapse selection"),
    b("%", SelectAll, Both, "select all"),
    b("i", Insert, Both, "insert"),
    b("a", Append, Both, "append"),
    b("I", InsertLineStart, Both, "insert at line start"),
    b("A", AppendLineEnd, Both, "append at line end"),
    b("o", OpenBelow, Both, "line below"),
    b("O", OpenAbove, Both, "line above"),
    b("d", Delete, Both, "delete"),
    b("c", Change, Both, "change"),
    b("y", Yank, Both, "yank"),
    b("p", PasteAfter, Both, "paste after"),
    b("P", PasteBefore, Both, "paste before"),
    b("r", Replace, Both, "replace with a char"),
    b(">", Indent, Both, "indent"),
    b("<", Outdent, Both, "outdent"),
    b("J", Join, Both, "join lines"),
    b("~", ToggleCase, Both, "toggle case"),
    b("m i", Object(false), Both, "inside an object"),
    b("m a", Object(true), Both, "around an object"),
    b("A-o", Grow, Both, "grow to the syntax node"),
    b("A-i", Shrink, Both, "shrink to a syntax node"),
    b("m s", SurroundAdd, Both, "add surround"),
    b("m d", SurroundDelete, Both, "delete surround"),
    b("m r", SurroundReplace, Both, "replace surround"),
    b("u", Undo, Both, "undo"),
    b("U", Redo, Both, "redo"),
    b("/", Search, Both, "search"),
    b("n", SearchNext, Both, "next match"),
    b("N", SearchPrev, Both, "previous match"),
    b("*", SearchSelection, Both, "search the selection"),
    b("z z", Fold(Fold::Toggle), Both, "toggle"),
    b("z c", Fold(Fold::Close), Both, "close"),
    b("z o", Fold(Fold::Open), Both, "open"),
    b("z M", Fold(Fold::CloseAll), Both, "close all"),
    b("z R", Fold(Fold::OpenAll), Both, "open all"),
    b("z m", Fold(Fold::CloseLevel), Both, "close a level"),
    b("z r", Fold(Fold::OpenLevel), Both, "open a level"),
    b("space y", ClipboardYank, Both, "yank to clipboard"),
    b("space p", ClipboardPaste, Both, "paste from clipboard"),
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
