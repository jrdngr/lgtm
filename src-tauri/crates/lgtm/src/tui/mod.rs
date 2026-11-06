//! The terminal UI: a picker over the jj log, the review of what was picked, and the editor.

mod dash;
mod decor;
mod edit;
mod files;
mod github;
mod heat;
mod highlight;
mod home;
mod keybar;
mod load;
mod motions;
mod paint;
mod picker;
mod plain;
mod review;
mod serve;
mod settings;
mod term;
mod theme;
mod versions;
mod walk;
mod watch;
mod width;

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
use theme::{ACCENT, BG, CYAN, FG, PANEL, RED};

/// The frames of the spinner a running jj action shows.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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
    /// A file, as given relative to the current directory, at a line from 1. A folder opens
    /// the list of files.
    File(PathBuf, u32),
}

/// Which screen is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Home,
    Review,
    Editor,
    /// DASH mode: what the agent is doing.
    Dash,
}

pub use edit::SPLIT_FLAG;

/// Runs the UI in the jj workspace around the file or folder it opens, else around the
/// current directory, opening `start`. Outside jj only the editor runs. A `split` is a second
/// one beside another, which keeps the MCP socket.
pub fn run(start: Start, pr: Option<String>, split: bool) -> i32 {
    let cwd = std::env::current_dir().unwrap_or_default();
    let target = match &start {
        Start::File(path, line) => Some((normalize(&cwd.join(path)), *line)),
        _ => None,
    };
    let folder = target.as_ref().filter(|(t, _)| t.is_dir()).map(|(t, _)| t);
    let near = match &target {
        Some((t, _)) if !t.is_dir() => t.parent().map(Path::to_path_buf),
        Some((t, _)) => Some(t.clone()),
        None => Some(cwd.clone()),
    }
    .unwrap_or_else(|| cwd.clone());
    let file = target.as_ref().filter(|(t, _)| !t.is_dir());
    let Some(root) = crate::agent::workspace(&near) else {
        if matches!(start, Start::Revset(_)) {
            eprintln!("lgtm: not in a jj repo");
            return 1;
        }
        let root = near
            .ancestors()
            .find(|d| d.join(".git").exists())
            .map(Path::to_path_buf)
            .or(folder.cloned())
            .unwrap_or(cwd);
        let file = file.map(|(f, line)| (workspace_name(&root, f), *line));
        return plain::run(root, file);
    };
    let (tx, rx) = mpsc::channel();
    let socket = crate::agent::socket_path(&root).filter(|_| !split);
    let server = match socket.map(|p| serve::start(&p, tx.clone())) {
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
        refreshed: Instant::now(),
        busy: None,
        after_edit: None,
        shift: false,
        home: home::Home::default(),
        dash: dash::Dash::default(),
        workspace_key: None,
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
    app.buffers.files = app.settings.files;
    app.buffers.files_width = app.settings.editor_files_width;
    app.home.revisions_width = app.settings.home_revisions_width;
    app.home.files.heat_on = app.settings.heat;
    app.buffers.set_heat_on(app.settings.heat);
    app.loader.log();
    app.loader.files(&app.root);
    app.loader.decor();
    match start {
        Start::Home => {}
        Start::Revset(revset) => app.open(Target::Revset(revset)),
        Start::File(..) => {
            match file {
                Some((path, line)) => {
                    let name = workspace_name(&app.root, path);
                    if let Err(e) = app.buffers.open(&name, *line) {
                        eprintln!("lgtm: couldn't open {name}: {e}");
                        return 1;
                    }
                }
                None => app.buffers.pick(crate::edit::keymap::Pick::Files),
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
    dash: dash::Dash,
    /// The name the store files this workspace's turns under, once DASH mode asked.
    workspace_key: Option<String>,
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
    /// When the home screen last snapshotted the working copy.
    refreshed: Instant,
    /// The jj action running, and since when.
    busy: Option<(&'static str, Instant)>,
    /// The review to open once the running `jj edit` is done.
    after_edit: Option<Target>,
    /// Whether Shift is held, where the terminal reports it.
    shift: bool,
    /// The review's toggles, as last left.
    settings: Settings,
}

/// How often to look for another app's writes to the store.
const POLL: Duration = Duration::from_millis(250);
/// How often the home screen snapshots the working copy, so edits by others show up.
const REFRESH: Duration = Duration::from_secs(5);

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

    /// Shows DASH mode, snapshotting edits so the turn's diff keeps up.
    fn show_dash(&mut self) {
        if self.workspace_key.is_none() {
            self.workspace_key = self.loader.workspace_key(&self.root);
        }
        self.screen = Screen::Dash;
        self.snapshots.store(true, Ordering::Relaxed);
        self.refresh_dash();
    }

    /// Goes to the home screen from any other, closing the review.
    fn go_home(&mut self) {
        self.review = None;
        self.screen = Screen::Home;
        self.snapshots.store(false, Ordering::Relaxed);
    }

    /// Reads the newest turn again, and its diff when its edits changed.
    fn refresh_dash(&mut self) {
        let (Some(store), Some(key)) = (self.store.as_ref(), self.workspace_key.as_deref()) else {
            return;
        };
        let activity = store.agent_activity(key).ok().flatten();
        if let Some(start) = self.dash.set_activity(activity) {
            self.loader.turn_diff(start);
        }
    }

    /// Loads what's on screen again after a jj operation.
    fn reload(&mut self) {
        self.loader.log();
        self.loader.files(&self.root);
        self.loader.decor();
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
                    Event::Mouse(_) if self.help => {}
                    Event::Mouse(ev) => match self.screen {
                        Screen::Editor => self.buffers.mouse(ev),
                        Screen::Review => {
                            if let Some(r) = self.review.as_mut() {
                                r.mouse(ev, self.store.as_mut());
                            }
                        }
                        Screen::Home => {
                            let outcome = self.home.mouse(ev);
                            self.home_outcome(outcome);
                        }
                        Screen::Dash => self.dash_mouse(ev),
                    },
                    _ => {}
                }
            }
            while let Ok(msg) = rx.try_recv() {
                self.message(msg);
            }
            if self.screen == Screen::Home && self.refreshed.elapsed() >= REFRESH {
                self.refreshed = Instant::now();
                // A snapshot that changed something lands as an operation, which reloads.
                self.loader.decor();
            }
            if self.polled.elapsed() >= POLL {
                self.polled = Instant::now();
                self.poll_store();
                self.buffers.check_disk();
            }
            for name in self.buffers.take_base_requests() {
                self.loader.base(name);
            }
            if std::mem::take(&mut self.buffers.saved) {
                self.loader.decor();
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
        if self.screen == Screen::Dash {
            self.refresh_dash();
        }
        let Some(store) = self.store.as_ref() else {
            return;
        };
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
                    let mut labels: std::collections::HashMap<String, String> =
                        match self.store.as_ref() {
                            Some(store) => store
                                .turns_starting_at(&ids)?
                                .into_iter()
                                .map(|(c, t)| (c, lgtm_core::turns::turn_name(&t)))
                                .collect(),
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
            Msg::Files(listing) => {
                self.home.files.set_listing(listing.clone());
                self.buffers.set_files(listing);
            }
            Msg::Base { name, text } => self.buffers.set_base(&name, text),
            Msg::Decor(decor) => {
                self.home.files.decor = decor.clone();
                self.buffers.set_decor(decor);
            }
            Msg::Acted(result) => {
                self.busy = None;
                let then = self.after_edit.take();
                match result {
                    Ok(_) => {
                        self.loader.log();
                        if let Some(target) = then {
                            self.open(target);
                        }
                    }
                    Err(e) => self.home.notice = Some(format!("{e:#}")),
                }
            }
            Msg::Bookmarks(found) => {
                if let home::Outcome::Jj(action) = self.home.bookmarks(found) {
                    self.act(action);
                }
            }
            Msg::PushPlan(plan) => {
                self.busy = None;
                if let home::Outcome::Jj(action) = self.home.push_plan(plan) {
                    self.act(action);
                }
            }
            Msg::Written(paths) => {
                self.home.files.heat.written(&paths);
                self.buffers.written(&paths);
            }
            Msg::Changed => {
                self.buffers.bases_stale();
                self.reload();
                if self.screen == Screen::Dash
                    && let Some(start) = self.dash.start()
                {
                    self.loader.turn_diff(start.to_string());
                }
            }
            Msg::TurnDiff { start, result } => self.dash.diff_loaded(&start, result),
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
            let outcome = self.buffers.key(key);
            if self.buffers.files != self.settings.files
                || self.buffers.files_width != self.settings.editor_files_width
                || self.buffers.heat_on() != self.settings.heat
            {
                self.settings.files = self.buffers.files;
                self.settings.editor_files_width = self.buffers.files_width;
                self.settings.heat = self.buffers.heat_on();
                self.home.files.heat_on = self.settings.heat;
                settings::save(&self.settings);
                if let Some(r) = self.review.as_mut() {
                    r.apply(&self.settings);
                }
            }
            return match outcome {
                edit::Outcome::None => false,
                edit::Outcome::Review(path, line, text) => {
                    self.show_review(&path, line, &text);
                    false
                }
                edit::Outcome::Home => {
                    self.go_home();
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
        if self.screen == Screen::Dash {
            if key.code == KeyCode::Char('?') && !self.dash.typing() {
                self.help = true;
                return false;
            }
            let outcome = self.dash.key(key, &mut self.home.files);
            self.save_heat();
            return match outcome {
                dash::Outcome::None => false,
                dash::Outcome::Home => {
                    self.go_home();
                    false
                }
                dash::Outcome::Quit => true,
                dash::Outcome::Edit(path) => {
                    self.open_from_dash(path);
                    false
                }
                dash::Outcome::Review(path, line, text) => {
                    self.snapshots.store(false, Ordering::Relaxed);
                    self.show_review(&path, line, &text);
                    false
                }
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
            let settings = r.settings(&self.settings);
            if settings != self.settings {
                settings::save(&settings);
                self.buffers.files = settings.files;
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
        let outcome = self.home.key(key);
        if self.home.revisions_width != self.settings.home_revisions_width {
            self.settings.home_revisions_width = self.home.revisions_width;
            settings::save(&self.settings);
        }
        self.save_heat();
        self.home_outcome(outcome)
    }

    /// The wheel scrolls the tree or the diff under it, and a click in the tree opens a file
    /// or a folder.
    fn dash_mouse(&mut self, ev: event::MouseEvent) {
        use event::{MouseButton, MouseEventKind};
        let tree = &mut self.home.files;
        let over_tree = tree.contains(ev.column, ev.row);
        match ev.kind {
            MouseEventKind::ScrollDown if over_tree => tree.scroll(3),
            MouseEventKind::ScrollUp if over_tree => tree.scroll(-3),
            MouseEventKind::ScrollDown => self.dash.scroll(3),
            MouseEventKind::ScrollUp => self.dash.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) if over_tree => {
                if let files::Outcome::Open(path) = tree.click(ev.row) {
                    self.open_from_dash(path);
                }
            }
            _ => {}
        }
    }

    /// Opens `path` in the editor from DASH mode.
    fn open_from_dash(&mut self, path: String) {
        match self.buffers.open(&path, 0) {
            Ok(()) => {
                self.screen = Screen::Editor;
                self.editor_back = Some(Screen::Dash);
            }
            Err(e) => self.dash.notice = Some(format!("couldn't open {path}: {e}")),
        }
    }

    /// Keeps the file trees' heat toggle, after a key on a screen that changes it.
    fn save_heat(&mut self) {
        if self.home.files.heat_on != self.settings.heat {
            self.settings.heat = self.home.files.heat_on;
            self.buffers.set_heat_on(self.settings.heat);
            settings::save(&self.settings);
        }
    }

    /// Acts on what a key or click on the home screen asked for. Returns whether to quit.
    fn home_outcome(&mut self, outcome: home::Outcome) -> bool {
        match outcome {
            home::Outcome::Open(target) => self.open(target),
            home::Outcome::EditOpen { rev, target } => {
                self.after_edit = Some(target);
                self.act(lgtm_core::jj::JjAction::Edit { rev });
            }
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
            home::Outcome::Jj(action) => self.act(action),
            home::Outcome::Bookmarks(rev) => self.loader.bookmarks(rev),
            home::Outcome::PushPlan(revs) => {
                self.busy = Some(("checking the push", Instant::now()));
                self.loader.push_plan(revs);
            }
            home::Outcome::Dash => self.show_dash(),
            home::Outcome::Quit => return true,
            home::Outcome::None => {}
        }
        false
    }

    fn draw(&mut self, c: &mut Canvas) {
        if self.screen == Screen::Editor {
            draw_editor(c, &mut self.buffers, self.help);
            return;
        }
        let Some((x, body)) = frame(c) else {
            return;
        };
        let area = c.area;
        let top = area.y;
        if self.screen == Screen::Dash {
            self.dash.title(c, x, top);
            self.dash
                .draw(c, body, &mut self.home.files, &mut self.home.picker);
            let bottom = area.bottom() - 1;
            if let Some(n) = &self.dash.notice {
                c.put_right(top, 1, n, theme::fg(RED).bg(PANEL));
            }
            if let Some(text) = self.dash.prompt() {
                keybar::prompt(c, bottom, &text, &[]);
                keybar::menu(c, body, &self.dash.command_menu());
                return;
            }
            keybar::draw_badged(c, bottom, Some(("DASH", theme::TEAL)), &self.dash.hints());
            if self.help {
                keybar::help(c, body, dash::KEYS);
            }
            return;
        }
        let bottom = area.bottom() - 1;
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
                    keybar::help(c, body, &r.help_keys());
                }
                return;
            }
            None => {
                c.put(x, top, &home(&self.root), theme::fg(FG).bg(PANEL));
                self.home.busy = self.busy.map(|(doing, since)| {
                    let frame =
                        SPINNER[(since.elapsed().as_millis() / 100) as usize % SPINNER.len()];
                    format!("{frame} {doing}…")
                });
                if let Some(n) = &self.home.notice {
                    c.put_right(top, 1, n, theme::fg(RED).bg(PANEL));
                }
                self.home.draw(c, body);
                if let Some(text) = self.home.prompt() {
                    keybar::prompt(c, bottom, &text, &[]);
                    keybar::menu(c, self.home.menu_area(body), &self.home.command_menu());
                    return;
                }
                (self.home.hints(), home::KEYS)
            }
        };
        let badge = if self.review.is_none() {
            self.home.badge()
        } else {
            None
        };
        keybar::draw_badged(c, bottom, badge, &hints);
        if self.review.is_none() {
            keybar::menu(c, self.home.menu_area(body), &self.home.command_menu());
        }
        if self.help {
            keybar::help(c, body, keys);
        }
    }
}

impl App {
    /// Runs a jj action, showing it as running until it's done.
    fn act(&mut self, action: lgtm_core::jj::JjAction) {
        self.busy = Some((motions::doing(&action), Instant::now()));
        self.loader.act(action);
    }
}

/// Paints the background and the top bar with `lgtm` on it. Returns where the title goes on
/// and the body, when there's room.
fn frame(c: &mut Canvas) -> Option<(u16, Rect)> {
    let area = c.area;
    if area.height < 3 {
        return None;
    }
    for y in area.top()..area.bottom() {
        c.fill(area.x, y, area.width, BG);
    }
    let top = area.y;
    c.fill(area.x, top, area.width, PANEL);
    let x = c.put(area.x + 1, top, "lgtm", theme::bold(ACCENT).bg(PANEL)) + 2;
    Some((x, Rect::new(area.x, top + 1, area.width, area.height - 2)))
}

/// Draws the editor screen, with the keys over it when `help` is up.
fn draw_editor(c: &mut Canvas, e: &mut edit::Editor, help: bool) {
    let Some((x, body)) = frame(c) else {
        return;
    };
    let (top, bottom) = (c.area.y, c.area.bottom() - 1);
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
    if help {
        let keys: Vec<(&str, &str)> = crate::edit::keymap::BINDS
            .iter()
            .filter(|b| !matches!(b.keys, "left" | "right" | "up" | "down"))
            .map(|b| (b.keys, b.label))
            .collect();
        keybar::help(c, body, &keys);
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
