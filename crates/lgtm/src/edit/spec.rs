//! Key cases for tests: text with its selection marked, keys to press, and the text and
//! selection they should leave.
//!
//! `[` and `]` mark a selection, its cursor on the last char unless `<` follows `[`. `|`
//! marks the insert-mode gap. Keys are chars, with `<name>` for named keys like `<esc>`,
//! `<ret>` or `<C-d>`.

use std::path::Path;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::buffer::{Buffer, Sel};
use super::keys::{Keys, Mode, View};

pub fn buffer(marked: &str) -> Buffer {
    buffer_named(marked, "t.txt")
}

pub fn buffer_named(marked: &str, name: &str) -> Buffer {
    let mut text = String::new();
    let (mut start, mut end, mut back, mut gap) = (None, None, false, None);
    let mut chars = marked.chars().peekable();
    let mut n = 0;
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                start = Some(n);
                if chars.peek() == Some(&'<') {
                    chars.next();
                    back = true;
                }
            }
            ']' => end = Some(n - 1),
            '|' => gap = Some(n),
            c => {
                text.push(c);
                n += 1;
            }
        }
    }
    let mut b = Buffer::with_text(Path::new(name), name, &text);
    b.sel = match (start, end, gap) {
        (_, _, Some(g)) => Sel::point(g),
        (Some(s), Some(e), _) if back => Sel::new(e, s),
        (Some(s), Some(e), _) => Sel::new(s, e),
        _ => Sel::point(0),
    };
    b
}

/// The buffer's text with its selection marked as `buffer` reads it.
pub fn marked(b: &Buffer, mode: Mode) -> String {
    let text = b.text.to_string();
    let mut out = String::new();
    for (i, c) in text.chars().enumerate() {
        if mode == Mode::Insert {
            if i == b.sel.head {
                out.push('|');
            }
        } else if i == b.sel.from() {
            out.push('[');
            if b.sel.head < b.sel.anchor {
                out.push('<');
            }
        }
        out.push(c);
        if mode != Mode::Insert && i == b.sel.to() {
            out.push(']');
        }
    }
    if mode == Mode::Insert && b.sel.head >= text.chars().count() {
        out.push('|');
    }
    out
}

pub fn keys(spec: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let mut chars = spec.chars();
    while let Some(c) = chars.next() {
        if c != '<' {
            out.push(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
            continue;
        }
        let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
        if name == "S-tab" {
            out.push(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
            continue;
        }
        let (mods, base) = match name.split_once('-') {
            Some(("C", b)) => (KeyModifiers::CONTROL, b),
            Some(("A", b)) => (KeyModifiers::ALT, b),
            _ => (KeyModifiers::NONE, name.as_str()),
        };
        let code = match base {
            "esc" => KeyCode::Esc,
            "ret" => KeyCode::Enter,
            "bs" => KeyCode::Backspace,
            "del" => KeyCode::Delete,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            "lt" => KeyCode::Char('<'),
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            one => KeyCode::Char(one.chars().next().expect("a key")),
        };
        out.push(KeyEvent::new(code, mods));
    }
    out
}

/// Presses `spec`'s keys on `start` and returns the marked result.
pub fn run(start: &str, spec: &str) -> String {
    run_in(buffer(start), spec)
}

pub fn run_in(mut b: Buffer, spec: &str) -> String {
    let mut k = Keys::default();
    for key in keys(spec) {
        k.press(
            &mut b,
            key,
            View {
                height: 10,
                hidden: &[],
            },
        );
    }
    marked(&b, k.mode)
}

/// Asserts each case: start, keys, expected.
pub fn check(cases: &[(&str, &str, &str)]) {
    for (start, spec, want) in cases {
        assert_eq!(&run(start, spec), want, "{start:?} with {spec:?}");
    }
}
