//! The issues screen: waiting questions, the issue list, answering, and opened issues.

mod input;
mod list;
mod marks;
mod mouse;
pub mod picker;
pub(crate) mod qq;
mod question;
mod undo;
mod view;
mod wrap;

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use lgtm_tui_kit::flow::{self, Piece};
use lgtm_tui_kit::keybar::{self, Hint};
use lgtm_tui_kit::markdown::Outcome as ViewOutcome;
use lgtm_tui_kit::mode::{self, Mode};
use lgtm_tui_kit::paint::Canvas;
use lgtm_tui_kit::theme::{self, ACCENT, AMBER, BG, FG, GREEN, PANEL, RED};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::folder::{Folder, Stamp};
use crate::issue::Issue;
use crate::workflow::Workflow;
use crate::{lgterm, ops};
use input::Input;
use list::Row;
use question::Draft;
use view::View;

/// How often the folder is checked for changes.
const POLL: Duration = Duration::from_millis(500);

enum Screen {
    List,
    Question { id: u32, qid: String, top: usize },
    View(Box<View>),
}

#[derive(Clone, Copy, PartialEq)]
enum Ask {
    New,
    Filter,
    Command,
}

/// What a key asks of whatever hosts the screen.
#[derive(Debug, PartialEq)]
pub enum Outcome {
    None,
    /// Open this file in the editor.
    Edit(PathBuf),
    /// Pick another issues folder.
    Folders,
    /// Switch to another screen.
    Mode(Mode),
    Home,
    Quit,
}

pub struct Issues {
    folder: Folder,
    who: String,
    issues: Vec<Issue>,
    wf: Workflow,
    bad: Vec<String>,
    stamp: Stamp,
    screen: Screen,
    sel: usize,
    top: usize,
    filter: String,
    prompt: Option<(Ask, Input)>,
    notice: Option<String>,
    drafts: HashMap<(u32, String), Draft>,
    /// What the last key asked of the host.
    outcome: Outcome,
    /// Whether Space was pressed, for the space menu.
    space: bool,
    /// Whether `space m` waits for a mode's key.
    mode_menu: bool,
    /// The screens `space m` can switch to.
    modes: Vec<Mode>,
    /// The issue whose status `s` is about to set, while its menu waits for a key.
    status_for: Option<u32>,
    history: undo::History,
    polled: Instant,
    /// Rows the body had at the last draw.
    height: usize,
    /// Where the list was drawn last, for clicks.
    list_area: Rect,
    /// The last click's row and when, to tell a double click.
    click: Option<(usize, Instant)>,
    /// Folding statuses whose issues are listed.
    expanded: Vec<String>,
}

/// ⌥1 to ⌥9: as Alt and a digit, or as the characters a US Mac keyboard types for them.
fn alt_digit(key: &KeyEvent) -> Option<usize> {
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    if key.modifiers.contains(KeyModifiers::ALT) && ('1'..='9').contains(&c) {
        return c.to_digit(10).map(|d| d as usize);
    }
    "¡™£¢∞§¶•ª".chars().position(|m| m == c).map(|i| i + 1)
}

impl Issues {
    pub fn new(folder: Folder, who: String) -> Issues {
        let mut issues = Issues {
            who,
            folder,
            issues: Vec::new(),
            wf: Workflow::default(),
            bad: Vec::new(),
            stamp: Vec::new(),
            screen: Screen::List,
            sel: 0,
            top: 0,
            filter: String::new(),
            prompt: None,
            notice: None,
            drafts: HashMap::new(),
            outcome: Outcome::None,
            space: false,
            mode_menu: false,
            modes: vec![Mode::Editor, Mode::Review, Mode::Home],
            status_for: None,
            history: undo::History::default(),
            polled: Instant::now(),
            height: 0,
            list_area: Rect::default(),
            click: None,
            expanded: Vec::new(),
        };
        issues.reload();
        issues
    }

    pub fn root(&self) -> &std::path::Path {
        &self.folder.root
    }

    /// Picks up changes to the folder, every so often.
    pub fn tick(&mut self) {
        if self.polled.elapsed() >= POLL {
            self.polled = Instant::now();
            if self.folder.stamp() != self.stamp {
                self.reload();
            }
        }
    }

    /// Shows `n` over the key bar until the next key.
    pub fn notice(&mut self, n: String) {
        self.notice = Some(n);
    }

    /// Whether keys are going into a line being typed.
    pub fn typing(&self) -> bool {
        self.prompt.is_some() || matches!(self.screen, Screen::Question { .. })
    }

    pub fn reload(&mut self) {
        self.stamp = self.folder.stamp();
        match self.folder.load() {
            Ok((issues, bad)) => {
                self.issues = issues;
                self.bad = bad;
            }
            Err(e) => self.notice = Some(format!("couldn't read the folder: {e:#}")),
        }
        match self.folder.workflow() {
            Ok(wf) => self.wf = wf,
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
        if let Screen::View(v) = &mut self.screen
            && let Some(i) = self.issues.iter().find(|i| i.id == v.id)
        {
            v.refresh(i);
        }
        self.clamp();
    }

    fn issue(&self, id: u32) -> Option<&Issue> {
        self.issues.iter().find(|i| i.id == id)
    }

    fn rows(&self) -> Vec<Row> {
        list::rows(&self.issues, &self.wf, &self.filter, &self.expanded)
    }

    fn clamp(&mut self) {
        let rows = self.rows();
        if rows.get(self.sel).is_none_or(|r| !r.pickable()) {
            self.sel = rows.iter().position(Row::pickable).unwrap_or(0);
        }
    }

    /// The waiting question after `after` in list order, wrapping, or the first.
    fn next_question(&self, after: Option<(u32, &str)>) -> Option<(u32, String)> {
        let all: Vec<(u32, String)> = list::questions(&self.issues, "")
            .into_iter()
            .map(|(i, q)| (i.id, q))
            .collect();
        let start = after
            .and_then(|(id, q)| all.iter().position(|(i, x)| *i == id && x == q))
            .map_or(0, |p| p + 1);
        all.iter()
            .cycle()
            .skip(start)
            .take(all.len())
            .find(|(id, q)| after != Some((*id, q.as_str())))
            .cloned()
    }

    pub fn scroll(&mut self, by: isize) {
        match &mut self.screen {
            Screen::Question { top, .. } => *top = top.saturating_add_signed(by),
            Screen::View(v) => {
                let code = if by > 0 {
                    KeyCode::Char('j')
                } else {
                    KeyCode::Char('k')
                };
                for _ in 0..by.unsigned_abs() {
                    v.key(KeyEvent::new(code, KeyModifiers::NONE));
                }
            }
            Screen::List => {}
        }
    }

    pub fn paste(&mut self, text: &str) {
        if let Some((_, input)) = self.prompt.as_mut() {
            input.insert(text);
        } else if let Screen::Question { id, qid, .. } = &self.screen {
            let key = (*id, qid.clone());
            self.drafts.entry(key).or_default().input.insert(text);
        }
    }

    /// Sets the screens `space m` can switch to.
    pub fn set_modes(&mut self, modes: Vec<Mode>) {
        self.modes = modes;
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        self.outcome = Outcome::None;
        if key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::SUPER)
            && let Some(text) = self.selection()
        {
            self.notice = Some(match lgtm_tui_kit::clipboard::copy(&text) {
                Ok(()) => "copied".into(),
                Err(e) => format!("couldn't copy: {e}"),
            });
            return Outcome::None;
        }
        if self.prompt.is_some() {
            self.prompt_key(key);
        } else if let Some(id) = self.status_for.take() {
            let status = self
                .wf
                .statuses
                .iter()
                .find(|s| key.code == KeyCode::Char(s.key));
            if let Some(status) = status.map(|s| s.id.clone()) {
                self.set_status(id, status);
            }
        } else if std::mem::take(&mut self.mode_menu) {
            if let KeyCode::Char(c) = key.code
                && let Some(m) = Mode::pick(c, &self.modes)
            {
                self.outcome = Outcome::Mode(m);
            }
        } else if std::mem::take(&mut self.space) {
            match key.code {
                KeyCode::Char('f') => self.outcome = Outcome::Folders,
                KeyCode::Char('m') => self.mode_menu = true,
                _ => {}
            }
        } else if key.code == KeyCode::Char(' ') && !matches!(self.screen, Screen::Question { .. })
        {
            self.space = true;
        } else {
            match &self.screen {
                Screen::List => self.list_key(key),
                Screen::Question { .. } => self.question_key(key),
                Screen::View(_) => self.view_key(key),
            }
        }
        std::mem::replace(&mut self.outcome, Outcome::None)
    }

    /// The space menu, while Space waits for its second key, or the status menu, where a
    /// status shows what the issue lacks to move there.
    pub fn space_menu(&self) -> Vec<(String, String)> {
        if let Some(issue) = self.status_for.and_then(|id| self.issue(id)) {
            return self
                .wf
                .statuses
                .iter()
                .map(|s| {
                    let unmet = self.wf.unmet(issue, &self.issues, s);
                    let label = match unmet.is_empty() {
                        true => s.label.clone(),
                        false => {
                            let needs = ops::needs(issue, &self.issues, &self.wf, &unmet);
                            format!("{} · {needs}", s.label)
                        }
                    };
                    (s.key.to_string(), label)
                })
                .collect();
        }
        if self.mode_menu {
            return mode::menu(&self.modes)
                .into_iter()
                .map(|(k, l)| (k, l.to_string()))
                .collect();
        }
        if !self.space {
            return vec![];
        }
        vec![
            ("f".into(), "issues folders".into()),
            ("m".into(), "mode".into()),
        ]
    }

    /// Runs a `:` command.
    fn command(&mut self, line: &str) {
        match line.trim() {
            "q" | "q!" | "qa" | "qa!" | "quit" => self.outcome = Outcome::Quit,
            "h" | "home" => self.outcome = Outcome::Home,
            "" => {}
            other => self.notice = Some(format!("unknown command :{other}")),
        }
    }

    fn prompt_key(&mut self, key: KeyEvent) {
        let Some((ask, mut input)) = self.prompt.take() else {
            return;
        };
        let mut sent = None;
        match key.code {
            KeyCode::Esc => {
                if ask == Ask::Filter {
                    self.filter.clear();
                }
            }
            KeyCode::Enter => match ask {
                Ask::New => match self.history.record(&self.folder, None, || {
                    let (id, d) =
                        ops::file(&self.folder, &input.text, "", &self.who, None, vec![])?;
                    sent = d;
                    Ok(id)
                }) {
                    Ok(id) => {
                        let d = sent.take();
                        self.deliver(d);
                        self.reload();
                        self.select_issue(id);
                    }
                    Err(e) => self.notice = Some(format!("{e:#}")),
                },
                Ask::Filter => {}
                Ask::Command => self.command(&input.text),
            },
            _ => {
                input.key(key);
                if ask == Ask::Filter {
                    self.filter = input.text.clone();
                    self.clamp();
                }
                self.prompt = Some((ask, input));
            }
        }
    }

    fn select_issue(&mut self, id: u32) {
        if let Some(p) = self.rows().iter().position(|r| *r == Row::Issue(id)) {
            self.sel = p;
        }
    }

    fn move_sel(&mut self, by: isize) {
        let rows = self.rows();
        let mut i = self.sel as isize;
        loop {
            i += by;
            match rows.get(i as usize) {
                _ if i < 0 => return,
                None => return,
                Some(r) if r.pickable() => break,
                Some(_) => {}
            }
        }
        self.sel = i as usize;
    }

    fn list_key(&mut self, key: KeyEvent) {
        let rows = self.rows();
        let row = rows.get(self.sel).cloned();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_sel(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_sel(-1),
            KeyCode::Char('g') | KeyCode::Home => {
                self.sel = 0;
                self.clamp();
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.sel = rows.iter().rposition(Row::pickable).unwrap_or(0);
            }
            KeyCode::Enter => match row {
                Some(Row::Question { id, qid }) => {
                    self.screen = Screen::Question { id, qid, top: 0 }
                }
                Some(Row::Issue(id)) => self.open(id),
                Some(Row::Folded(status, ..)) => self.toggle(status),
                _ => {}
            },
            KeyCode::Char('s') => self.status_for = row.and_then(|r| r.issue()),
            KeyCode::Char('u') => self.undo(false),
            KeyCode::Char('U') => self.undo(true),
            KeyCode::Char('n') => self.prompt = Some((Ask::New, Input::default())),
            KeyCode::Char('/') => {
                let mut input = Input::default();
                input.insert(&self.filter);
                self.prompt = Some((Ask::Filter, input));
            }
            KeyCode::Char(':') => self.prompt = Some((Ask::Command, Input::default())),
            KeyCode::Esc => {
                self.filter.clear();
                self.clamp();
            }
            _ => {}
        }
    }

    fn open(&mut self, id: u32) {
        if let Some(issue) = self.issue(id) {
            self.screen = Screen::View(Box::new(View::new(issue)));
        }
    }

    /// Sets an issue's status, which moves its file to the folder for it, without a note in
    /// its log, so setting it back leaves the file as it was. A status whose requirements
    /// the issue doesn't meet refuses it.
    fn set_status(&mut self, id: u32, status: String) {
        let mut sent = None;
        let set = self.history.record(&self.folder, Some(id), || {
            sent = ops::move_to(&self.folder, id, &status, false, None)?;
            Ok(id)
        });
        if let Err(e) = set {
            self.notice = Some(format!("{e:#}"));
        }
        self.deliver(sent);
        self.reload();
        self.select_issue(id);
    }

    /// Tells a status's terminal that an issue moved there.
    fn deliver(&mut self, d: Option<ops::Delivery>) {
        if let Some(d) = d
            && let Err(e) = lgterm::send(&d.to, &d.text)
        {
            self.notice = Some(format!("moved; couldn't tell {}: {e:#}", d.to));
        }
    }

    fn undo(&mut self, redo: bool) {
        let done = if redo {
            self.history.redo(&self.folder)
        } else {
            self.history.undo(&self.folder)
        };
        match done {
            Ok(Some(id)) => {
                self.reload();
                self.select_issue(id);
            }
            Ok(None) => {
                self.notice = Some(
                    if redo {
                        "nothing to redo"
                    } else {
                        "nothing to undo"
                    }
                    .into(),
                )
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
    }

    fn view_key(&mut self, key: KeyEvent) {
        let Screen::View(v) = &mut self.screen else {
            return;
        };
        let id = v.id;
        match key.code {
            KeyCode::Char('e') => {
                if let Ok(path) = self.folder.path(id) {
                    self.outcome = Outcome::Edit(path);
                }
            }
            KeyCode::Char('s') => self.status_for = Some(id),
            KeyCode::Char('u') => self.undo(false),
            KeyCode::Char('U') => self.undo(true),
            KeyCode::Enter => {
                let next = self
                    .issue(id)
                    .and_then(|i| i.waiting().first().and_then(|q| q.qid()).map(str::to_owned));
                if let Some(qid) = next {
                    self.screen = Screen::Question { id, qid, top: 0 };
                }
            }
            _ => match v.key(key) {
                ViewOutcome::Close => self.screen = Screen::List,
                ViewOutcome::Command => self.prompt = Some((Ask::Command, Input::default())),
                ViewOutcome::None => {}
            },
        }
    }

    fn question_key(&mut self, key: KeyEvent) {
        let Screen::Question { id, qid, top } = &mut self.screen else {
            return;
        };
        let (id, qid) = (*id, qid.clone());
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let half = (self.height / 2).max(1) as isize;
        let draft = self.drafts.entry((id, qid.clone())).or_default();
        match key.code {
            KeyCode::Esc => self.screen = Screen::List,
            KeyCode::Tab => {
                if let Some((id, qid)) = self.next_question(Some((id, &qid))) {
                    self.screen = Screen::Question { id, qid, top: 0 };
                }
            }
            KeyCode::Char('o') if ctrl => self.open(id),
            KeyCode::Char('d') if ctrl => *top = top.saturating_add_signed(half),
            KeyCode::Char('u') if ctrl && draft.input.is_empty() => {
                *top = top.saturating_sub(half as usize)
            }
            KeyCode::PageDown => *top = top.saturating_add_signed(half * 2),
            KeyCode::PageUp => *top = top.saturating_sub(half as usize * 2),
            KeyCode::Enter => self.send(id, &qid),
            KeyCode::Backspace if draft.input.is_empty() => draft.pick = None,
            _ => {
                if let Some(n) = alt_digit(&key) {
                    let count = self
                        .issue(id)
                        .and_then(|i| i.log.iter().rev().find(|e| e.qid() == Some(&qid)))
                        .map_or(0, |q| qq::options(&q.text).len());
                    let draft = self.drafts.entry((id, qid)).or_default();
                    if n <= count {
                        draft.pick = Some(n);
                    }
                } else {
                    draft.input.key(key);
                }
            }
        }
    }

    fn send(&mut self, id: u32, qid: &str) {
        let Some(draft) = self.drafts.get(&(id, qid.to_owned())) else {
            return;
        };
        let text = draft.input.text.trim().to_owned();
        let response = match draft.pick {
            Some(option) => ops::Response::Pick {
                option,
                notes: text,
            },
            None if text.is_empty() => return,
            None => ops::Response::Reply(text),
        };
        match ops::answer(&self.folder, id, qid, &self.who, &response) {
            Ok(delivery) => {
                self.drafts.remove(&(id, qid.to_owned()));
                if let Some(d) = delivery
                    && let Err(e) = lgterm::send(&d.to, &d.text)
                {
                    self.notice = Some(format!("saved; not sent to {}: {e:#}", d.to));
                }
            }
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                return;
            }
        }
        self.reload();
        self.screen = match self.next_question(Some((id, qid))) {
            Some((id, qid)) => Screen::Question { id, qid, top: 0 },
            None => Screen::List,
        };
    }

    /// The key bar's badge and hints for the screen that's up.
    fn hints(&self) -> (Option<(&'static str, Color)>, Vec<Hint<'static>>) {
        match &self.screen {
            Screen::List => {
                let hints = vec![
                    ("j k", "move", false),
                    ("Enter", "open", false),
                    ("n", "new", false),
                    ("s", "status", false),
                    ("u U", "undo redo", false),
                    ("/", "filter", !self.filter.is_empty()),
                    ("Space", "menu", false),
                ];
                (Some(("ISS", AMBER)), hints)
            }
            Screen::Question { id, qid, .. } => {
                let picked = self
                    .drafts
                    .get(&(*id, qid.clone()))
                    .is_some_and(|d| d.pick.is_some());
                let mut hints = vec![("⌥1-9", "pick", picked), ("Enter", "send", false)];
                if picked {
                    hints.push(("BS", "unpick", false));
                }
                hints.extend([
                    ("C-d C-u", "scroll", false),
                    ("Tab", "next question", false),
                    ("C-o", "issue", false),
                    ("Esc", "list", false),
                ]);
                (Some(("ANS", AMBER)), hints)
            }
            Screen::View(v) => {
                let mut hints = vec![("j k", "scroll", false)];
                let waiting = self
                    .issues
                    .iter()
                    .find(|i| i.id == v.id)
                    .is_some_and(|i| !i.waiting().is_empty());
                if waiting {
                    hints.push(("Enter", "answer", false));
                }
                hints.extend([
                    ("e", "edit", false),
                    ("s", "status", false),
                    ("u U", "undo redo", false),
                    ("Esc", "list", false),
                ]);
                (Some(("ISS", AMBER)), hints)
            }
        }
    }

    pub fn draw(&mut self, c: &mut Canvas) {
        let area = c.area;
        for y in area.y..area.bottom() {
            c.fill(area.x, y, area.width, BG);
        }
        let (left, right) = self.title();
        let width = area.width.saturating_sub(1);
        let head = flow::rows(width, &left, &right);
        let (badge, hints) = self.hints();
        let foot = if self.prompt.is_some() || self.notice.is_some() {
            1
        } else {
            keybar::rows(area.width, badge, &hints)
        };
        if area.height < head + foot + 2 {
            return;
        }
        flow::draw(c, (area.x, area.y), width, &left, &right);
        let body = Rect {
            y: area.y + head + 1,
            height: area.height - head - 1 - foot,
            ..area
        };
        let bottom = area.bottom() - 1;
        match &mut self.screen {
            Screen::List => {
                let rows = self.rows();
                self.height = body.height as usize;
                if self.sel < self.top {
                    self.top = self.sel;
                } else if self.sel >= self.top + self.height {
                    self.top = self.sel + 1 - self.height;
                }
                self.list_area = body;
                list::draw(c, body, (&self.issues, &self.wf), &rows, self.sel, self.top);
                if !self.bad.is_empty() {
                    let y = body.bottom() - 1;
                    c.fill(area.x, y, area.width, BG);
                    c.put(
                        area.x + 1,
                        y,
                        &format!("unreadable: {}", self.bad.join(", ")),
                        theme::fg(RED),
                    );
                }
            }
            Screen::Question { id, qid, top } => {
                let (id, qid) = (*id, qid.clone());
                let Some(issue) = self.issues.iter().find(|i| i.id == id) else {
                    self.screen = Screen::List;
                    return self.draw(c);
                };
                let Some(q) = issue.log.iter().rev().find(|e| e.qid() == Some(&qid)) else {
                    self.screen = Screen::List;
                    return self.draw(c);
                };
                let draft = self
                    .drafts
                    .get(&(id, qid.clone()))
                    .cloned()
                    .unwrap_or_default();
                self.height = body.height.saturating_sub(2) as usize;
                question::draw(c, body, (issue, &self.wf, &self.issues), q, &draft, top);
                keybar::draw_badged(c, bottom, badge, &hints);
                return self.overlay(c, bottom);
            }
            Screen::View(v) => {
                let id = v.id;
                let Some(issue) = self.issues.iter().find(|i| i.id == id) else {
                    self.screen = Screen::List;
                    return self.draw(c);
                };
                v.draw(c, body, (issue, &self.wf, &self.issues));
            }
        }
        keybar::draw_badged(c, bottom, badge, &hints);
        let items = self.space_menu();
        let menu: Vec<(String, &str)> =
            items.iter().map(|(k, l)| (k.clone(), l.as_str())).collect();
        keybar::menu(c, body, &menu);
        self.overlay(c, bottom);
    }

    /// The prompt or notice over the key bar.
    fn overlay(&self, c: &mut Canvas, y: u16) {
        let x0 = c.area.x;
        if let Some((ask, input)) = &self.prompt {
            let label = match ask {
                Ask::New => "new issue: ",
                Ask::Filter => "/",
                Ask::Command => ":",
            };
            c.fill(x0, y, c.area.width, PANEL);
            let x = c.put(x0 + 1, y, label, theme::bold(ACCENT).bg(PANEL));
            let before: String = input.text.chars().take(input.at).collect();
            c.put(x, y, &input.text, theme::fg(FG).bg(PANEL));
            c.bar_at(x + before.chars().count() as u16, y);
        } else if let Some(n) = &self.notice {
            c.fill(x0, y, c.area.width, PANEL);
            c.put(x0 + 1, y, n, theme::fg(FG).bg(PANEL));
        }
    }

    /// The title bar's pieces: the folder on the left and the counts by status on the right.
    fn title(&self) -> (Vec<Piece>, Vec<Piece>) {
        let waiting: usize = self.issues.iter().map(|i| i.waiting().len()).sum();
        let quiet = |id: &str| {
            self.issues
                .iter()
                .filter(|i| i.status == id && i.waiting().is_empty())
                .count()
        };
        let left = vec![
            Piece::new(0, "issues", theme::fg(ACCENT)),
            Piece::new(2, picker::home(&self.folder.root), theme::fg(FG)),
        ];
        let mut right = vec![Piece::new(2, format!("● {waiting}"), theme::fg(AMBER))];
        for s in self.wf.statuses.iter().filter(|s| !s.fold) {
            let text = format!("{} {}", s.glyph, quiet(&s.id));
            right.push(Piece::new(2, text, theme::fg(marks::color(&s.color))));
        }
        let folded: usize = self
            .wf
            .statuses
            .iter()
            .filter(|s| s.fold)
            .map(|s| quiet(&s.id))
            .sum();
        right.push(Piece::new(2, format!("✓ {folded}"), theme::fg(GREEN)));
        (left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_of(issues: &Issues, width: u16) -> (u16, Vec<String>) {
        let area = Rect::new(0, 0, width, 6);
        let mut buf = ratatui::buffer::Buffer::empty(area);
        let (left, right) = issues.title();
        let n = flow::draw(
            &mut Canvas::new(&mut buf, area),
            (0, 0),
            width - 1,
            &left,
            &right,
        );
        let rows = (0..area.height)
            .map(|y| {
                let row: String = (0..width).map(|x| buf[(x, y)].symbol()).collect();
                row.trim_end().to_owned()
            })
            .collect();
        (n, rows)
    }

    #[test]
    fn the_title_wraps_its_counts_onto_more_rows_instead_of_overlapping() {
        let dir = tempfile::tempdir().unwrap();
        let issues = Issues::new(Folder::init(dir.path()).unwrap(), "jordan".into());
        let (n, rows) = rows_of(&issues, 200);
        assert_eq!(n, 1);
        assert!(rows[0].starts_with("issues  "));
        assert!(rows[0].ends_with("✓ 0"));
        let (n, rows) = rows_of(&issues, 14);
        assert!(n > 3);
        assert_eq!(rows[0], "issues");
        let counts: Vec<&str> = rows[2..n as usize]
            .iter()
            .flat_map(|r| r.split("  "))
            .collect();
        assert_eq!(counts.first(), Some(&"● 0"));
        assert_eq!(counts.last(), Some(&"✓ 0"));
        assert!(counts.iter().all(|c| c.chars().count() == 3));
    }

    #[test]
    fn space_m_switches_only_to_the_modes_the_host_offers() {
        let dir = tempfile::tempdir().unwrap();
        let mut issues = Issues::new(Folder::init(dir.path()).unwrap(), "jordan".into());
        issues.set_modes(vec![Mode::Editor]);
        let press = |i: &mut Issues, c| i.key(KeyEvent::from(KeyCode::Char(c)));
        press(&mut issues, ' ');
        press(&mut issues, 'm');
        assert_eq!(
            issues.space_menu(),
            [("e".to_string(), "editor".to_string())]
        );
        assert_eq!(press(&mut issues, 'r'), Outcome::None);
        press(&mut issues, ' ');
        press(&mut issues, 'm');
        assert_eq!(press(&mut issues, 'e'), Outcome::Mode(Mode::Editor));
    }

    #[test]
    fn option_digits_come_as_alt_or_as_mac_option_characters() {
        let key = |c, m| KeyEvent::new(KeyCode::Char(c), m);
        assert_eq!(alt_digit(&key('3', KeyModifiers::ALT)), Some(3));
        assert_eq!(alt_digit(&key('£', KeyModifiers::NONE)), Some(3));
        assert_eq!(alt_digit(&key('ª', KeyModifiers::NONE)), Some(9));
        assert_eq!(alt_digit(&key('3', KeyModifiers::NONE)), None);
        assert_eq!(alt_digit(&key('0', KeyModifiers::ALT)), None);
    }
}
