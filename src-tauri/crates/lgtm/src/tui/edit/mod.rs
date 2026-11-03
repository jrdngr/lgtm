//! The editor screen: open buffers, the one in view, the `:` command line, and the keys.

mod clipboard;
mod command;
mod draw;

use std::path::Path;

use lgtm_core::syntax::Segment;
use ratatui::crossterm::event::KeyEvent;

use crate::edit::buffer::{Buffer, Sel};
use crate::edit::keys::{Action, Keys, Mode, View};
use crate::edit::motion;

pub enum Outcome {
    None,
    /// Leave the editor.
    Back,
}

/// A buffer with where it's scrolled to and its colors.
pub struct Open {
    pub buf: Buffer,
    /// The first line and column in view.
    top: usize,
    left: usize,
    /// Highlight segments per line, and the buffer version they're for.
    colors: Option<(u64, Vec<Vec<Segment>>)>,
}

impl Open {
    fn new(buf: Buffer) -> Self {
        Self {
            buf,
            top: 0,
            left: 0,
            colors: None,
        }
    }
}

/// Puts the cursor on `line`, from 1, at its first char that isn't a space.
fn go_line(b: &mut Buffer, line: usize) {
    let line = (line.max(1) - 1).min(b.last_line());
    b.sel = Sel::point(motion::first_non_blank(b, line));
    b.clamp();
}

pub struct Editor {
    pub open: Vec<Open>,
    cur: usize,
    keys: Keys,
    /// The `:` or `/` line being typed.
    prompt: Option<command::Prompt>,
    /// A failure or warning to show until the next key.
    pub notice: Option<String>,
    /// Lines of text the editor had when last drawn.
    height: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            open: vec![],
            cur: 0,
            keys: Keys::default(),
            prompt: None,
            notice: None,
            height: 20,
        }
    }

    /// Opens the file `name` in the workspace at `root` at `line`, from 1, or goes to it when
    /// it's open already.
    pub fn open(&mut self, root: &Path, name: &str, line: u32) -> std::io::Result<()> {
        let at = match self.open.iter().position(|o| o.buf.name == name) {
            Some(i) => i,
            None => {
                let buf = Buffer::open(&root.join(name), name)?;
                self.open.push(Open::new(buf));
                self.open.len() - 1
            }
        };
        self.cur = at;
        self.keys.mode = Mode::Normal;
        go_line(&mut self.open[at].buf, line as usize);
        Ok(())
    }

    pub fn current(&self) -> Option<&Buffer> {
        self.open.get(self.cur).map(|o| &o.buf)
    }

    /// The names of buffers with unsaved edits.
    pub fn unsaved(&self) -> Vec<&str> {
        self.open
            .iter()
            .filter(|o| o.buf.dirty())
            .map(|o| o.buf.name.as_str())
            .collect()
    }

    /// Whether keys are going into text, so screen-wide keys like `?` aren't theirs.
    pub fn typing(&self) -> bool {
        self.prompt.is_some() || self.keys.mode == Mode::Insert || self.keys.waiting()
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        if self.prompt.is_some() {
            return self.prompt_key(key);
        }
        let view = View {
            height: self.height,
        };
        let Some(open) = self.open.get_mut(self.cur) else {
            return Outcome::Back;
        };
        let action = self.keys.press(&mut open.buf, key, view);
        self.act(action);
        Outcome::None
    }

    /// Does what a key asked of the screen.
    fn act(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Command => self.prompt = Some(command::Prompt::new(command::Line::Command)),
            Action::Search => self.prompt = Some(command::Prompt::new(command::Line::Search)),
            Action::Copy(text) => {
                if let Err(e) = clipboard::copy(&text) {
                    self.notice = Some(format!("couldn't copy: {e}"));
                }
            }
            Action::Paste => match clipboard::paste() {
                Ok(text) => {
                    if let Some(open) = self.open.get_mut(self.cur) {
                        self.keys.paste_text(&mut open.buf, &text);
                    }
                }
                Err(e) => self.notice = Some(format!("couldn't paste: {e}")),
            },
            Action::Notice(n) => self.notice = Some(n),
        }
    }
}
