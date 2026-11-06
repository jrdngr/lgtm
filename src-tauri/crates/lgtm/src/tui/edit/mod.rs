//! The editor screen: open buffers, the one in view, the `:` command line, and the keys.

pub mod clipboard;
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
use crate::tui::width;

pub enum Outcome {
    None,
    /// Leave the editor.
    Back,
    /// Go to the home screen, keeping the open files.
    Home,
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
    /// Whether the sidebar shows, when there's room.
    pub files: bool,
    /// Whether a file was saved since this was last cleared.
    pub saved: bool,
    /// The sidebar's width with its border, once `<` or `>` set it.
    pub files_width: Option<u16>,
    /// The sidebar's width as last drawn, 0 when hidden.
    sidebar_width: u16,
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
            files: true,
            saved: false,
            files_width: None,
            sidebar_width: 0,
        }
    }

    /// The buffers whose base the gutter still needs, marked as asked for. Files outside
    /// the root and files git ignores have none, and the files must be listed to know which
    /// are ignored.
    pub fn take_base_requests(&mut self) -> Vec<String> {
        if !self.sidebar.loaded {
            return vec![];
        }
        let sidebar = &self.sidebar;
        self.open
            .iter_mut()
            .filter_map(|o| {
                (!std::mem::replace(&mut o.base_asked, true)).then(|| o.buf.name.clone())
            })
            .filter(|name| !Path::new(name).is_absolute() && !sidebar.is_ignored(name))
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

    /// Takes how the working copy changes files, for the sidebar.
    pub fn set_decor(&mut self, decor: crate::tui::decor::Decorations) {
        self.sidebar.decor = decor;
    }

    /// Takes the files just written, for the sidebar's heat.
    pub fn written(&mut self, paths: &[String]) {
        self.sidebar.heat.written(paths);
    }

    /// Whether the sidebar shows heat.
    pub fn heat_on(&self) -> bool {
        self.sidebar.heat_on
    }

    pub fn set_heat_on(&mut self, on: bool) {
        self.sidebar.heat_on = on;
    }

    /// Takes the repo's files, for the sidebar and `space f`.
    pub fn set_files(&mut self, listing: crate::tui::walk::Listing) {
        self.sidebar.set_listing(listing);
        if self
            .finder
            .as_ref()
            .is_some_and(|f| f.kind != Pick::OpenFiles)
        {
            let paths = self.listed();
            if let Some(f) = self.finder.as_mut() {
                f.set_paths(paths);
            }
        }
    }

    /// The files git doesn't ignore, each with whether it has unsaved edits.
    fn listed(&self) -> Vec<(String, bool)> {
        let dirty = |p: &str| self.open.iter().any(|o| o.buf.name == p && o.buf.dirty());
        self.sidebar
            .files()
            .map(|p| (p.clone(), dirty(p)))
            .collect()
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
        if !Path::new(name).is_absolute() {
            self.sidebar.reveal(name);
        }
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

    /// A click in the sidebar opens a file or a folder, and the wheel scrolls it. Over the
    /// text the wheel scrolls the view three lines a notch, and the cursor with it, as the
    /// review does.
    pub fn mouse(&mut self, ev: ratatui::crossterm::event::MouseEvent) {
        use ratatui::crossterm::event::{
            KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
        };
        if self.finder.is_some() {
            return;
        }
        if !self.sidebar.contains(ev.column, ev.row) {
            let (code, by) = match ev.kind {
                MouseEventKind::ScrollDown => (KeyCode::Down, 3),
                MouseEventKind::ScrollUp => (KeyCode::Up, -3),
                _ => return,
            };
            self.scroll_view(by);
            for _ in 0..3 {
                self.key(KeyEvent::new(code, KeyModifiers::NONE));
            }
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
        if let Some(step) = crate::tui::side::files_step(&key) {
            self.files_width = Some(width::moved(self.sidebar_width, step, draw::SIDEBAR_MIN));
            return;
        }
        let code = match key.code {
            KeyCode::Char('j') if !ctrl => KeyCode::Down,
            KeyCode::Char('k') if !ctrl => KeyCode::Up,
            KeyCode::Char('h') => KeyCode::Left,
            KeyCode::Char('l') => KeyCode::Right,
            KeyCode::Char('d') if ctrl => KeyCode::PageDown,
            KeyCode::Char('u') if ctrl => KeyCode::PageUp,
            KeyCode::Char('g') => KeyCode::Home,
            KeyCode::Char('G') => KeyCode::End,
            KeyCode::Esc | KeyCode::Tab | KeyCode::Char(' ') => {
                if !self.open.is_empty() {
                    self.leave_sidebar();
                }
                return;
            }
            KeyCode::Char(':') => {
                self.prompt = Some(command::Prompt::new(command::Line::Command));
                return;
            }
            KeyCode::Char('a') => {
                self.prompt = Some(command::Prompt::new(command::Line::New));
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

    /// Puts the keys in the sidebar, as when no file is open.
    pub fn browse(&mut self) {
        self.in_sidebar = true;
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
        let paths: Vec<(String, bool)> = match kind {
            Pick::OpenFiles => self
                .open
                .iter()
                .map(|o| (o.buf.name.clone(), o.buf.dirty()))
                .collect(),
            Pick::Files | Pick::Grep => self.listed(),
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
                    self.in_sidebar = true;
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

    /// Moves the view `by` lines that show, keeping the last line at the bottom at most.
    fn scroll_view(&mut self, by: isize) {
        let height = self.height;
        let Some(open) = self.open.get_mut(self.cur) else {
            return;
        };
        let visible = open.folds.visible(open.buf.last_line());
        let at = visible.partition_point(|&l| l < open.top) as isize + by;
        let most = visible.len().saturating_sub(height) as isize;
        open.top = visible
            .get(at.clamp(0, most.max(0)) as usize)
            .copied()
            .unwrap_or(0);
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
        if self.in_sidebar || self.open.is_empty() {
            self.in_sidebar = true;
            self.sidebar_key(key);
            return Outcome::None;
        }
        let Some(open) = self.open.get_mut(self.cur) else {
            return Outcome::None;
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
            Action::Sidebar => {
                self.files = true;
                self.in_sidebar = true;
            }
            Action::ToggleSidebar => {
                self.files = !self.files;
                self.in_sidebar = false;
            }
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
    fn the_wheel_over_the_text_scrolls_the_view_and_the_cursor_with_it() {
        use ratatui::crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
        let (_dir, mut e) = editor_on(&"x\n".repeat(10));
        let line = |e: &Editor| e.current().map(|b| b.line_of(b.sel.head));
        let start = line(&e).unwrap();
        let wheel = |kind| MouseEvent {
            kind,
            column: 50,
            row: 5,
            modifiers: KeyModifiers::NONE,
        };
        e.height = 4;
        e.mouse(wheel(MouseEventKind::ScrollDown));
        assert_eq!((line(&e), e.open[0].top), (Some(start + 3), 3));
        e.mouse(wheel(MouseEventKind::ScrollDown));
        e.mouse(wheel(MouseEventKind::ScrollDown));
        // The view stops with the last line at the bottom.
        assert_eq!(e.open[0].top, 6);
        e.mouse(wheel(MouseEventKind::ScrollUp));
        assert_eq!(e.open[0].top, 3);
    }

    #[test]
    fn colon_q_leaves_the_editor_but_not_with_unsaved_edits() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        let quit = |e: &mut Editor| {
            for c in [':', 'q'] {
                e.key(KeyEvent::from(KeyCode::Char(c)));
            }
            e.key(KeyEvent::from(KeyCode::Enter))
        };
        let (_dir, mut e) = editor_on("one\n");
        assert!(matches!(quit(&mut e), Outcome::Back));
        e.open[0].buf.insert(0, "x");
        assert!(matches!(quit(&mut e), Outcome::None));
        assert!(e.notice.as_deref().is_some_and(|n| n.contains("a.txt")));
    }

    #[test]
    fn colon_h_goes_home_even_with_unsaved_edits_which_stay_open() {
        use ratatui::crossterm::event::{KeyCode, KeyEvent};
        let (_dir, mut e) = editor_on("one\n");
        e.open[0].buf.insert(0, "x");
        for c in [':', 'h'] {
            e.key(KeyEvent::from(KeyCode::Char(c)));
        }
        assert!(matches!(
            e.key(KeyEvent::from(KeyCode::Enter)),
            Outcome::Home
        ));
        assert_eq!(e.unsaved(), ["a.txt"]);
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
        e.set_files(crate::tui::walk::list(dir.path()));
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

    #[test]
    fn e_opens_a_new_file_that_w_creates_and_close_leaves_the_list_of_files() {
        let (dir, mut e) = editor_on("one\n");
        e.set_files(crate::tui::walk::list(dir.path()));
        let new = dir.path().join("b/new.txt");
        e.run_command(&format!("e {}", new.display()));
        assert_eq!(e.current().map(|b| b.name.as_str()), Some("b/new.txt"));
        e.open[1].buf.insert(0, "hi\n");
        assert!(matches!(e.run_command("close"), Outcome::None));
        assert_eq!(e.open.len(), 2);
        e.run_command("wa");
        assert_eq!(std::fs::read_to_string(&new).unwrap(), "hi\n");
        assert!(e.sidebar.files().any(|p| p == "b/new.txt"));
        e.run_command("c");
        e.run_command("c");
        assert!(e.open.is_empty());
        assert!(e.in_sidebar && e.finder.is_none());
    }

    #[test]
    fn with_no_file_open_keys_stay_in_the_sidebar_where_a_makes_a_file_in_its_folder() {
        let (dir, mut e) = editor_on("one\n");
        std::fs::create_dir(dir.path().join("b")).unwrap();
        std::fs::write(dir.path().join("b/c.txt"), "see\n").unwrap();
        e.set_files(crate::tui::walk::list(dir.path()));
        e.run_command("close");
        press(&mut e, "<esc><space>");
        assert!(e.in_sidebar);
        e.sidebar.reveal("b/c.txt");
        press(&mut e, "anew/d.txt<ret>");
        let made = dir.path().join("b/new/d.txt");
        assert_eq!(std::fs::read_to_string(&made).unwrap(), "");
        assert_eq!(e.current().map(|b| b.name.as_str()), Some("b/new/d.txt"));
        assert!(!e.in_sidebar);
        assert!(e.sidebar.files().any(|p| p == "b/new/d.txt"));
        // An existing file opens as it is.
        press(&mut e, "<space>e");
        e.sidebar.reveal("a.txt");
        press(&mut e, "aa.txt<ret>");
        assert_eq!(
            e.current().map(|b| b.text.to_string()),
            Some("one\n".into())
        );
    }

    #[test]
    fn w_with_a_path_saves_there_and_edits_that_file_after() {
        let (dir, mut e) = editor_on("one\n");
        e.open[0].buf.insert(0, "x");
        let out = std::env::temp_dir().join(format!("lgtm-save-as-{}", std::process::id()));
        e.run_command(&format!("w {}", out.display()));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "xone\n");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("a.txt")).unwrap(),
            "one\n"
        );
        assert_eq!(e.open[0].buf.name, out.to_string_lossy());
        std::fs::remove_file(out).unwrap();
    }

    #[test]
    fn the_gutter_has_no_base_for_ignored_files_or_files_outside() {
        let (dir, mut e) = editor_on("one\n");
        std::fs::write(dir.path().join(".gitignore"), "*.log\n").unwrap();
        std::fs::write(dir.path().join("b.log"), "").unwrap();
        e.open("b.log", 1).unwrap();
        e.open("/etc/hostname", 1).unwrap();
        // Not until the files are listed.
        assert!(e.take_base_requests().is_empty());
        e.set_files(crate::tui::walk::list(dir.path()));
        e.bases_stale();
        assert_eq!(e.take_base_requests(), ["a.txt"]);
    }
}
