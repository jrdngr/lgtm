//! The editor screen: open buffers, the one in view, the `:` command line, and the keys.

mod clipboard;
mod command;
mod draw;
mod finder;

use std::path::{Path, PathBuf};

use lgtm_core::syntax::Segment;
use ratatui::crossterm::event::KeyEvent;

use crate::edit::buffer::{Buffer, Sel};
use crate::edit::fold::Folds;
use crate::edit::keymap::Pick;
use crate::edit::keys::{Action, Keys, Mode, View};
use crate::edit::motion;
use crate::tui::files::FileTree;

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
    folds: Folds,
}

impl Open {
    fn new(buf: Buffer) -> Self {
        Self {
            buf,
            top: 0,
            left: 0,
            colors: None,
            folds: Folds::default(),
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
    root: PathBuf,
    pub open: Vec<Open>,
    cur: usize,
    keys: Keys,
    /// The `:` or `/` line being typed.
    prompt: Option<command::Prompt>,
    /// A failure or warning to show until the next key.
    pub notice: Option<String>,
    /// Lines of text the editor had when last drawn.
    height: usize,
    /// The list `space` opened, over the text.
    finder: Option<finder::Finder>,
    /// The repo's files beside the text, open down to the buffer in view.
    sidebar: FileTree,
}

impl Editor {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            open: vec![],
            cur: 0,
            keys: Keys::default(),
            prompt: None,
            notice: None,
            height: 20,
            finder: None,
            sidebar: FileTree::default(),
        }
    }

    /// Takes the repo's files, for the sidebar and `space f`.
    pub fn set_files(&mut self, paths: Vec<String>) {
        self.sidebar.set_paths(paths);
    }

    /// Opens the file `name`, a path in the workspace, at `line`, from 1, or goes to it when
    /// it's open already. Line 0 leaves an open file's cursor where it was.
    pub fn open(&mut self, name: &str, line: u32) -> std::io::Result<()> {
        let (at, new) = match self.open.iter().position(|o| o.buf.name == name) {
            Some(i) => (i, false),
            None => {
                let buf = Buffer::open(&self.root.join(name), name)?;
                self.open.push(Open::new(buf));
                (self.open.len() - 1, true)
            }
        };
        self.cur = at;
        self.keys.mode = Mode::Normal;
        if line > 0 || new {
            go_line(&mut self.open[at].buf, line as usize);
        }
        self.sidebar.reveal(name);
        Ok(())
    }

    /// Opens a list of `kind` to pick from.
    pub fn pick(&mut self, kind: Pick) {
        let dirty = |p: &str| self.open.iter().any(|o| o.buf.name == p && o.buf.dirty());
        let paths: Vec<(String, bool)> = match kind {
            Pick::Buffers => self
                .open
                .iter()
                .map(|o| (o.buf.name.clone(), o.buf.dirty()))
                .collect(),
            Pick::Files | Pick::Grep => self
                .sidebar
                .paths()
                .iter()
                .map(|p| (p.clone(), dirty(p)))
                .collect(),
        };
        self.finder = Some(finder::Finder::new(kind, &self.root, paths));
    }

    fn finder_key(&mut self, key: KeyEvent) -> Outcome {
        let Some(f) = self.finder.as_mut() else {
            return Outcome::None;
        };
        match f.key(key) {
            finder::Outcome::None => {}
            finder::Outcome::Close => self.finder = None,
            finder::Outcome::Open(path, line) => {
                self.finder = None;
                if let Err(e) = self.open(&path, line) {
                    self.notice = Some(format!("couldn't open {path}: {e}"));
                }
            }
        }
        Outcome::None
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
        self.prompt.is_some()
            || self.finder.is_some()
            || self.keys.mode == Mode::Insert
            || self.keys.waiting()
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        if self.prompt.is_some() {
            return self.prompt_key(key);
        }
        if self.finder.is_some() {
            return self.finder_key(key);
        }
        let Some(open) = self.open.get_mut(self.cur) else {
            return Outcome::Back;
        };
        open.folds.update(&open.buf);
        let hidden = open.folds.hidden();
        let view = View {
            height: self.height,
            hidden: &hidden,
        };
        let action = self.keys.press(&mut open.buf, key, view);
        self.act(action);
        if let Some(open) = self.open.get_mut(self.cur) {
            // Whatever moved the cursor into a closed fold opens it.
            open.folds.update(&open.buf);
            let line = open.buf.line_of(open.buf.sel.head);
            open.folds.reveal(line);
        }
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
            Action::Pick(kind) => self.pick(kind),
            Action::Fold(fold) => {
                if let Some(open) = self.open.get_mut(self.cur) {
                    let line = open.buf.line_of(open.buf.sel.head);
                    open.folds.apply(fold, line);
                }
            }
        }
    }
}
