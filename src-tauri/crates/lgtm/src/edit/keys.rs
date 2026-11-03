//! Modes, and turning keys into commands on a buffer through the key table.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::buffer::{Buffer, Sel};
use super::keymap::{self, Cmd, Found};
use super::motion;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Select,
}

/// What a key asks of the screen around the buffer.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    None,
    /// Open the `:` command line.
    Command,
}

/// The screen's size, for keys that move by it.
#[derive(Clone, Copy)]
pub struct View {
    pub height: usize,
}

#[derive(Default)]
pub struct Keys {
    pub mode: Mode,
    /// The keys of a binding typed so far.
    pending: Vec<String>,
    /// The buffer's version when insert mode started.
    inserted_at: u64,
}

impl Keys {
    /// The keys typed toward a binding, like `g` or `space`.
    pub fn pending(&self) -> String {
        self.pending.join(" ")
    }

    pub fn press(&mut self, b: &mut Buffer, key: KeyEvent, view: View) -> Action {
        if self.mode == Mode::Insert {
            self.insert_key(b, key);
            return Action::None;
        }
        let Some(name) = keymap::name(&key) else {
            return Action::None;
        };
        self.pending.push(name);
        let seq = self.pending();
        match keymap::find(&seq, self.mode == Mode::Select) {
            Found::Partial => Action::None,
            Found::None => {
                self.pending.clear();
                Action::None
            }
            Found::Run(cmd) => {
                self.pending.clear();
                self.run(b, cmd, view)
            }
        }
    }

    fn run(&mut self, b: &mut Buffer, cmd: Cmd, view: View) -> Action {
        let extend = self.mode == Mode::Select;
        let half = (view.height / 2).max(1);
        match cmd {
            Cmd::Left => self.go(b, motion::left(b, b.sel.head), extend),
            Cmd::Right => self.go(b, motion::right(b, b.sel.head), extend),
            Cmd::Down => self.vertical(b, 1, extend),
            Cmd::Up => self.vertical(b, -1, extend),
            Cmd::HalfDown => self.vertical(b, half as isize, extend),
            Cmd::HalfUp => self.vertical(b, -(half as isize), extend),
            Cmd::Insert => self.enter_insert(b, b.sel.from()),
            Cmd::Append => self.enter_insert(b, (b.sel.to() + 1).min(b.len())),
            Cmd::InsertLineStart => {
                let line = b.line_of(b.sel.from());
                self.enter_insert(b, motion::first_non_blank(b, line));
            }
            Cmd::AppendLineEnd => {
                let line = b.line_of(b.sel.to());
                self.enter_insert(b, b.line_end(line));
            }
            Cmd::OpenBelow => self.open_line(b, true),
            Cmd::OpenAbove => self.open_line(b, false),
            Cmd::Undo => {
                b.undo();
            }
            Cmd::Redo => {
                b.redo();
            }
            Cmd::Command => return Action::Command,
            Cmd::Escape => self.mode = Mode::Normal,
        }
        if cmd != Cmd::Down && cmd != Cmd::Up && cmd != Cmd::HalfDown && cmd != Cmd::HalfUp {
            b.column = None;
        }
        Action::None
    }

    /// Moves the cursor to `to`, the selection following it in select mode.
    fn go(&mut self, b: &mut Buffer, to: usize, extend: bool) {
        b.sel = if extend {
            Sel::new(b.sel.anchor, to)
        } else {
            Sel::point(to)
        };
    }

    fn vertical(&mut self, b: &mut Buffer, by: isize, extend: bool) {
        let (to, column) = motion::vertical(b, b.sel.head, by, b.column);
        b.column = Some(column);
        self.go(b, to, extend);
    }

    fn enter_insert(&mut self, b: &mut Buffer, at: usize) {
        b.checkpoint();
        b.sel = Sel::point(at);
        self.start_insert(b);
    }

    fn start_insert(&mut self, b: &Buffer) {
        self.inserted_at = b.version;
        self.mode = Mode::Insert;
    }

    /// Opens a line below or above the selection's, with its indent, to type into.
    fn open_line(&mut self, b: &mut Buffer, below: bool) {
        b.checkpoint();
        let line = if below {
            b.line_of(b.sel.to())
        } else {
            b.line_of(b.sel.from())
        };
        let indent = b.indent(line);
        let at = if below {
            let end = b.line_end(line);
            b.insert(end, &format!("\n{indent}"));
            end + 1 + indent.chars().count()
        } else {
            let start = b.line_start(line);
            b.insert(start, &format!("{indent}\n"));
            start + indent.chars().count()
        };
        b.sel = Sel::point(at);
        self.start_insert(b);
    }

    fn insert_key(&mut self, b: &mut Buffer, key: KeyEvent) {
        let at = b.sel.head;
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => {
                b.drop_empty_checkpoint();
                self.mode = Mode::Normal;
                // Back onto the last char typed, as vim does.
                let typed = b.version != self.inserted_at;
                let back = typed && at > b.line_start(b.line_of(at));
                b.sel = Sel::point(if back { at - 1 } else { at });
                b.clamp();
                return;
            }
            KeyCode::Char('w') if ctrl => {
                let from = motion::word_back_from(b, at);
                b.remove(from, at);
                b.sel = Sel::point(from);
            }
            KeyCode::Char(c) if !ctrl => {
                b.insert(at, &c.to_string());
                b.sel = Sel::point(at + 1);
            }
            KeyCode::Tab => {
                let tab = motion::indent_unit(b);
                b.insert(at, &tab);
                b.sel = Sel::point(at + tab.chars().count());
            }
            KeyCode::Enter => {
                let indent = b.indent(b.line_of(at));
                // Not more indent than the cursor is into the line.
                let into = at - b.line_start(b.line_of(at));
                let indent: String = indent.chars().take(into).collect();
                b.insert(at, &format!("\n{indent}"));
                b.sel = Sel::point(at + 1 + indent.chars().count());
            }
            KeyCode::Backspace if at > 0 => {
                b.remove(at - 1, at);
                b.sel = Sel::point(at - 1);
            }
            KeyCode::Delete if at < b.len() => b.remove(at, at + 1),
            KeyCode::Left => b.sel = Sel::point(at.saturating_sub(1)),
            KeyCode::Right => b.sel = Sel::point((at + 1).min(b.len())),
            KeyCode::Up | KeyCode::Down => {
                let by = if key.code == KeyCode::Up { -1 } else { 1 };
                let (to, column) = motion::vertical(b, at, by, b.column);
                b.column = Some(column);
                b.sel = Sel::point(to);
                return;
            }
            _ => {}
        }
        b.column = None;
    }
}
