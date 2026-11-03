//! The editor screen: open buffers, the one in view, the `:` command line, and the keys.

mod clipboard;
mod command;
mod draw;
mod finder;
mod split;

pub use split::FLAG as SPLIT_FLAG;

use std::path::{Path, PathBuf};

use lgtm_core::syntax::Segment;
use ratatui::crossterm::event::KeyEvent;

use crate::edit::buffer::{Buffer, Sel};
use crate::edit::fold::Folds;
use crate::edit::gutter::Mark;
use crate::edit::keymap::Pick;
use crate::edit::keys::{Action, Keys, Mode, View};
use crate::edit::motion;
use crate::tui::files::FileTree;

pub enum Outcome {
    None,
    /// Leave the editor.
    Back,
    /// Go to the review, on this file's line from 1, which holds this text.
    Review(String, u32, String),
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
    /// The file at `@-`, for the diff gutter: `None` until it's read. A file not there is
    /// empty.
    base: Option<String>,
    /// Whether the base was asked for.
    base_asked: bool,
    /// The gutter's marks, and the buffer version they're for.
    marks: Option<(u64, Vec<Option<Mark>>)>,
    /// Whether the file changed on disk under unsaved edits.
    pub conflict: bool,
}

impl Open {
    fn new(buf: Buffer) -> Self {
        Self {
            buf,
            top: 0,
            left: 0,
            colors: None,
            folds: Folds::default(),
            base: None,
            base_asked: false,
            marks: None,
            conflict: false,
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
    /// Whether keys go to the sidebar.
    in_sidebar: bool,
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
            in_sidebar: false,
        }
    }

    /// The buffers whose base the gutter still needs, marked as asked for.
    pub fn take_base_requests(&mut self) -> Vec<String> {
        self.open
            .iter_mut()
            .filter_map(|o| {
                (!std::mem::replace(&mut o.base_asked, true)).then(|| o.buf.name.clone())
            })
            .collect()
    }

    /// Asks for every buffer's base again, after jj may have moved `@-`.
    pub fn bases_stale(&mut self) {
        self.open.iter_mut().for_each(|o| o.base_asked = false);
    }

    pub fn set_base(&mut self, name: &str, text: String) {
        if let Some(o) = self.open.iter_mut().find(|o| o.buf.name == name) {
            if o.base.as_deref() != Some(text.as_str()) {
                o.marks = None;
            }
            o.base = Some(text);
        }
    }

    /// Takes what other programs wrote to open files: a buffer with no unsaved edits reads
    /// it, and one with edits is marked as in conflict.
    pub fn check_disk(&mut self) {
        for o in &mut self.open {
            if let Some(text) = o.buf.disk_text() {
                if o.buf.dirty() {
                    o.conflict = true;
                } else {
                    o.buf.take_disk(&text);
                }
            }
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

    /// Opens `file`, else the buffer in view at its cursor's line, in a herdr pane to the
    /// right or below. Outside herdr `file` opens as a buffer here.
    pub fn split(&mut self, file: Option<&str>, right: bool) {
        let here = self
            .current()
            .map(|b| (b.name.clone(), b.line_of(b.sel.head) as u32 + 1));
        let (name, line) = match (file, here) {
            (Some(f), Some((name, line))) if f == name => (name, line),
            (Some(f), _) => (f.to_string(), 1),
            (None, Some(at)) => at,
            (None, None) => return,
        };
        let failed = if split::herdr() {
            split::open(&self.root.join(&name), line, right).err()
        } else if file.is_some() {
            self.open(&name, 0).err()
        } else {
            self.notice = Some("no herdr pane to split".into());
            return;
        };
        if let Some(e) = failed {
            self.notice = Some(format!("couldn't open {name}: {e}"));
        }
    }

    /// A click in the sidebar opens a file or a folder, and the wheel scrolls it.
    pub fn mouse(&mut self, ev: ratatui::crossterm::event::MouseEvent) {
        use ratatui::crossterm::event::{MouseButton, MouseEventKind};
        if self.finder.is_some() || !self.sidebar.contains(ev.column, ev.row) {
            return;
        }
        match ev.kind {
            MouseEventKind::ScrollDown => self.sidebar.scroll(3),
            MouseEventKind::ScrollUp => self.sidebar.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) => {
                if let crate::tui::files::Outcome::Open(path) = self.sidebar.click(ev.row) {
                    self.in_sidebar = false;
                    if let Err(e) = self.open(&path, 0) {
                        self.notice = Some(format!("couldn't open {path}: {e}"));
                    }
                }
            }
            _ => {}
        }
    }

    /// Moves in the sidebar's tree, opens and closes its folders, and opens its files.
    fn sidebar_key(&mut self, key: KeyEvent) {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let code = match key.code {
            KeyCode::Char('j') if !ctrl => KeyCode::Down,
            KeyCode::Char('k') if !ctrl => KeyCode::Up,
            KeyCode::Char('h') => KeyCode::Left,
            KeyCode::Char('l') => KeyCode::Right,
            KeyCode::Char('d') if ctrl => KeyCode::PageDown,
            KeyCode::Char('u') if ctrl => KeyCode::PageUp,
            KeyCode::Char('g') => KeyCode::Home,
            KeyCode::Char('G') => KeyCode::End,
            KeyCode::Esc | KeyCode::Tab | KeyCode::Char('q' | ' ') => {
                self.leave_sidebar();
                return;
            }
            code @ (KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Enter) => code,
            _ => return,
        };
        let key = KeyEvent::new(code, KeyModifiers::NONE);
        if let crate::tui::files::Outcome::Open(path) = self.sidebar.key(key) {
            match self.open(&path, 0) {
                Ok(()) => self.in_sidebar = false,
                Err(e) => self.notice = Some(format!("couldn't open {path}: {e}")),
            }
        }
    }

    /// Back to the text, the tree's cursor back on the buffer in view.
    fn leave_sidebar(&mut self) {
        self.in_sidebar = false;
        if let Some(name) = self.current().map(|b| b.name.clone()) {
            self.sidebar.reveal(&name);
        }
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
            finder::Outcome::Close => {
                self.finder = None;
                if self.open.is_empty() {
                    return Outcome::Back;
                }
            }
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
        if self.in_sidebar {
            self.sidebar_key(key);
            return Outcome::None;
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
        if action == Action::Review {
            let b = &open.buf;
            let line = b.line_of(b.sel.head);
            let text = b.slice(b.line_start(line), b.line_end(line));
            return Outcome::Review(b.name.clone(), line as u32 + 1, text);
        }
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
            Action::Review => {}
            Action::Sidebar => self.in_sidebar = true,
            Action::Split { right } => self.split(None, right),
            Action::Fold(fold) => {
                if let Some(open) = self.open.get_mut(self.cur) {
                    let line = open.buf.line_of(open.buf.sel.head);
                    open.folds.apply(fold, line);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor_on(text: &str) -> (tempfile::TempDir, Editor) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), text).unwrap();
        let mut e = Editor::new(dir.path());
        e.open("a.txt", 1).unwrap();
        (dir, e)
    }

    #[test]
    fn a_clean_buffer_takes_what_was_written_to_its_file() {
        let (dir, mut e) = editor_on("one\n");
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        e.check_disk();
        assert_eq!(e.open[0].buf.text.to_string(), "one\ntwo\n");
        assert!(!e.open[0].buf.dirty());
        assert!(!e.open[0].conflict);
    }

    #[test]
    fn a_buffer_with_edits_keeps_them_and_is_in_conflict_until_reload() {
        let (dir, mut e) = editor_on("one\n");
        e.open[0].buf.insert(0, "x");
        std::fs::write(dir.path().join("a.txt"), "one\ntwo\n").unwrap();
        e.check_disk();
        assert_eq!(e.open[0].buf.text.to_string(), "xone\n");
        assert!(e.open[0].conflict);
        e.run_command("reload");
        assert_eq!(e.open[0].buf.text.to_string(), "one\ntwo\n");
        assert!(!e.open[0].conflict);
        // Undo brings the edits back.
        e.open[0].buf.undo();
        assert_eq!(e.open[0].buf.text.to_string(), "xone\n");
    }

    fn press(e: &mut Editor, keys: &str) {
        for key in crate::edit::spec::keys(keys) {
            e.key(key);
        }
    }

    #[test]
    fn space_e_moves_to_the_file_tree_where_folders_open_and_enter_opens_a_file() {
        let (dir, mut e) = editor_on("one\n");
        std::fs::create_dir(dir.path().join("b")).unwrap();
        std::fs::write(dir.path().join("b/c.txt"), "see\n").unwrap();
        e.set_files(vec!["a.txt".into(), "b/c.txt".into()]);
        e.sidebar.reveal("a.txt");
        press(&mut e, "<space>ek");
        assert_eq!(e.sidebar.selected(), Some("b"));
        press(&mut e, "lj");
        assert_eq!(e.sidebar.selected(), Some("b/c.txt"));
        // Keys that edit text do nothing here.
        press(&mut e, "d");
        press(&mut e, "<ret>");
        assert_eq!(e.current().map(|b| b.name.as_str()), Some("b/c.txt"));
        assert_eq!(e.open[0].buf.text.to_string(), "one\n");
        press(&mut e, "d");
        assert_eq!(e.open[1].buf.text.to_string(), "ee\n");
        press(&mut e, "<space>e<esc>");
        assert_eq!(e.sidebar.selected(), Some("b/c.txt"));
    }
}
