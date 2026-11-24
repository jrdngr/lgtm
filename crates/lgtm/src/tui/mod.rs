//! The terminal UI: the editor, the review with its revisions drawer, and the issues.

mod decor;
mod drawer;
mod edit;
mod files;
mod frame;
mod github;
mod heat;
mod help;
mod issues;
pub mod load;
mod mention;
mod modes;
mod motions;
mod picker;
mod plain;
mod plain_issues;
mod resolve;
pub mod review;
mod settings;
mod side;
mod term;
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

use lgtm_core::paths;
use lgtm_core::repo::Repo;
use lgtm_core::store::Store;
use lgtm_tui_kit::{flow, highlight, keybar, paint, theme};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::open_review;
use load::{Loader, Msg, Target};
use paint::Canvas;
use review::Review;
use settings::Settings;
use term::Term;
use theme::{CYAN, PANEL, RED};

/// The frames of the spinner a running jj action shows.
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// The spinner's frame for work that started at `since`.
pub fn spinner(since: Instant) -> &'static str {
    SPINNER[(since.elapsed().as_millis() / 100) as usize % SPINNER.len()]
}

/// The store of the repo the workspace at `root` belongs to.
pub fn store(root: &Path) -> Option<Store> {
    paths::data_dir()
        .zip(paths::find_root(root))
        .and_then(|(data, main)| Store::open(&paths::repo_dir(&data, &main).join("lgtm.db")).ok())
}

/// What the UI opens on.
pub enum Start {
    Editor,
    Revset(String),
    /// A file, as given relative to the current directory, at a line from 1. A folder opens
    /// the list of files.
    File(PathBuf, u32),
    /// Every file with conflicts in the working copy.
    Conflicts,
    /// The issues screen.
    Issues,
}

/// Which screen is up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Screen {
    Review,
    Editor,
    Issues,
}

pub use edit::SPLIT_FLAG;

/// Runs the UI in the jj workspace or git working tree around the file or folder it opens,
/// else around the current directory, opening `start`. Outside both only the editor runs.
pub fn run(start: Start, pr: Option<String>) -> i32 {
    crate::register::on_start();
    if let Some(home) = std::env::var_os("HOME") {
        lgtm_issues::skill::install(Path::new(&home));
        crate::review_cmd::skill::install(Path::new(&home));
    }
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
    let Some(root) = lgtm_core::repo::root_of(&near) else {
        if matches!(start, Start::Revset(_) | Start::Conflicts) {
            eprintln!("lgtm: not in a jj or git repo");
            return 1;
        }
        let root = folder.cloned().unwrap_or(cwd);
        let file = file.map(|(f, line)| (workspace_name(&root, f), *line));
        return plain::run(root, file, matches!(start, Start::Issues));
    };
    let (tx, rx) = mpsc::channel();
    let shown = open_review::folder(&root).and_then(|f| open_review::Mine::start(&f).ok());
    let loader = match Loader::new(Repo::open(&root), tx) {
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
        shown,
        snapshots,
        version: store.as_ref().and_then(|s| s.data_version().ok()),
        store,
        polled: Instant::now(),
        refreshed: Instant::now(),
        busy: None,
        making: false,
        shift: false,
        drawer: drawer::Drawer::default(),
        review: None,
        buffers,
        screen: Screen::Editor,
        editor_back: None,
        review_back: None,
        pull_pr: pr.is_some(),
        pr,
        prs: HashMap::new(),
        help: None,
        agent_pick: None,
        loads: 0,
        settings: settings::load(),
        issues: None,
        folders: None,
    };
    app.buffers.files = app.settings.files;
    app.buffers.files_width = app.settings.editor_files_width;
    app.buffers.wrap = app.settings.editor_wrap;
    app.buffers.render_markdown = app.settings.render_markdown;
    app.drawer.picker.git = app.loader.is_git();
    app.buffers.set_heat_on(app.settings.heat);
    app.loader.log();
    app.loader.files(&app.root);
    app.loader.decor();
    match start {
        Start::Editor => app.buffers.browse(),
        Start::Issues => app.show_issues(Some(&cwd)),
        Start::Conflicts => {
            let names = match app.loader.conflicted() {
                Ok(names) => names,
                Err(e) => {
                    eprintln!("lgtm: {e:#}");
                    return 1;
                }
            };
            let Some(first) = names.first() else {
                eprintln!("lgtm: no conflicts in the working copy");
                return 0;
            };
            for name in names.iter().chain([first]) {
                if let Err(e) = app.buffers.open(name, 1) {
                    eprintln!("lgtm: couldn't open {name}: {e}");
                    return 1;
                }
            }
            app.screen = Screen::Editor;
        }
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
                None => app.buffers.browse(),
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
    /// Where `lgtm agent review` learns which review this app shows.
    shown: Option<open_review::Mine>,
    /// Whether the watcher snapshots edits, while the review shows the working copy.
    snapshots: Arc<AtomicBool>,
    /// Agents to pick from for a selection sent with `a`.
    agent_pick: Option<mention::Pick>,
    drawer: drawer::Drawer,
    review: Option<Review>,
    buffers: edit::Editor,
    screen: Screen,
    /// The screen leaving the editor goes back to. `None` quits.
    editor_back: Option<Screen>,
    /// The screen the review was opened from, which Ctrl-o goes back to.
    review_back: Option<Screen>,
    /// The PR given on the command line, whose threads `p` pulls.
    pr: Option<String>,
    /// Whether to pull that PR's threads once the first review loads.
    pull_pr: bool,
    /// The PR pulled for each review, by its name.
    prs: HashMap<String, (String, u32)>,
    help: Option<help::Help>,
    loads: u64,
    /// The repo's store. `None` when it couldn't be opened.
    store: Option<Store>,
    /// The store's data version when last read, to notice the app's writes.
    version: Option<i64>,
    polled: Instant,
    /// When the editor last snapshotted the working copy.
    refreshed: Instant,
    /// The jj action running, and since when.
    busy: Option<(&'static str, Instant)>,
    /// The running action makes a new change, which the cursor goes to once it's made.
    making: bool,
    /// Whether Shift is held, where the terminal reports it.
    shift: bool,
    /// The review's toggles, as last left.
    settings: Settings,
    /// The issues screen, once it's been opened, or the folder picker before it.
    issues: Option<Box<lgtm_issues::tui::Issues>>,
    folders: Option<lgtm_issues::tui::picker::Picker>,
}

/// How often to look for another app's writes to the store.
const POLL: Duration = Duration::from_millis(250);
/// How often the editor snapshots the working copy, so edits by others show up.
pub(super) const REFRESH: Duration = Duration::from_secs(5);

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

    /// Goes back to the screen this one was reached from, for Ctrl-o.
    fn back(&mut self) {
        let to = match self.screen {
            Screen::Issues => None,
            Screen::Editor => self.editor_back,
            Screen::Review => self.review_back,
        };
        match to {
            Some(Screen::Review) if self.review.is_some() => self.screen = Screen::Review,
            Some(Screen::Editor) => self.screen = Screen::Editor,
            Some(Screen::Issues) => self.show_issues(None),
            _ => {}
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
        if let Some(r) = self.review.as_mut().filter(|_| !path.is_empty()) {
            r.go_to_line(path, line, text);
        }
        self.screen = Screen::Review;
        self.review_back = Some(Screen::Editor);
    }

    fn run(&mut self, term: &mut Term, rx: &Receiver<Msg>) -> anyhow::Result<()> {
        loop {
            term.terminal.draw(|f| {
                let area = f.area();
                let c = &mut Canvas::new(f.buffer_mut(), area);
                self.draw(c);
                if let Some(at) = c.bar {
                    f.set_cursor_position(at);
                }
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
                    Event::Paste(text) if self.screen == Screen::Editor && self.help.is_none() => {
                        self.buffers.paste(&text);
                    }
                    Event::Paste(text) if self.screen == Screen::Issues => {
                        self.issues_paste(&text);
                    }
                    Event::Mouse(_) if self.help.is_some() => {}
                    Event::Mouse(ev) => match self.screen {
                        Screen::Editor => self.buffers.mouse(ev),
                        Screen::Review if self.drawer.contains(ev.column, ev.row) => {
                            let outcome = self.drawer.mouse(ev);
                            self.drawer_outcome(outcome);
                        }
                        Screen::Review if self.drawer.is_open() => {}
                        Screen::Review => {
                            if let Some(r) = self.review.as_mut() {
                                r.mouse(ev, self.store.as_mut());
                            }
                        }
                        Screen::Issues => self.issues_mouse(ev),
                    },
                    _ => {}
                }
            }
            while let Ok(msg) = rx.try_recv() {
                self.message(msg);
            }
            self.issues_tick();
            // Agents change files while the editor is open too, so both trees keep up.
            if self.screen == Screen::Editor && self.refreshed.elapsed() >= REFRESH {
                self.refreshed = Instant::now();
                // A snapshot that changed something lands as an operation, which reloads.
                self.loader.decor();
            }
            if self.polled.elapsed() >= POLL {
                self.polled = Instant::now();
                self.poll_store();
                self.record_shown();
                self.buffers.check_disk();
            }
            for name in self.buffers.take_base_requests() {
                self.loader.base(name);
            }
            for (name, text) in self.buffers.take_resolve_requests() {
                self.loader.resolve(name, text);
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
        let Some(store) = self.store.as_ref() else {
            return;
        };
        if let Some(r) = self.review.as_mut() {
            r.rematch(store);
        }
    }

    /// Records which review this app shows, for `lgtm agent review`.
    fn record_shown(&mut self) {
        let Some(mine) = self.shown.as_mut() else {
            return;
        };
        mine.show(self.review.as_ref().map(|r| open_review::Shown {
            revset: r.revset.clone(),
            between: match &r.target {
                Target::Revset(_) => None,
                Target::Between { from, to, .. } => Some((from.clone(), to.clone())),
            },
            pr: r.pr().cloned(),
        }));
    }

    fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Log(result) => {
                self.drawer.picker.loaded(result);
                for rev in self.drawer.picker.versions_revs() {
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
                self.drawer.picker.versions_loaded(&rev, labeled);
            }
            Msg::Diff { id, result } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.loaded(result);
                    if std::mem::take(&mut self.pull_pr) {
                        r.pulling = Some(Instant::now());
                        self.loader.pull(r.id, self.pr.clone(), r.bookmarks());
                    }
                    if let Some(store) = self.store.as_ref() {
                        r.load_comments(store);
                    }
                    self.snapshots.store(r.working_copy(), Ordering::Relaxed);
                }
            }
            Msg::Containing { id, found } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.containing = found;
                }
            }
            Msg::Files(listing) => self.buffers.set_files(listing),
            Msg::Base { name, text } => self.buffers.set_base(&name, text),
            Msg::Resolve {
                name,
                text,
                session,
            } => self.buffers.set_resolve(&name, &text, &session),
            Msg::Decor(decor) => self.buffers.set_decor(decor),
            Msg::Acted(result) => {
                self.busy = None;
                let made = std::mem::take(&mut self.making);
                match result {
                    Ok(_) => {
                        self.drawer.picker.to_working_copy = made;
                        self.loader.log();
                    }
                    Err(e) => self.drawer.notice = Some(format!("{e:#}")),
                }
            }
            Msg::Bookmarks(found) => {
                let outcome = self.drawer.bookmarks(found);
                self.drawer_outcome(outcome);
            }
            Msg::PushPlan(plan) => {
                self.busy = None;
                let outcome = self.drawer.push_plan(plan);
                self.drawer_outcome(outcome);
            }
            Msg::Written(paths) => self.buffers.written(&paths),
            Msg::Changed => {
                self.buffers.bases_stale();
                self.reload();
            }
            Msg::Highlight { id, file, sides } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.highlighted(file, sides);
                }
            }
            Msg::Pulled { id, result } => {
                let Some(r) = self.review.as_mut().filter(|r| r.id == id) else {
                    return;
                };
                r.pulling = None;
                if let Some(store) = self.store.as_ref() {
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
        if key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::SUPER)
            && let Some(text) = self.selection()
        {
            match lgtm_tui_kit::clipboard::copy(&text) {
                Ok(()) => self.notice("copied".into()),
                Err(e) => self.notice(format!("couldn't copy: {e}")),
            }
            return false;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if !self.buffers.blocks_quit() {
                return true;
            }
            self.screen = Screen::Editor;
            return false;
        }
        if let Some(h) = self.help.as_mut() {
            if !h.key(&key) {
                self.help = None;
            }
            return false;
        }
        if let Some(pick) = self.agent_pick.take() {
            if let Some(n) = pick.key(key) {
                self.notice(n);
            }
            return false;
        }
        if self.screen == Screen::Issues {
            return self.issues_key(key);
        }
        if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.back();
            return false;
        }
        if self.screen == Screen::Editor {
            if key.code == KeyCode::Char('?') && !self.buffers.typing() {
                self.help = Some(help::Help::default());
                return false;
            }
            let outcome = self.buffers.key(key);
            if self.buffers.files != self.settings.files
                || self.buffers.files_width != self.settings.editor_files_width
                || self.buffers.heat_on() != self.settings.heat
                || self.buffers.wrap != self.settings.editor_wrap
                || self.buffers.render_markdown != self.settings.render_markdown
            {
                self.settings.files = self.buffers.files;
                self.settings.editor_wrap = self.buffers.wrap;
                self.settings.render_markdown = self.buffers.render_markdown;
                self.settings.editor_files_width = self.buffers.files_width;
                self.settings.heat = self.buffers.heat_on();
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
                edit::Outcome::Mention(text) => {
                    self.mention(text);
                    false
                }
                edit::Outcome::Issues => {
                    self.show_issues(None);
                    false
                }
                edit::Outcome::Quit => true,
            };
        }
        let typing = match self.review.as_ref() {
            _ if self.drawer.is_open() => self.drawer.typing(),
            Some(r) => r.typing(),
            None => false,
        };
        if key.code == KeyCode::Char('?') && !typing {
            self.help = Some(help::Help::default());
            return false;
        }
        if self.drawer.is_open() {
            let outcome = self.drawer.key(key);
            self.drawer_outcome(outcome);
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
                review::Outcome::Quit => return true,
                review::Outcome::Editor(at) => self.show_editor(at),
                review::Outcome::Pick(kind) => {
                    self.show_editor(None);
                    self.buffers.pick(kind);
                }
                review::Outcome::Pull => {
                    let pr = r.pr().map(|(_, n)| n.to_string()).or(self.pr.clone());
                    r.pulling = Some(Instant::now());
                    self.loader.pull(r.id, pr, r.bookmarks());
                }
                review::Outcome::Mention(text) => self.mention(text),
                review::Outcome::Issues => self.show_issues(None),
                review::Outcome::Revisions => {
                    let newest = r.newest().map(str::to_string);
                    self.drawer.show(newest.as_deref());
                }
                review::Outcome::None => {}
            }
        }
        false
    }

    /// Acts on what a key or click in the revisions drawer asked for.
    fn drawer_outcome(&mut self, outcome: drawer::Outcome) {
        match outcome {
            drawer::Outcome::Open(target) => self.open(target),
            drawer::Outcome::Versions(revs) => {
                for rev in revs {
                    self.loader.versions(rev);
                }
            }
            drawer::Outcome::Jj(action) => self.act(action),
            drawer::Outcome::Bookmarks(rev) => self.loader.bookmarks(rev),
            drawer::Outcome::PushPlan(revs) => {
                self.busy = Some(("checking the push", Instant::now()));
                self.loader.push_plan(revs);
            }
            drawer::Outcome::Mode(to) => self.switch(to),
            drawer::Outcome::None => {}
        }
    }

    fn draw(&mut self, c: &mut Canvas) {
        if self.screen == Screen::Issues {
            self.draw_issues(c);
            return;
        }
        if self.screen == Screen::Editor {
            frame::draw_editor(
                c,
                &mut self.buffers,
                self.help.as_mut(),
                self.agent_pick.as_ref(),
            );
            return;
        }
        let unsaved = self
            .buffers
            .unsaved()
            .into_iter()
            .map(str::to_string)
            .collect();
        let Some(r) = self.review.as_mut() else {
            return;
        };
        r.unsaved = unsaved;
        if self.drawer.is_open() {
            self.drawer.busy = self
                .busy
                .map(|(doing, since)| format!("{} {doing}…", spinner(since)));
            draw_with_drawer(c, r, &mut self.drawer, self.help.as_mut());
            return;
        }
        let width = c.area.width;
        let bottom = c.area.bottom().saturating_sub(1);
        let (left, right) = r.title();
        let prompt = r.prompt().map(|(t, o)| (t, o.to_vec()));
        let hints = r.hints(self.shift);
        let badge = Some(("REV", CYAN));
        let foot = match prompt {
            Some(_) => 1,
            None => keybar::rows(width, badge, &hints),
        };
        let Some(body) = frame::frame(c, left, &right, foot) else {
            return;
        };
        r.draw(c, body);
        if let Some((text, offers)) = prompt {
            keybar::prompt(c, bottom, &text, &offers);
            keybar::menu(c, body, &r.command_menu());
            return;
        }
        keybar::draw_badged(c, bottom, badge, &hints);
        keybar::menu(c, body, &r.space_menu());
        if let Some(pick) = &self.agent_pick {
            keybar::menu(c, body, &pick.menu());
        }
        if let Some(h) = self.help.as_mut() {
            h.draw(c, body, &r.help_groups());
        }
    }
}

/// The review with the revisions drawer open in its file list's place, and the drawer's keys.
fn draw_with_drawer(
    c: &mut Canvas,
    r: &mut Review,
    d: &mut drawer::Drawer,
    help: Option<&mut help::Help>,
) {
    let bottom = c.area.bottom().saturating_sub(1);
    let (left, mut right) = r.title();
    if let Some(n) = &d.notice {
        right = vec![flow::Piece::new(2, n.clone(), theme::fg(RED).bg(PANEL))];
    }
    let prompt = d.prompt();
    let hints = d.hints();
    let badge = Some(("REV", CYAN));
    let foot = match prompt {
        Some(_) => 1,
        None => keybar::rows(c.area.width, badge, &hints),
    };
    let Some(body) = frame::frame(c, left, &right, foot) else {
        return;
    };
    r.draw_diff_only(c, drawer::Drawer::beside(body));
    d.draw(c, body);
    match prompt {
        Some(text) => keybar::prompt(c, bottom, &text, &[]),
        None => {
            keybar::draw_badged(c, bottom, badge, &hints);
        }
    }
    keybar::menu(c, body, &d.leader_menu());
    if let Some(h) = help {
        h.draw(c, body, &drawer::help_groups(d.picker.git));
    }
}

impl App {
    /// Sends `text` to an agent, or asks which.
    fn mention(&mut self, text: String) {
        match mention::send(text) {
            mention::Sent::Done => {}
            mention::Sent::Pick(pick) => self.agent_pick = Some(pick),
            mention::Sent::Notice(n) => self.notice(n),
        }
    }

    /// Shows `n` on the screen that's up.
    /// The text selected on the screen in view, for ⌘C to copy.
    fn selection(&self) -> Option<String> {
        if self.help.is_some() {
            return None;
        }
        match self.screen {
            Screen::Editor => self.buffers.selection(),
            Screen::Review => self.review.as_ref()?.selected(),
            Screen::Issues => self.issues.as_ref()?.selection(),
        }
    }

    fn notice(&mut self, n: String) {
        match self.review.as_mut() {
            Some(r) if self.screen != Screen::Editor => r.notice = Some(n),
            _ => self.buffers.notice = Some(n),
        }
    }

    /// Runs a jj action, showing it as running until it's done.
    fn act(&mut self, action: lgtm_core::jj::JjAction) {
        self.busy = Some((motions::doing(&action), Instant::now()));
        self.making = matches!(
            action,
            lgtm_core::jj::JjAction::NewAfter { .. } | lgtm_core::jj::JjAction::NewBefore { .. }
        );
        self.loader.act(action);
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
