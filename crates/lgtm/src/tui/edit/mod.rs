//! The editor screen: open buffers, the one in view, the `:` command line, and the keys.

mod click;
pub mod clipboard;
mod command;
pub(crate) mod draw;
mod finder;
pub mod help;
mod split;

pub use split::FLAG as SPLIT_FLAG;

use crate::tui::help::Group;
use std::path::{Path, PathBuf};

use lgtm_core::syntax::Segment;
use lgtm_tui_kit::markdown as preview;
use ratatui::crossterm::event::KeyEvent;

use crate::edit::buffer::{Buffer, Sel};
use crate::edit::fold::Folds;
use crate::edit::gutter::Mark;
use crate::edit::keymap::Pick;
use crate::edit::keys::{Action, Keys, Mode, View};
use crate::edit::motion;
use crate::tui::files::FileTree;
use crate::tui::resolve::{self, Resolver};
use crate::tui::width;

pub enum Outcome {
    None,
    /// Quit lgtm.
    Quit,
    /// Go to the home screen, keeping the open files.
    Home,
    /// Go to the review, on this file's line from 1, which holds this text.
    Review(String, u32, String),
    /// Send where these lines are to an agent.
    Mention(String),
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
    /// Resolving the file's conflicts, which then shows in place of its text.
    pub resolve: Option<Box<Resolver>>,
    /// Whether jj was asked about the conflicts' sides.
    resolve_asked: bool,
    /// The markdown rendered, which then shows in place of its text.
    preview: Option<Box<preview::Preview>>,
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
            resolve: None,
            resolve_asked: false,
            preview: None,
        }
    }

    /// Resolves the file's conflicts, when it has any.
    fn start_resolving(&mut self) -> bool {
        let text = self.buf.text.to_string();
        if !lgtm_core::resolve::has_conflicts(&text) {
            return false;
        }
        self.resolve = Some(Box::new(Resolver::new(&self.buf.name, &text)));
        self.resolve_asked = false;
        true
    }

    /// Whether it has edits or picks that saving would write.
    pub fn dirty(&self) -> bool {
        self.buf.dirty() || self.resolve.as_ref().is_some_and(|r| r.dirty())
    }
}

/// Opens a link from a preview: a web address in the browser, else a path from the
/// workspace in whatever opens it.
fn open_link(url: &str) {
    let _ = std::process::Command::new("open")
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
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
    /// The text area's width when `space b` showed the sidebar, so it shows on a narrow
    /// screen too until the screen gets narrower.
    files_asked: Option<u16>,
    /// The text area's width as last drawn, with the sidebar.
    body_width: u16,
    /// Whether a file was saved since this was last cleared.
    pub saved: bool,
    /// The sidebar's width with its border, once `<` or `>` set it.
    pub files_width: Option<u16>,
    /// Whether long lines wrap at the view's edge.
    pub wrap: bool,
    /// Whether markdown files open rendered: as the last one was left.
    pub render_markdown: bool,
    /// The sidebar's width as last drawn, 0 when hidden.
    sidebar_width: u16,
    /// The rows of text as last drawn, top down.
    rows: Vec<click::TextRow>,
    /// Where a drag over the text started.
    drag_from: Option<usize>,
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
            files_asked: None,
            body_width: 0,
            saved: false,
            files_width: None,
            wrap: false,
            render_markdown: false,
            sidebar_width: 0,
            rows: vec![],
            drag_from: None,
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

    /// The files being resolved whose sides jj should fill in, marked as asked for.
    pub fn take_resolve_requests(&mut self) -> Vec<(String, String)> {
        self.open
            .iter_mut()
            .filter_map(|o| {
                let ask = o.resolve.is_some() && !std::mem::replace(&mut o.resolve_asked, true);
                ask.then(|| (o.buf.name.clone(), o.buf.text.to_string()))
            })
            .collect()
    }

    /// Takes the sides jj filled in for the file `name`, when its text is still `text`.
    pub fn set_resolve(&mut self, name: &str, text: &str, filled: &lgtm_core::resolve::Session) {
        if let Some(o) = self.open.iter_mut().find(|o| o.buf.name == name)
            && o.buf.text == text
            && let Some(r) = o.resolve.as_mut()
        {
            r.adopt(filled);
        }
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
                if o.dirty() {
                    o.conflict = true;
                } else {
                    o.buf.take_disk(&text);
                    if o.resolve.is_some() && !o.start_resolving() {
                        o.resolve = None;
                    }
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
        let dirty = |p: &str| self.open.iter().any(|o| o.buf.name == p && o.dirty());
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
                let mut open = Open::new(buf);
                open.start_resolving();
                self.open.push(open);
                (self.open.len() - 1, true)
            }
        };
        self.cur = at;
        self.keys.mode = Mode::Normal;
        if line > 0 || new {
            go_line(&mut self.open[at].buf, line as usize);
        }
        let open = &mut self.open[at];
        if new && self.render_markdown && crate::edit::markdown::is_markdown(&open.buf.name) {
            let top = open.buf.line_of(open.buf.sel.head);
            open.preview = Some(Box::new(preview::Preview::new(top)));
        }
        if !Path::new(name).is_absolute() {
            self.sidebar.reveal(name);
        }
        Ok(())
    }

    /// Opens `file`, else the buffer in view at its cursor's line, in an lgterm or herdr pane
    /// to the right or below. Elsewhere `file` opens as a buffer here.
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
        let failed = if let Some(host) = split::host() {
            split::open(&host, &self.root.join(&name), line, right).err()
        } else if file.is_some() {
            self.open(&name, 0).err()
        } else {
            self.notice = Some("no pane to split".into());
            return;
        };
        if let Some(e) = failed {
            self.notice = Some(format!("couldn't open {name}: {e}"));
        }
    }

    /// A click in the sidebar opens a file or a folder, and the wheel scrolls it. Over the
    /// text the wheel scrolls the view three lines a notch, and the cursor with it, as the
    /// review does, and a click or a drag moves the cursor or selects.
    pub fn mouse(&mut self, ev: ratatui::crossterm::event::MouseEvent) {
        use ratatui::crossterm::event::{
            KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
        };
        if self.finder.is_some() {
            return;
        }
        if !self.sidebar.contains(ev.column, ev.row)
            && let Some(p) = self.open.get_mut(self.cur).and_then(|o| o.preview.as_mut())
        {
            match ev.kind {
                MouseEventKind::ScrollDown => p.scroll(3),
                MouseEventKind::ScrollUp => p.scroll(-3),
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(url) = p.link_at(ev.column, ev.row) {
                        open_link(url);
                    }
                }
                _ => {}
            }
            return;
        }
        if !self.sidebar.contains(ev.column, ev.row) || self.drag_from.is_some() {
            let (code, by) = match ev.kind {
                MouseEventKind::ScrollDown => (KeyCode::Down, 3),
                MouseEventKind::ScrollUp => (KeyCode::Up, -3),
                kind => return self.text_mouse(kind, ev.column, ev.row),
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

    /// Shows the markdown file in view rendered, or its text again at the line that was at
    /// the top.
    fn toggle_preview(&mut self) {
        let Some(open) = self.open.get_mut(self.cur) else {
            return;
        };
        if let Some(p) = open.preview.take() {
            let line = p.src_line();
            open.top = line;
            go_line(&mut open.buf, line + 1);
            self.render_markdown = false;
            return;
        }
        if !crate::edit::markdown::is_markdown(&open.buf.name) {
            self.notice = Some("not markdown".into());
            return;
        }
        self.keys.mode = Mode::Normal;
        open.preview = Some(Box::new(preview::Preview::new(open.top)));
        self.render_markdown = true;
    }

    /// Shows the next open file, or the one before, wrapping around.
    fn cycle_file(&mut self, forward: bool) {
        let n = self.open.len();
        if n < 2 {
            return;
        }
        self.cur = if forward {
            (self.cur + 1) % n
        } else {
            (self.cur + n - 1) % n
        };
        self.keys.mode = Mode::Normal;
        if let Some(name) = self.current().map(|b| b.name.clone())
            && !Path::new(&name).is_absolute()
        {
            self.sidebar.reveal(&name);
        }
    }

    /// Whether unsaved edits keep the editor from quitting. If so, says which files and
    /// shows one: the file in view when it's one of them, else the first.
    pub fn blocks_quit(&mut self) -> bool {
        let unsaved = self.unsaved();
        if unsaved.is_empty() {
            return false;
        }
        self.notice = Some(format!("unsaved: {}", unsaved.join(", ")));
        if !self.open.get(self.cur).is_some_and(|o| o.dirty()) {
            let Some(i) = self.open.iter().position(|o| o.dirty()) else {
                return true;
            };
            self.cur = i;
            self.keys.mode = Mode::Normal;
            if let Some(name) = self.current().map(|b| b.name.clone())
                && !Path::new(&name).is_absolute()
            {
                self.sidebar.reveal(&name);
            }
        }
        self.in_sidebar = false;
        true
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
            .filter(|o| o.dirty())
            .map(|o| o.buf.name.as_str())
            .collect()
    }

    /// Whether keys are going into text, so screen-wide keys like `?` aren't theirs.
    pub fn typing(&self) -> bool {
        if let Some(r) = self.resolver() {
            return self.prompt.is_some() || self.finder.is_some() || r.typing();
        }
        self.prompt.is_some()
            || self.finder.is_some()
            || self.keys.mode == Mode::Insert
            || self.keys.waiting()
    }

    /// Whether the file in view shows rendered.
    pub fn previewing(&self) -> bool {
        self.open.get(self.cur).is_some_and(|o| o.preview.is_some())
    }

    /// The resolver of the file in view, when it's resolving.
    pub fn resolver(&self) -> Option<&Resolver> {
        self.open.get(self.cur)?.resolve.as_deref()
    }

    /// The keys for `?`, in groups.
    pub fn help_groups(&self) -> Vec<Group> {
        if self.resolver().is_some() {
            resolve::help_groups()
        } else if self.previewing() {
            help::preview_groups()
        } else {
            help::groups()
        }
    }

    /// Takes text the terminal pasted: into the line being typed, else the text.
    pub fn paste(&mut self, text: &str) {
        self.notice = None;
        if let Some(p) = self.prompt.as_mut() {
            p.text.push_str(&text.replace(['\n', '\r'], " "));
            return;
        }
        if self.finder.is_some() || self.in_sidebar || self.previewing() {
            return;
        }
        if let Some(r) = self.open.get_mut(self.cur).and_then(|o| o.resolve.as_mut()) {
            r.paste(text);
            return;
        }
        if let Some(open) = self.open.get_mut(self.cur) {
            self.keys
                .put_text(&mut open.buf, &text.replace("\r\n", "\n"));
        }
    }

    /// ⌘C copies the selection and ⌘V pastes, where the terminal passes them on.
    fn command_key(&mut self, key: &KeyEvent) -> bool {
        use ratatui::crossterm::event::{KeyCode, KeyModifiers};
        if !key.modifiers.contains(KeyModifiers::SUPER)
            || self.prompt.is_some()
            || self.resolver().is_some()
            || self.previewing()
        {
            return false;
        }
        match key.code {
            KeyCode::Char('c') => {
                if let Some(b) = self.current().filter(|b| b.len() > 0) {
                    let text = b.slice(b.sel.from(), b.sel.to() + 1);
                    self.act(Action::Copy(text));
                }
                true
            }
            KeyCode::Char('v') => {
                match clipboard::paste() {
                    Ok(text) => self.paste(&text),
                    Err(e) => self.notice = Some(format!("couldn't paste: {e}")),
                }
                true
            }
            _ => false,
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        if self.finder.is_none() && !self.in_sidebar && self.command_key(&key) {
            return Outcome::None;
        }
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
        if let Some(r) = self.open.get_mut(self.cur).and_then(|o| o.resolve.as_mut()) {
            match r.key(key) {
                resolve::Outcome::None => {}
                resolve::Outcome::Command => self.act(Action::Command),
                resolve::Outcome::Notice(n) => self.notice = Some(n),
                resolve::Outcome::Copy(text) => self.act(Action::Copy(text)),
            }
            return Outcome::None;
        }
        if let Some(p) = self.open.get_mut(self.cur).and_then(|o| o.preview.as_mut()) {
            match p.key(key) {
                preview::Outcome::None => {}
                preview::Outcome::Command => self.act(Action::Command),
                preview::Outcome::Close => self.toggle_preview(),
            }
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
        if action == Action::SendToAgent {
            let b = &open.buf;
            let (from, to) = (b.line_of(b.sel.from()), b.line_of(b.sel.to()));
            return Outcome::Mention(crate::tui::mention::lines(&b.name, from + 1, to + 1));
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
            Action::CycleFile { forward } => self.cycle_file(forward),
            Action::Preview => self.toggle_preview(),
            Action::Review | Action::SendToAgent => {}
            Action::Sidebar => {
                self.files = true;
                self.in_sidebar = true;
            }
            Action::ToggleSidebar => {
                // It flips what shows: a sidebar hidden for want of room comes up.
                let shown = self.sidebar_width > 0;
                self.files = !shown;
                self.files_asked = (!shown).then_some(self.body_width);
                self.in_sidebar = false;
            }
            Action::ToggleWrap => self.wrap = !self.wrap,
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
    fn markdown_files_open_rendered_or_not_as_the_last_one_was_left() {
        let dir = tempfile::tempdir().unwrap();
        for f in ["a.md", "b.md", "c.md", "d.txt"] {
            std::fs::write(dir.path().join(f), "# Title\n").unwrap();
        }
        let mut e = Editor::new(dir.path());
        let rendered = |e: &Editor| e.previewing();
        e.open("a.md", 1).unwrap();
        assert!(!rendered(&e));
        press(&mut e, "<space>m");
        assert!(rendered(&e) && e.render_markdown);
        e.open("b.md", 1).unwrap();
        assert!(rendered(&e));
        e.open("d.txt", 1).unwrap();
        assert!(!rendered(&e));
        e.open("b.md", 1).unwrap();
        press(&mut e, "<space>m");
        assert!(!rendered(&e) && !e.render_markdown);
        e.open("c.md", 1).unwrap();
        assert!(!rendered(&e));
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
        let (dir, mut e) = editor_on("one\n");
        assert!(matches!(quit(&mut e), Outcome::Quit));
        e.open[0].buf.insert(0, "x");
        assert!(matches!(quit(&mut e), Outcome::None));
        assert!(e.notice.as_deref().is_some_and(|n| n.contains("a.txt")));

        // The unsaved file comes into view.
        std::fs::write(dir.path().join("b.txt"), "two\n").unwrap();
        e.open("b.txt", 1).unwrap();
        e.in_sidebar = true;
        assert!(matches!(quit(&mut e), Outcome::None));
        assert_eq!(e.current().map(|b| b.name.as_str()), Some("a.txt"));
        assert!(!e.in_sidebar);
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
    fn space_w_wraps_long_lines_and_the_view_follows_the_cursor_down_the_rows() {
        use ratatui::buffer::Buffer as Screen;
        use ratatui::layout::Rect;
        let long = "abcdefghijklmnopqrstuvwxyz";
        let (_dir, mut e) = editor_on(&format!("{long}\nend\n").repeat(3));
        e.files = false;
        // Three columns of numbers and three of gutter leave ten for the text.
        let area = Rect::new(0, 0, 16, 4);
        let draw = |e: &mut Editor| {
            let mut screen = Screen::empty(area);
            e.draw(&mut crate::tui::paint::Canvas::new(&mut screen, area), area);
            (0..area.height)
                .map(|y| {
                    (6..area.width)
                        .map(|x| screen[(x, y)].symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_string()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(draw(&mut e), ["abcdefghij", "end", "abcdefghij", "end"]);
        press(&mut e, "<space>w");
        assert_eq!(draw(&mut e), ["abcdefghij", "klmnopqrst", "uvwxyz", "end"]);
        // The second long line's last row is the fifth row down, so the view moves a line.
        press(&mut e, "jj$");
        assert_eq!(draw(&mut e), ["end", "abcdefghij", "klmnopqrst", "uvwxyz"]);
    }

    #[test]
    fn clicking_moves_the_cursor_and_dragging_selects_in_any_mode() {
        use ratatui::buffer::Buffer as Screen;
        use ratatui::crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
        use ratatui::layout::Rect;
        let (_dir, mut e) = editor_on("one\n\ttwo\nthree\n");
        e.files = false;
        let area = Rect::new(0, 0, 30, 6);
        e.draw(
            &mut crate::tui::paint::Canvas::new(&mut Screen::empty(area), area),
            area,
        );
        // The text starts at column 6, after the gutter and the numbers.
        let mouse = |e: &mut Editor, kind, column, row| {
            e.mouse(MouseEvent {
                kind,
                column,
                row,
                modifiers: KeyModifiers::NONE,
            })
        };
        let sel = |e: &Editor| e.current().map(|b| (b.sel.anchor, b.sel.head)).unwrap();
        let down = MouseEventKind::Down(MouseButton::Left);
        let drag = MouseEventKind::Drag(MouseButton::Left);
        let up = MouseEventKind::Up(MouseButton::Left);

        press(&mut e, "i");
        // Past the tab's four columns, on the "w" of "two".
        mouse(&mut e, down, 11, 1);
        mouse(&mut e, up, 11, 1);
        assert_eq!((sel(&e), e.keys.mode), ((6, 6), Mode::Insert));

        // Past the end of "one", the cursor goes on its newline.
        mouse(&mut e, down, 20, 0);
        assert_eq!(sel(&e), (3, 3));
        mouse(&mut e, drag, 8, 2);
        mouse(&mut e, up, 8, 2);
        assert_eq!((sel(&e), e.keys.mode), ((3, 11), Mode::Normal));

        press(&mut e, "v");
        mouse(&mut e, down, 6, 0);
        mouse(&mut e, drag, 7, 0);
        assert_eq!((sel(&e), e.keys.mode), ((0, 1), Mode::Select));
    }

    #[test]
    fn the_file_tree_hides_on_a_narrow_screen_until_space_b_shows_it() {
        use ratatui::buffer::Buffer as Screen;
        use ratatui::layout::Rect;
        let (dir, mut e) = editor_on("one\n");
        e.set_files(crate::tui::walk::list(dir.path()));
        let draw = |e: &mut Editor, width| {
            let area = Rect::new(0, 0, width, 10);
            let mut screen = Screen::empty(area);
            e.draw(&mut crate::tui::paint::Canvas::new(&mut screen, area), area);
            e.sidebar_width > 0
        };
        assert!(draw(&mut e, 120));
        assert!(!draw(&mut e, 80));
        press(&mut e, "<space>b");
        assert!(draw(&mut e, 80));
        press(&mut e, "<space>b");
        assert!(!draw(&mut e, 80));
        // Shown by hand, it still goes when the screen gets narrower.
        press(&mut e, "<space>b");
        assert!(draw(&mut e, 80));
        assert!(!draw(&mut e, 76));
        assert!(!draw(&mut e, 80));
    }

    #[test]
    fn space_a_sends_the_selected_lines() {
        let (_dir, mut e) = editor_on("one\ntwo\nthree\n");
        let out = press_last(&mut e, "j<space>a");
        assert!(matches!(out, Outcome::Mention(t) if t == "a.txt:2"));
        press(&mut e, "xx");
        let out = press_last(&mut e, "<space>a");
        assert!(matches!(out, Outcome::Mention(t) if t == "a.txt:2-3"));
    }

    fn press_last(e: &mut Editor, keys: &str) -> Outcome {
        let mut out = Outcome::None;
        for key in crate::edit::spec::keys(keys) {
            out = e.key(key);
        }
        out
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
    fn wc_saves_and_closes_and_close_bang_drops_edits() {
        let (dir, mut e) = editor_on("one\n");
        let path = dir.path().join("a.txt");
        e.open[0].buf.insert(0, "zero\n");
        e.run_command("wc");
        assert!(e.open.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "zero\none\n");

        e.run_command(&format!("e {}", path.display()));
        e.open[0].buf.insert(0, "dropped\n");
        e.run_command("c!");
        assert!(e.open.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "zero\none\n");
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

    #[test]
    fn a_terminal_paste_types_into_insert_mode_the_command_line_or_after_the_selection() {
        let (_dir, mut e) = editor_on("ab\n");
        press(&mut e, "i");
        e.paste("x\r\ny");
        press(&mut e, "<esc>");
        assert_eq!(e.open[0].buf.text.to_string(), "x\nyab\n");
        e.open[0].buf.sel = Sel::point(0);
        e.paste("Z");
        assert_eq!(e.open[0].buf.text.to_string(), "xZ\nyab\n");
        press(&mut e, ":");
        e.paste("w\n");
        assert_eq!(e.prompt.as_ref().map(|p| p.text.as_str()), Some("w "));
    }

    #[test]
    fn g_n_and_g_p_step_through_the_open_files_and_wrap_around() {
        let (dir, mut e) = editor_on("a\n");
        for name in ["b.txt", "c.txt"] {
            std::fs::write(dir.path().join(name), "x\n").unwrap();
            e.open(name, 1).unwrap();
        }
        let name = |e: &Editor| e.current().map(|b| b.name.clone()).unwrap();
        press(&mut e, "gn");
        assert_eq!(name(&e), "a.txt");
        press(&mut e, "gn");
        assert_eq!(name(&e), "b.txt");
        press(&mut e, "gp");
        press(&mut e, "gp");
        assert_eq!(name(&e), "c.txt");
    }

    #[test]
    fn space_m_shows_a_markdown_file_rendered_and_back_at_the_line_it_scrolled_to() {
        let dir = tempfile::tempdir().unwrap();
        let md: String = (0..40).map(|i| format!("para {i}\n\n")).collect();
        std::fs::write(dir.path().join("a.md"), md).unwrap();
        std::fs::write(dir.path().join("b.txt"), "x\n").unwrap();
        let mut e = Editor::new(dir.path());
        e.open("b.txt", 1).unwrap();
        press(&mut e, " m");
        assert!(!e.previewing());
        assert_eq!(e.notice.as_deref(), Some("not markdown"));
        e.open("a.md", 1).unwrap();
        press(&mut e, " m");
        assert!(e.previewing());
        let area = ratatui::layout::Rect::new(0, 0, 60, 10);
        let mut screen = ratatui::buffer::Buffer::empty(area);
        e.files = false;
        e.draw(&mut crate::tui::paint::Canvas::new(&mut screen, area), area);
        // Typing doesn't reach the text.
        press(&mut e, "ixjjjj");
        assert!(!e.open[1].buf.dirty());
        press(&mut e, " m");
        assert!(!e.previewing());
        let b = e.current().unwrap();
        assert_eq!(b.line_of(b.sel.head), 4);
    }
}
