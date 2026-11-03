//! The terminal UI: a picker over the jj log, the review of what was picked, and the editor.

mod edit;
mod files;
mod github;
mod highlight;
mod home;
mod keybar;
mod load;
mod paint;
mod picker;
mod review;
mod serve;
mod settings;
mod term;
mod theme;
mod versions;
mod watch;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use lgtm_core::jj::Jj;
use lgtm_core::paths;
use lgtm_core::store::Store;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

use load::{Loader, Msg, Target};
use paint::Canvas;
use review::Review;
use settings::Settings;
use term::Term;
use theme::{ACCENT, BG, CYAN, FG, PANEL};

/// The store of the repo the workspace at `root` belongs to, shared with the desktop app.
pub fn store(root: &Path) -> Option<Store> {
    paths::data_dir()
        .zip(paths::find_root(root))
        .and_then(|(data, main)| Store::open(&paths::repo_dir(&data, &main).join("lgtm.db")).ok())
}

/// What the UI opens on.
pub enum Start {
    Home,
    Revset(String),
    /// A file, as given relative to the current directory, at a line from 1.
    File(PathBuf, u32),
}

/// Which screen is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Home,
    Review,
    Editor,
}

/// Runs the UI in the jj workspace around the current directory, opening `start`.
pub fn run(start: Start, pr: Option<String>) -> i32 {
    let Some(root) = std::env::current_dir()
        .ok()
        .and_then(|d| crate::agent::workspace(&d))
    else {
        eprintln!("lgtm: not in a jj repo");
        return 1;
    };
    let (tx, rx) = mpsc::channel();
    let server = match crate::agent::socket_path(&root).map(|p| serve::start(&p, tx.clone())) {
        Some(Ok(s)) => Some(s),
        Some(Err(e)) => {
            eprintln!("lgtm: {e:#}");
            return 1;
        }
        None => None,
    };
    let loader = match Loader::new(Jj::new(&root), tx) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            return 1;
        }
    };
    let store = store(&root);
    let buffers = edit::Editor::new(&root);
    let snapshots = Arc::new(AtomicBool::new(false));
    let watcher = loader.watch(&root, snapshots.clone()).ok();
    let mut app = App {
        root,
        loader,
        _watcher: watcher,
        _server: server,
        snapshots,
        version: store.as_ref().and_then(|s| s.data_version().ok()),
        store,
        polled: Instant::now(),
        shift: false,
        home: home::Home::default(),
        review: None,
        direct: matches!(start, Start::Revset(_)),
        buffers,
        screen: Screen::Home,
        editor_back: None,
        pr,
        prs: HashMap::new(),
        help: false,
        loads: 0,
        settings: settings::load(),
    };
    app.loader.log();
    app.loader.files();
    match start {
        Start::Home => {}
        Start::Revset(revset) => app.open(Target::Revset(revset)),
        Start::File(path, line) => {
            let cwd = std::env::current_dir().unwrap_or_default();
            let name = workspace_name(&app.root, &cwd.join(path));
            if let Err(e) = app.buffers.open(&name, line) {
                eprintln!("lgtm: couldn't open {name}: {e}");
                return 1;
            }
            app.screen = Screen::Editor;
        }
    }
    let mut term = Term::start();
    let result = app.run(&mut term, &rx);
    term.stop();
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            1
        }
    }
}

struct App {
    root: PathBuf,
    loader: Loader,
    _watcher: Option<notify::RecommendedWatcher>,
    /// Where agents ask this app about the review, over MCP.
    _server: Option<serve::Server>,
    /// Whether the watcher snapshots edits, while the review shows the working copy.
    snapshots: Arc<AtomicBool>,
    home: home::Home,
    review: Option<Review>,
    /// Whether the review was opened from the command line, so leaving it quits.
    direct: bool,
    buffers: edit::Editor,
    screen: Screen,
    /// The screen leaving the editor goes back to. `None` quits.
    editor_back: Option<Screen>,
    /// The PR given on the command line, whose threads `p` pulls.
    pr: Option<String>,
    /// The PR pulled for each review, by its name.
    prs: HashMap<String, (String, u32)>,
    help: bool,
    loads: u64,
    /// The repo's store, shared with the desktop app. `None` when it couldn't be opened.
    store: Option<Store>,
    /// The store's data version when last read, to notice the app's writes.
    version: Option<i64>,
    polled: Instant,
    /// Whether Shift is held, where the terminal reports it.
    shift: bool,
    /// The review's toggles, as last left.
    settings: Settings,
}

/// How often to look for another app's writes to the store.
const POLL: Duration = Duration::from_millis(250);

impl App {
    fn open(&mut self, target: Target) {
        self.loads += 1;
        self.loader.review(self.loads, target.clone());
        let mut review = Review::new(self.loads, target);
        review.apply(&self.settings);
        if let Some(pr) = self.prs.get(&review.revset) {
            review.set_pr(pr.clone());
        }
        self.review = Some(review);
        self.screen = Screen::Review;
    }

    /// Loads what's on screen again after a jj operation.
    fn reload(&mut self) {
        self.loader.log();
        self.loader.files();
        if let Some(r) = self.review.as_mut() {
            self.loads += 1;
            r.id = self.loads;
            self.loader.review(self.loads, r.target.clone());
        }
    }

    /// Goes to the editor from the review, at `at` or on the buffer it had.
    fn show_editor(&mut self, at: Option<(String, u32)>) {
        if let Some((path, line)) = at
            && let Err(e) = self.buffers.open(&path, line)
        {
            if let Some(r) = self.review.as_mut() {
                r.notice = Some(format!("couldn't open {path}: {e}"));
            }
            return;
        }
        self.screen = Screen::Editor;
        self.editor_back = Some(Screen::Review);
    }

    /// Goes to the review from the editor, on `path`'s `line`, opening the working copy's
    /// review when none is open.
    fn show_review(&mut self, path: &str, line: u32, text: &str) {
        match self.review.is_some() {
            true => self.reload(),
            false => self.open(Target::Revset("@".into())),
        }
        if let Some(r) = self.review.as_mut() {
            r.go_to_line(path, line, text);
        }
        self.screen = Screen::Review;
    }

    fn run(&mut self, term: &mut Term, rx: &Receiver<Msg>) -> anyhow::Result<()> {
        loop {
            term.terminal.draw(|f| {
                let area = f.area();
                self.draw(&mut Canvas::new(f.buffer_mut(), area));
            })?;
            if event::poll(Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) if term::is_shift(&key) => {
                        self.shift = key.kind != KeyEventKind::Release;
                    }
                    Event::Key(key) => {
                        if key.kind != KeyEventKind::Release && self.key(term::plain(key)) {
                            return Ok(());
                        }
                    }
                    Event::Mouse(ev) => {
                        let reviewing = self.screen == Screen::Review && !self.help;
                        if let (Some(r), true) = (self.review.as_mut(), reviewing) {
                            r.mouse(ev);
                        }
                    }
                    _ => {}
                }
            }
            while let Ok(msg) = rx.try_recv() {
                self.message(msg);
            }
            if self.polled.elapsed() >= POLL {
                self.polled = Instant::now();
                self.poll_store();
                self.buffers.check_disk();
            }
            for name in self.buffers.take_base_requests() {
                self.loader.base(name);
            }
        }
    }

    /// Rematches marks when another app wrote to the store.
    fn poll_store(&mut self) {
        let Some(store) = self.store.as_ref() else {
            return;
        };
        let version = store.data_version().ok();
        if version == self.version {
            return;
        }
        self.version = version;
        if let Some(r) = self.review.as_mut() {
            r.rematch(store);
        }
    }

    fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Log(result) => {
                self.home.picker.loaded(result);
                for rev in self.home.picker.versions_revs() {
                    self.loader.versions(rev);
                }
            }
            Msg::Versions { rev, result } => {
                let labeled = result.and_then(|list| {
                    let ids: Vec<String> = list.iter().map(|v| v.commit_id.clone()).collect();
                    let mut labels = match self.store.as_ref() {
                        Some(store) => store.snapshot_labels(&ids)?,
                        None => Default::default(),
                    };
                    Ok(list
                        .into_iter()
                        .map(|v| {
                            let label = labels.remove(&v.commit_id);
                            (v, label)
                        })
                        .collect())
                });
                self.home.picker.versions_loaded(&rev, labeled);
            }
            Msg::Diff { id, result } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.loaded(result);
                    if let Some(store) = self.store.as_ref() {
                        r.load_comments(store);
                    }
                    self.snapshots.store(r.working_copy(), Ordering::Relaxed);
                }
            }
            Msg::Files(result) => {
                // Without jj's list the tree keeps what it had.
                if let Ok(paths) = result {
                    self.home.files.set_paths(paths.clone());
                    self.buffers.set_files(paths);
                }
            }
            Msg::Base { name, text } => self.buffers.set_base(&name, text),
            Msg::Changed => {
                self.buffers.bases_stale();
                self.reload();
            }
            Msg::Agent { req, reply } => {
                let answer = match self.review.as_mut() {
                    Some(r) => r.answer(req, self.store.as_mut()),
                    None => Err("lgtm has no review open".into()),
                };
                let _ = reply.send(answer);
            }
            Msg::Highlight { id, file, sides } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.highlighted(file, sides);
                }
            }
            Msg::Pulled { id, result } => {
                if let (Some(r), Some(store)) = (
                    self.review.as_mut().filter(|r| r.id == id),
                    self.store.as_ref(),
                ) {
                    r.pulled(store, result);
                    if let Some(pr) = r.pr() {
                        self.prs.insert(r.revset.clone(), pr.clone());
                    }
                }
            }
            Msg::Built { id, files, cfg } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.built(files, cfg, self.store.as_ref());
                }
            }
        }
    }

    /// Handles a key. Returns whether to quit.
    fn key(&mut self, key: KeyEvent) -> bool {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            let unsaved = self.buffers.unsaved();
            if unsaved.is_empty() {
                return true;
            }
            self.buffers.notice = Some(format!("unsaved: {}", unsaved.join(", ")));
            self.screen = Screen::Editor;
            return false;
        }
        if self.help {
            self.help = false;
            return false;
        }
        if self.screen == Screen::Editor {
            if key.code == KeyCode::Char('?') && !self.buffers.typing() {
                self.help = true;
                return false;
            }
            return match self.buffers.key(key) {
                edit::Outcome::None => false,
                edit::Outcome::Review(path, line, text) => {
                    self.show_review(&path, line, &text);
                    false
                }
                edit::Outcome::Back => match self.editor_back {
                    Some(screen) => {
                        self.screen = screen;
                        false
                    }
                    None => true,
                },
            };
        }
        let typing = match self.review.as_ref() {
            Some(r) => r.typing(),
            None => self.home.typing(),
        };
        if key.code == KeyCode::Char('?') && !typing {
            self.help = true;
            return false;
        }
        if let Some(r) = self.review.as_mut() {
            let outcome = r.key(key, self.store.as_mut());
            let settings = r.settings();
            if settings != self.settings {
                settings::save(&settings);
                self.settings = settings;
            }
            match outcome {
                review::Outcome::Back => {
                    self.review = None;
                    self.screen = Screen::Home;
                    self.snapshots.store(false, Ordering::Relaxed);
                    return self.direct;
                }
                review::Outcome::Quit => return true,
                review::Outcome::Editor(None) if self.buffers.open.is_empty() => {
                    r.notice = Some("nothing open in the editor".into());
                }
                review::Outcome::Editor(at) => self.show_editor(at),
                review::Outcome::Pick(kind) => {
                    self.show_editor(None);
                    self.buffers.pick(kind);
                }
                review::Outcome::Pull => {
                    let pr = r.pr().map(|(_, n)| n.to_string()).or(self.pr.clone());
                    self.loader.pull(r.id, pr, r.bookmarks());
                }
                review::Outcome::None => {}
            }
            return false;
        }
        match self.home.key(key) {
            home::Outcome::Open(target) => self.open(target),
            home::Outcome::Versions(revs) => {
                for rev in revs {
                    self.loader.versions(rev);
                }
            }
            home::Outcome::Edit(path) => match self.buffers.open(&path, 0) {
                Ok(()) => {
                    self.screen = Screen::Editor;
                    self.editor_back = Some(Screen::Home);
                }
                Err(e) => self.buffers.notice = Some(format!("couldn't open {path}: {e}")),
            },
            home::Outcome::Quit => return true,
            home::Outcome::None => {}
        }
        false
    }

    fn draw(&mut self, c: &mut Canvas) {
        let area = c.area;
        if area.height < 3 {
            return;
        }
        for y in area.top()..area.bottom() {
            c.fill(area.x, y, area.width, BG);
        }
        let top = area.y;
        let bottom = area.bottom() - 1;
        let body = Rect::new(area.x, top + 1, area.width, area.height - 2);
        c.fill(area.x, top, area.width, PANEL);
        let x = c.put(area.x + 1, top, "lgtm", theme::bold(ACCENT).bg(PANEL)) + 2;
        if self.screen == Screen::Editor {
            self.draw_editor(c, x, body);
            return;
        }
        let unsaved = self
            .buffers
            .unsaved()
            .into_iter()
            .map(str::to_string)
            .collect();
        let (hints, keys) = match self.review.as_mut() {
            Some(r) => {
                r.unsaved = unsaved;
                r.title(c, x, top);
                r.draw(c, body);
                if let Some((text, offers)) = r.prompt() {
                    keybar::prompt(c, bottom, &text, offers);
                    keybar::menu(c, body, &r.command_menu());
                    return;
                }
                keybar::draw_badged(c, bottom, Some(("REV", CYAN)), &r.hints(self.shift));
                keybar::menu(c, body, &r.space_menu());
                if self.help {
                    keybar::help(c, body, review::KEYS);
                }
                return;
            }
            None => {
                c.put(x, top, &home(&self.root), theme::fg(FG).bg(PANEL));
                self.home.draw(c, body);
                (self.home.hints(), home::KEYS)
            }
        };
        keybar::draw(c, bottom, &hints);
        if self.help {
            keybar::help(c, body, keys);
        }
    }
}

impl App {
    fn draw_editor(&mut self, c: &mut Canvas, x: u16, body: Rect) {
        let (top, bottom) = (c.area.y, c.area.bottom() - 1);
        let e = &mut self.buffers;
        e.title(c, x, top);
        e.draw(c, body);
        if let Some(text) = e.prompt() {
            keybar::prompt(c, bottom, &text, &[]);
            keybar::menu(c, body, &e.command_menu());
            return;
        }
        keybar::draw_badged(c, bottom, Some(e.badge()), &e.hints());
        let pending = e.pending_menu();
        if !pending.is_empty() {
            keybar::menu(c, body, &pending);
        }
        if self.help {
            let keys: Vec<(&str, &str)> = crate::edit::keymap::BINDS
                .iter()
                .filter(|b| !matches!(b.keys, "left" | "right" | "up" | "down"))
                .map(|b| (b.keys, b.label))
                .collect();
            keybar::help(c, body, &keys);
        }
    }
}

/// `file`'s path in the workspace at `root`, or the whole path when it's outside it.
fn workspace_name(root: &Path, file: &Path) -> String {
    let file = normalize(file);
    let root = normalize(root);
    match file.strip_prefix(&root) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => file.to_string_lossy().into_owned(),
    }
}

/// `path` without `.` and `..` parts, and through symlinks where it exists.
fn normalize(path: &Path) -> PathBuf {
    if let Ok(real) = path.canonicalize() {
        return real;
    }
    // A new file: its folder through symlinks, then its name.
    match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) => normalize(dir).join(name),
        _ => path.to_path_buf(),
    }
}

/// `path` with the home directory as `~`.
fn home(path: &Path) -> String {
    let text = path.to_string_lossy().into_owned();
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && text.starts_with(&h) => format!("~{}", &text[h.len()..]),
        _ => text,
    }
}
