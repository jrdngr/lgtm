//! Modes, and turning keys into commands on a buffer through the key table.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::buffer::{Buffer, Sel};
use super::fold::Fold;
use super::keymap::{self, Cmd, Found, Pick};
use super::motion;
use super::objects;
use super::ops::{self, Register};

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
    /// Open the search line.
    Search,
    /// Put this on the system clipboard.
    Copy(String),
    /// Paste the system clipboard after the selection, through `paste_text`.
    Paste,
    /// Something the buffer couldn't do, to show.
    Notice(String),
    /// Open or close folds at the cursor's line.
    Fold(Fold),
    /// Open a list to pick from.
    Pick(Pick),
    /// Go to the review.
    Review,
}

/// The screen around the buffer: its size, for keys that move by it, and the closed folds,
/// which `j` and `k` step over.
#[derive(Clone, Copy)]
pub struct View<'a> {
    pub height: usize,
    pub hidden: &'a [(usize, usize)],
}

/// A command waiting for the char typed after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Await {
    Find {
        forward: bool,
        till: bool,
    },
    Replace,
    SurroundAdd,
    SurroundDelete,
    /// `mr`: the pair to replace, once typed.
    SurroundReplace(Option<char>),
    /// `mi` or `ma`: whether it's around.
    Object(bool),
}

#[derive(Default)]
pub struct Keys {
    pub mode: Mode,
    /// The keys of a binding typed so far.
    pending: Vec<String>,
    awaiting: Option<Await>,
    /// The buffer's version when insert mode started.
    pub(super) inserted_at: u64,
    register: Register,
    /// What `n` and `N` look for.
    pub search: Option<String>,
    /// Selections `Alt-o` grew from, for `Alt-i` to go back through.
    grown: Vec<Sel>,
}

impl Keys {
    /// The keys typed toward a binding, like `g` or `space`.
    pub fn pending(&self) -> String {
        self.pending.join(" ")
    }

    /// Whether a key is still to come: part of a binding, or a command's char.
    pub fn waiting(&self) -> bool {
        !self.pending.is_empty() || self.awaiting.is_some()
    }

    pub fn press(&mut self, b: &mut Buffer, key: KeyEvent, view: View) -> Action {
        if self.mode == Mode::Insert {
            self.insert_key(b, key);
            return Action::None;
        }
        if let Some(wait) = self.awaiting.take() {
            return match key.code {
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.finish(b, wait, c)
                }
                _ => Action::None,
            };
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
                let action = self.run(b, cmd, view);
                if !matches!(cmd, Cmd::Down | Cmd::Up | Cmd::HalfDown | Cmd::HalfUp) {
                    b.column = None;
                }
                action
            }
        }
    }

    fn select(&self) -> bool {
        self.mode == Mode::Select
    }

    /// Takes a motion's selection, or in select mode moves only its head.
    fn take(&self, b: &mut Buffer, (anchor, head): (usize, usize)) {
        b.sel = if self.select() {
            Sel::new(b.sel.anchor, head)
        } else {
            Sel::new(anchor, head)
        };
    }

    /// Moves the cursor to `to`, the selection following it in select mode.
    fn go(&self, b: &mut Buffer, to: usize) {
        self.take(b, (to, to));
    }

    fn vertical(&self, b: &mut Buffer, by: isize, hidden: &[(usize, usize)]) {
        let (to, column) = motion::vertical_folded(b, b.sel.head, by, b.column, hidden);
        b.column = Some(column);
        self.go(b, to);
    }

    /// Starts an edit: one undo step, after which select mode ends.
    fn edit(&mut self, b: &mut Buffer) {
        b.checkpoint();
        if self.select() {
            self.mode = Mode::Normal;
        }
    }

    fn run(&mut self, b: &mut Buffer, cmd: Cmd, view: View) -> Action {
        let half = (view.height / 2).max(1) as isize;
        let head = b.sel.head;
        match cmd {
            Cmd::Left => self.go(b, motion::left(b, head)),
            Cmd::Right => self.go(b, motion::right(b, head)),
            Cmd::Down => self.vertical(b, 1, view.hidden),
            Cmd::Up => self.vertical(b, -1, view.hidden),
            Cmd::HalfDown => self.vertical(b, half, view.hidden),
            Cmd::HalfUp => self.vertical(b, -half, view.hidden),
            Cmd::NextWord(big) => self.take(b, motion::next_word_start(b, head, big)),
            Cmd::WordEnd(big) => self.take(b, motion::next_word_end(b, head, big)),
            Cmd::PrevWord(big) => self.take(b, motion::prev_word_start(b, head, big)),
            Cmd::Find { forward, till } => self.awaiting = Some(Await::Find { forward, till }),
            Cmd::FileStart => self.go(b, 0),
            Cmd::FileEnd => self.go(b, b.line_start(b.last_line())),
            Cmd::LineStart => self.go(b, b.line_start(b.line_of(head))),
            Cmd::LineEnd => self.go(b, motion::line_max(b, b.line_of(head))),
            Cmd::LineText => self.go(b, motion::first_non_blank(b, b.line_of(head))),
            Cmd::SelectLine => {
                let (from, to) = motion::line_select(b, b.sel.from(), b.sel.to());
                b.sel = Sel::new(from, to);
            }
            Cmd::ToggleSelect => {
                self.mode = if self.select() {
                    Mode::Normal
                } else {
                    Mode::Select
                };
            }
            Cmd::Collapse => b.sel = Sel::point(head),
            Cmd::SelectAll => b.sel = Sel::new(0, b.last_char()),
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
            Cmd::Delete if b.len() > 0 => {
                self.register = ops::yank(b);
                self.edit(b);
                ops::delete(b);
            }
            Cmd::Change => {
                if b.len() > 0 {
                    self.register = ops::yank(b);
                }
                b.checkpoint();
                let at = if b.len() > 0 { ops::change(b) } else { 0 };
                b.sel = Sel::point(at);
                self.start_insert(b);
            }
            Cmd::Yank if b.len() > 0 => {
                self.register = ops::yank(b);
                if self.select() {
                    self.mode = Mode::Normal;
                }
            }
            Cmd::PasteAfter | Cmd::PasteBefore => {
                let r = self.register.clone();
                self.edit(b);
                ops::paste(b, &r, cmd == Cmd::PasteAfter);
                b.drop_empty_checkpoint();
            }
            Cmd::Replace => self.awaiting = Some(Await::Replace),
            Cmd::Indent | Cmd::Outdent => {
                self.edit(b);
                ops::indent(b, cmd == Cmd::Indent);
                b.drop_empty_checkpoint();
            }
            Cmd::Join => {
                self.edit(b);
                ops::join(b);
                b.drop_empty_checkpoint();
            }
            Cmd::ToggleCase if b.len() > 0 => {
                self.edit(b);
                ops::toggle_case(b);
            }
            Cmd::Object(around) => self.awaiting = Some(Await::Object(around)),
            Cmd::Grow => match objects::grow(b) {
                Some((from, to)) => {
                    self.grown.push(b.sel);
                    b.sel = Sel::new(from, to);
                }
                None => return Action::Notice("no syntax node around the selection".into()),
            },
            Cmd::Shrink => {
                let inside = |s: &Sel| s.from() >= b.sel.from() && s.to() <= b.sel.to();
                match self.grown.pop().filter(inside) {
                    Some(sel) => b.sel = sel,
                    None => {
                        self.grown.clear();
                        if let Some((from, to)) = objects::shrink(b) {
                            b.sel = Sel::new(from, to);
                        }
                    }
                }
            }
            Cmd::Fold(fold) => return Action::Fold(fold),
            Cmd::SurroundAdd => self.awaiting = Some(Await::SurroundAdd),
            Cmd::SurroundDelete => self.awaiting = Some(Await::SurroundDelete),
            Cmd::SurroundReplace => self.awaiting = Some(Await::SurroundReplace(None)),
            Cmd::Undo => {
                b.undo();
            }
            Cmd::Redo => {
                b.redo();
            }
            Cmd::Search => return Action::Search,
            Cmd::SearchNext => return self.next_match(b, true),
            Cmd::SearchPrev => return self.next_match(b, false),
            Cmd::SearchSelection if b.len() > 0 => {
                self.search = Some(b.slice(b.sel.from(), b.sel.to() + 1));
            }
            Cmd::ClipboardYank if b.len() > 0 => {
                return Action::Copy(b.slice(b.sel.from(), b.sel.to() + 1));
            }
            Cmd::ClipboardPaste => return Action::Paste,
            Cmd::Pick(pick) => return Action::Pick(pick),
            Cmd::Review => return Action::Review,
            Cmd::Command => return Action::Command,
            Cmd::Escape => self.mode = Mode::Normal,
            Cmd::Delete | Cmd::Yank | Cmd::ToggleCase | Cmd::SearchSelection => {}
            Cmd::ClipboardYank => {}
        }
        Action::None
    }

    /// Runs a command that waited for `c`.
    fn finish(&mut self, b: &mut Buffer, wait: Await, c: char) -> Action {
        match wait {
            Await::Find { forward, till } => {
                if let Some(to) = motion::find_char(b, b.sel.head, c, forward, till) {
                    self.take(b, (b.sel.head, to));
                }
            }
            Await::Replace if b.len() > 0 => {
                self.edit(b);
                ops::replace(b, c);
            }
            Await::Replace => {}
            Await::SurroundAdd if b.len() > 0 => {
                self.edit(b);
                ops::surround_add(b, c);
            }
            Await::SurroundAdd => {}
            Await::SurroundDelete => {
                self.edit(b);
                if !ops::surround_delete(b, c) {
                    b.drop_empty_checkpoint();
                    return Action::Notice(format!("no {c} around the selection"));
                }
            }
            Await::Object(around) => match objects::object(b, c, around) {
                Some((from, to)) => self.take(b, (from, to)),
                None => return Action::Notice(format!("no {c} object here")),
            },
            Await::SurroundReplace(None) => self.awaiting = Some(Await::SurroundReplace(Some(c))),
            Await::SurroundReplace(Some(from)) => {
                self.edit(b);
                if !ops::surround_replace(b, from, c) {
                    b.drop_empty_checkpoint();
                    return Action::Notice(format!("no {from} around the selection"));
                }
            }
        }
        Action::None
    }

    /// Searches for `needle` from the cursor, and keeps it for `n` and `N`.
    pub fn search_for(&mut self, b: &mut Buffer, needle: &str) -> Action {
        self.search = Some(needle.to_string());
        self.next_match(b, true)
    }

    fn next_match(&mut self, b: &mut Buffer, forward: bool) -> Action {
        let Some(needle) = self.search.clone() else {
            return Action::None;
        };
        let at = if forward { b.sel.to() } else { b.sel.from() };
        match motion::search(b, at, &needle, forward) {
            Some((from, to)) => {
                self.take(b, (from, to - 1));
                Action::None
            }
            None => Action::Notice(format!("no match for {needle}")),
        }
    }

    /// Pastes `text`, from the system clipboard, after the selection.
    pub fn paste_text(&mut self, b: &mut Buffer, text: &str) {
        let r = Register {
            text: text.to_string(),
            lines: text.ends_with('\n'),
        };
        self.edit(b);
        ops::paste(b, &r, true);
        b.drop_empty_checkpoint();
    }
}
