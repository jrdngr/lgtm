//! The review UI: a picker over the jj log, and the review of what was picked.

mod editor;
mod highlight;
mod keybar;
mod load;
mod paint;
mod picker;
mod review;
mod term;
mod theme;
mod watch;

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

use load::{Loader, Msg};
use paint::Canvas;
use picker::Picker;
use review::Review;
use term::Term;
use theme::{ACCENT, BG, FG, PANEL};

/// The jj workspace `cwd` is in: the nearest ancestor holding a `.jj` directory.
fn workspace(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .find(|p| p.join(".jj").is_dir())
        .map(Path::to_path_buf)
}

/// Runs the UI in the jj workspace around the current directory, opening `revset` when given.
pub fn run(revset: Option<String>) -> i32 {
    let Some(root) = std::env::current_dir().ok().and_then(|d| workspace(&d)) else {
        eprintln!("lgtm: not in a jj repo");
        return 1;
    };
    let (tx, rx) = mpsc::channel();
    let loader = match Loader::new(Jj::new(&root), tx) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            return 1;
        }
    };
    let store = paths::data_dir()
        .zip(paths::find_root(&root))
        .and_then(|(data, main)| Store::open(&paths::repo_dir(&data, &main).join("lgtm.db")).ok());
    let snapshots = Arc::new(AtomicBool::new(false));
    let watcher = loader.watch(&root, snapshots.clone()).ok();
    let mut app = App {
        root,
        loader,
        _watcher: watcher,
        snapshots,
        edit: None,
        version: store.as_ref().and_then(|s| s.data_version().ok()),
        store,
        polled: Instant::now(),
        shift: false,
        picker: Picker::default(),
        review: None,
        direct: revset.is_some(),
        help: false,
        loads: 0,
    };
    app.loader.log();
    if let Some(revset) = revset {
        app.open(revset);
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
    /// Whether the watcher snapshots edits, while the review shows the working copy.
    snapshots: Arc<AtomicBool>,
    /// The file and line to open the editor at, once the frame is drawn.
    edit: Option<(String, u32)>,
    picker: Picker,
    review: Option<Review>,
    /// Whether the review was opened from the command line, so leaving it quits.
    direct: bool,
    help: bool,
    loads: u64,
    /// The repo's store, shared with the desktop app. `None` when it couldn't be opened.
    store: Option<Store>,
    /// The store's data version when last read, to notice the app's writes.
    version: Option<i64>,
    polled: Instant,
    /// Whether Shift is held, where the terminal reports it.
    shift: bool,
}

/// How often to look for another app's writes to the store.
const POLL: Duration = Duration::from_millis(250);

impl App {
    fn open(&mut self, revset: String) {
        self.loads += 1;
        self.loader.review(self.loads, revset.clone());
        self.review = Some(Review::new(self.loads, revset));
    }

    /// Loads what's on screen again after a jj operation.
    fn reload(&mut self) {
        self.loader.log();
        if let Some(r) = self.review.as_mut() {
            self.loads += 1;
            r.id = self.loads;
            self.loader.review(self.loads, r.revset.clone());
        }
    }

    /// Opens the editor at `path`, a path in the workspace, and `line`.
    fn open_editor(&mut self, term: &mut Term, path: &str, line: u32) {
        let failed = match editor::open(&self.root.join(path), line) {
            editor::Open::Detached(argv) => editor::spawn(&argv).err(),
            editor::Open::Here(argv) => term
                .suspended(|| {
                    std::process::Command::new(&argv[0])
                        .args(&argv[1..])
                        .status()
                })
                .err(),
        };
        if let (Some(e), Some(r)) = (failed, self.review.as_mut()) {
            r.notice = Some(format!("couldn't open the editor: {e}"));
        }
    }

    fn run(&mut self, term: &mut Term, rx: &Receiver<Msg>) -> anyhow::Result<()> {
        loop {
            if let Some((path, line)) = self.edit.take() {
                self.open_editor(term, &path, line);
            }
            term.terminal.draw(|f| {
                let area = f.area();
                self.draw(&mut Canvas::new(f.buffer_mut(), area));
            })?;
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
            {
                if term::is_shift(&key) {
                    self.shift = key.kind != KeyEventKind::Release;
                } else if key.kind != KeyEventKind::Release && self.key(term::plain(key)) {
                    return Ok(());
                }
            }
            while let Ok(msg) = rx.try_recv() {
                self.message(msg);
            }
            if self.polled.elapsed() >= POLL {
                self.polled = Instant::now();
                self.poll_store();
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
            Msg::Log(result) => self.picker.loaded(result),
            Msg::Diff { id, result } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.loaded(result);
                    self.snapshots.store(r.working_copy(), Ordering::Relaxed);
                }
            }
            Msg::Changed => self.reload(),
            Msg::Highlight { id, file, sides } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.highlighted(file, sides);
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
            return true;
        }
        if self.help {
            self.help = false;
            return false;
        }
        if key.code == KeyCode::Char('?') {
            self.help = true;
            return false;
        }
        if let Some(r) = self.review.as_mut() {
            match r.key(key, self.store.as_mut()) {
                review::Outcome::Back => {
                    self.review = None;
                    self.snapshots.store(false, Ordering::Relaxed);
                    return self.direct;
                }
                review::Outcome::Edit(path, line) => self.edit = Some((path, line)),
                review::Outcome::None => {}
            }
            return false;
        }
        match self.picker.key(key) {
            picker::Outcome::Open(revset) => self.open(revset),
            picker::Outcome::Quit => return true,
            picker::Outcome::None => {}
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
        let (hints, keys) = match self.review.as_mut() {
            Some(r) => {
                r.title(c, x, top);
                r.draw(c, body);
                (r.hints(self.shift), review::KEYS)
            }
            None => {
                c.put(x, top, &home(&self.root), theme::fg(FG).bg(PANEL));
                self.picker.draw(c, body);
                (self.picker.hints(), picker::KEYS)
            }
        };
        keybar::draw(c, bottom, &hints);
        if self.help {
            keybar::help(c, body, keys);
        }
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
