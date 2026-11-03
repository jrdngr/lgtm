//! The review UI: a picker over the jj log, and the review of what was picked.

mod editor;
mod github;
mod highlight;
mod keybar;
mod load;
mod paint;
mod picker;
mod review;
mod serve;
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
use picker::Picker;
use review::Review;
use term::Term;
use theme::{ACCENT, BG, FG, PANEL};

/// The store of the repo the workspace at `root` belongs to, shared with the desktop app.
pub fn store(root: &Path) -> Option<Store> {
    paths::data_dir()
        .zip(paths::find_root(root))
        .and_then(|(data, main)| Store::open(&paths::repo_dir(&data, &main).join("lgtm.db")).ok())
}

/// Runs the UI in the jj workspace around the current directory, opening `revset` when given.
pub fn run(revset: Option<String>, pr: Option<String>) -> i32 {
    let Some(root) = std::env::current_dir()
        .ok()
        .and_then(|d| crate::agent::workspace(&d))
    else {
        eprintln!("lgtm: not in a jj repo");
        return 1;
    };
    let config = match crate::config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lgtm: {e:#}");
            return 1;
        }
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
    let snapshots = Arc::new(AtomicBool::new(false));
    let watcher = loader.watch(&root, snapshots.clone()).ok();
    let mut app = App {
        root,
        editor: config.editor,
        loader,
        _watcher: watcher,
        _server: server,
        snapshots,
        edit: None,
        version: store.as_ref().and_then(|s| s.data_version().ok()),
        store,
        polled: Instant::now(),
        shift: false,
        picker: Picker::default(),
        review: None,
        direct: revset.is_some(),
        pr,
        prs: HashMap::new(),
        help: false,
        loads: 0,
    };
    app.loader.log();
    if let Some(revset) = revset {
        app.open(Target::Revset(revset));
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
    /// The command template that opens the editor.
    editor: String,
    loader: Loader,
    _watcher: Option<notify::RecommendedWatcher>,
    /// Where agents ask this app about the review, over MCP.
    _server: Option<serve::Server>,
    /// Whether the watcher snapshots edits, while the review shows the working copy.
    snapshots: Arc<AtomicBool>,
    /// The file and line to open the editor at, once the frame is drawn.
    edit: Option<(String, u32)>,
    picker: Picker,
    review: Option<Review>,
    /// Whether the review was opened from the command line, so leaving it quits.
    direct: bool,
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
}

/// How often to look for another app's writes to the store.
const POLL: Duration = Duration::from_millis(250);

impl App {
    fn open(&mut self, target: Target) {
        self.loads += 1;
        self.loader.review(self.loads, target.clone());
        let mut review = Review::new(self.loads, target);
        if let Some(pr) = self.prs.get(&review.revset) {
            review.set_pr(pr.clone());
        }
        self.review = Some(review);
    }

    /// Loads what's on screen again after a jj operation.
    fn reload(&mut self) {
        self.loader.log();
        if let Some(r) = self.review.as_mut() {
            self.loads += 1;
            r.id = self.loads;
            self.loader.review(self.loads, r.target.clone());
        }
    }

    /// Opens the editor at `path`, a path in the workspace, and `line`.
    fn open_editor(&mut self, term: &mut Term, path: &str, line: u32) {
        let failed = match editor::open(&self.editor, &self.root.join(path), line) {
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
                        if let (Some(r), false) = (self.review.as_mut(), self.help) {
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
                self.picker.loaded(result);
                if let Some(rev) = self.picker.versions_rev() {
                    self.loader.versions(rev.to_string());
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
                self.picker.versions_loaded(&rev, labeled);
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
            Msg::Changed => self.reload(),
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
            return true;
        }
        if self.help {
            self.help = false;
            return false;
        }
        let typing = self.review.as_ref().is_some_and(Review::typing);
        if key.code == KeyCode::Char('?') && !typing {
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
                review::Outcome::Pull => {
                    let pr = r.pr().map(|(_, n)| n.to_string()).or(self.pr.clone());
                    self.loader.pull(r.id, pr, r.bookmarks());
                }
                review::Outcome::None => {}
            }
            return false;
        }
        match self.picker.key(key) {
            picker::Outcome::Open(target) => self.open(target),
            picker::Outcome::Versions(rev) => self.loader.versions(rev),
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
                if let Some((text, offers)) = r.prompt() {
                    keybar::prompt(c, bottom, &text, offers);
                    keybar::menu(c, body, &r.command_menu());
                    return;
                }
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
