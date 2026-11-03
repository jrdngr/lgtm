//! The review UI: a picker over the jj log, and the review of what was picked.

mod highlight;
mod keybar;
mod load;
mod paint;
mod picker;
mod review;
mod theme;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use lgtm_core::jj::Jj;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

use load::{Loader, Msg};
use paint::Canvas;
use picker::Picker;
use review::Review;
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
    let mut app = App {
        root,
        loader,
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
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal, &rx);
    ratatui::restore();
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
    picker: Picker,
    review: Option<Review>,
    /// Whether the review was opened from the command line, so leaving it quits.
    direct: bool,
    help: bool,
    loads: u64,
}

impl App {
    fn open(&mut self, revset: String) {
        self.loads += 1;
        self.loader.review(self.loads, revset.clone());
        self.review = Some(Review::new(self.loads, revset));
    }

    fn run(&mut self, terminal: &mut DefaultTerminal, rx: &Receiver<Msg>) -> anyhow::Result<()> {
        loop {
            terminal.draw(|f| {
                let area = f.area();
                self.draw(&mut Canvas::new(f.buffer_mut(), area));
            })?;
            if event::poll(Duration::from_millis(50))?
                && let Event::Key(key) = event::read()?
                && key.kind != KeyEventKind::Release
                && self.key(key)
            {
                return Ok(());
            }
            while let Ok(msg) = rx.try_recv() {
                self.message(msg);
            }
        }
    }

    fn message(&mut self, msg: Msg) {
        match msg {
            Msg::Log(result) => self.picker.loaded(result),
            Msg::Diff { id, result } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.loaded(result);
                }
            }
            Msg::Highlight { id, file, sides } => {
                if let Some(r) = self.review.as_mut().filter(|r| r.id == id) {
                    r.highlighted(file, sides);
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
            if let review::Outcome::Back = r.key(key) {
                self.review = None;
                return self.direct;
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
                (r.hints(), review::KEYS)
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
