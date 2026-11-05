//! The screen `lgtm` opens on: the jj log on the left to pick a review from, and the repo's
//! files on the right to open in the editor. Tab moves between them.

use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::files::{self, FileTree};
use super::keybar::Hint;
use super::load::Target;
use super::motions::{self, Menu};
use super::paint::Canvas;
use super::picker::{self, Picker};
use super::theme::{self, ACCENT, AMBER, BG, CYAN, FG, HAIR, MUTED};
use super::width;
use lgtm_core::jj::{JjAction, PushPlan};

pub enum Outcome {
    None,
    Open(Target),
    /// `jj edit` the commit, then open its review.
    EditOpen {
        rev: String,
        target: Target,
    },
    Versions(Vec<String>),
    Edit(String),
    /// Run a jj menu motion.
    Jj(JjAction),
    /// Load the bookmarks nearest this commit, for the jj menu.
    Bookmarks(String),
    /// Find what a push of these commits' bookmarks would do, for the jj menu.
    PushPlan(String),
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Revisions,
    Files,
}

pub struct Home {
    pub picker: Picker,
    pub files: FileTree,
    focus: Focus,
    /// The `:` line being typed.
    cmd: Option<String>,
    /// A failure to show until the next key.
    pub notice: Option<String>,
    /// What a running jj action on the revisions is doing, with its spinner frame, shown in
    /// the Revisions header.
    pub busy: Option<String>,
    /// jj mode's menu, while the mode is on: the motions' keys act on the cursor's commit,
    /// and the log's moves still work.
    menu: Option<Menu>,
    /// The revisions side's width, once `<` or `>` set it.
    pub revisions_width: Option<u16>,
    /// The revisions side's width as last drawn.
    drawn_width: u16,
}

/// The narrowest either side gets.
const SIDE_MIN: u16 = 20;

/// The `:` commands, for the menu over the command line.
const COMMANDS: &[(&str, &str)] = &[("q", "quit")];

impl Default for Home {
    fn default() -> Self {
        Self {
            picker: Picker::default(),
            files: FileTree::default(),
            focus: Focus::Revisions,
            cmd: None,
            notice: None,
            busy: None,
            menu: None,
            revisions_width: None,
            drawn_width: 0,
        }
    }
}

/// The keys of the revisions side, for `?`.
pub const KEYS: &[(&str, &str)] = &[
    ("Tab", "files"),
    ("< >", "move divider"),
    ("j k", "move"),
    ("J K", "range"),
    ("Esc", "clear range"),
    ("Space", "jj mode"),
    ("Enter", "edit the commit and review it"),
    ("r", "review"),
    ("v", "snapshots"),
    ("a", "every snapshot"),
    (":", "command"),
];

impl Home {
    /// Whether keys are going into the search box.
    pub fn typing(&self) -> bool {
        self.focus == Focus::Files || self.cmd.is_some() || self.menu.is_some()
    }

    /// The `:` line, or what the jj menu asks for, as it shows.
    pub fn prompt(&self) -> Option<String> {
        if let Some(p) = self.menu.as_ref().and_then(Menu::prompt) {
            return Some(p);
        }
        self.cmd.as_ref().map(|c| format!(":{c}"))
    }

    /// The commands that match what's typed, or the jj menu's motions.
    pub fn command_menu(&self) -> Vec<(String, &'static str)> {
        if let Some(menu) = &self.menu {
            return menu.items();
        }
        let Some(typed) = &self.cmd else {
            return vec![];
        };
        COMMANDS
            .iter()
            .filter(|(name, _)| name.starts_with(typed.as_str()))
            .map(|(name, label)| (format!(":{name}"), *label))
            .collect()
    }

    fn command_key(&mut self, key: KeyEvent) -> Outcome {
        let Some(typed) = self.cmd.as_mut() else {
            return Outcome::None;
        };
        match key.code {
            KeyCode::Esc => self.cmd = None,
            KeyCode::Backspace if typed.is_empty() => self.cmd = None,
            KeyCode::Backspace => {
                typed.pop();
            }
            KeyCode::Tab => {
                let matches: Vec<&str> = COMMANDS
                    .iter()
                    .map(|(n, _)| *n)
                    .filter(|n| n.starts_with(typed.as_str()))
                    .collect();
                if let [one] = matches[..] {
                    *typed = one.to_string();
                }
            }
            KeyCode::Enter => {
                let line = self.cmd.take().unwrap_or_default();
                match line.trim() {
                    "q" | "q!" | "quit" => return Outcome::Quit,
                    "" => {}
                    other => self.notice = Some(format!("no command {other}")),
                }
            }
            KeyCode::Char(c) => typed.push(c),
            _ => {}
        }
        Outcome::None
    }

    /// Takes a jj menu step. A finished or dropped motion leaves jj mode on, ready for the
    /// next.
    fn menu_step(&mut self, step: motions::Step) -> Outcome {
        match step {
            motions::Step::Stay => Outcome::None,
            motions::Step::Close => {
                self.set_jj_mode(true);
                Outcome::None
            }
            motions::Step::Run(action) => {
                self.set_jj_mode(true);
                Outcome::Jj(action)
            }
            motions::Step::Bookmarks(rev) => Outcome::Bookmarks(rev),
            motions::Step::PushPlan(revs) => Outcome::PushPlan(revs),
        }
    }

    /// Takes what a push would do, for the jj menu.
    pub fn push_plan(&mut self, plan: anyhow::Result<PushPlan>) -> Outcome {
        let Some(menu) = self.menu.as_mut() else {
            return Outcome::None;
        };
        match plan {
            Ok(plan) => {
                let step = menu.push_plan(plan);
                self.menu_step(step)
            }
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                self.set_jj_mode(true);
                Outcome::None
            }
        }
    }

    /// Takes the bookmarks nearest the cursor for the jj menu.
    pub fn bookmarks(&mut self, found: anyhow::Result<Vec<String>>) -> Outcome {
        let Some(menu) = self.menu.as_mut() else {
            return Outcome::None;
        };
        match found {
            Ok(names) if names.is_empty() => {
                self.notice = Some("no bookmarks near the commit".into());
                self.set_jj_mode(true);
                Outcome::None
            }
            Ok(names) => {
                let step = menu.bookmarks(names);
                self.menu_step(step)
            }
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                self.set_jj_mode(true);
                Outcome::None
            }
        }
    }

    /// A click in the file tree opens a file or a folder, and the wheel scrolls it. A click
    /// in the log picks a commit or snapshot, a double click opens it, and the wheel moves
    /// through them.
    pub fn mouse(&mut self, ev: MouseEvent) -> Outcome {
        let jj = self.jj_mode();
        if jj && !self.menu.as_ref().is_some_and(Menu::idle) {
            return Outcome::None;
        }
        if self.picker.contains(ev.column, ev.row) {
            let outcome = match ev.kind {
                MouseEventKind::ScrollDown => {
                    self.picker.scroll(true);
                    picker::Outcome::None
                }
                MouseEventKind::ScrollUp => {
                    self.picker.scroll(false);
                    picker::Outcome::None
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    self.focus = Focus::Revisions;
                    self.picker.click(ev.row)
                }
                _ => picker::Outcome::None,
            };
            let opens = !matches!(outcome, picker::Outcome::None);
            self.set_jj_mode(jj && !opens);
            return self.picked(outcome);
        }
        if !self.files.contains(ev.column, ev.row) {
            return Outcome::None;
        }
        if matches!(ev.kind, MouseEventKind::Down(_)) {
            self.set_jj_mode(false);
        }
        match ev.kind {
            MouseEventKind::ScrollDown => self.files.scroll(3),
            MouseEventKind::ScrollUp => self.files.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = Focus::Files;
                if let files::Outcome::Open(path) = self.files.click(ev.row) {
                    return Outcome::Edit(path);
                }
            }
            _ => {}
        }
        Outcome::None
    }

    /// Whether jj mode is on.
    pub fn jj_mode(&self) -> bool {
        self.menu.is_some()
    }

    /// Turns jj mode on with a fresh menu on the cursor's commit, or off without one.
    fn set_jj_mode(&mut self, on: bool) {
        self.menu = on
            .then(|| self.picker.motion_target().map(Menu::new))
            .flatten();
    }

    /// A key in jj mode. With no motion under way, Space, Esc and Tab leave the mode, and the
    /// log's moves move; while a jj action runs, nothing else does anything.
    fn jj_key(&mut self, key: KeyEvent) -> Outcome {
        if self.menu.as_ref().is_some_and(Menu::idle) {
            match key.code {
                KeyCode::Char(' ') => self.set_jj_mode(false),
                KeyCode::Esc if self.picker.anchored() => {
                    self.picker.key(key);
                }
                KeyCode::Esc => self.set_jj_mode(false),
                KeyCode::Tab => {
                    self.set_jj_mode(false);
                    self.focus = Focus::Files;
                }
                _ if self.busy.is_some() => {}
                KeyCode::Char('j' | 'k' | 'J' | 'K') | KeyCode::Up | KeyCode::Down => {
                    self.picker.key(key);
                    self.set_jj_mode(true);
                }
                KeyCode::Enter | KeyCode::Char('r') => {
                    let outcome = self.picker.key(key);
                    self.set_jj_mode(false);
                    return self.picked(outcome);
                }
                _ => {
                    // The log may have changed under the menu since it opened.
                    self.set_jj_mode(true);
                    return self.menu_key(key);
                }
            }
            return Outcome::None;
        }
        self.menu_key(key)
    }

    fn menu_key(&mut self, key: KeyEvent) -> Outcome {
        match self.menu.as_mut() {
            Some(menu) => {
                let step = menu.key(key);
                self.menu_step(step)
            }
            None => Outcome::None,
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        if self.jj_mode() {
            return self.jj_key(key);
        }
        if self.cmd.is_some() {
            return self.command_key(key);
        }
        if let Some(step) = width::step(&key) {
            self.revisions_width = Some(width::moved(self.drawn_width, step, SIDE_MIN));
            return Outcome::None;
        }
        if key.code == KeyCode::Char(' ') && self.focus == Focus::Revisions {
            self.set_jj_mode(true);
            return Outcome::None;
        }
        let empty_search = self.focus == Focus::Revisions || self.files.query.is_empty();
        if key.code == KeyCode::Char(':') && empty_search {
            self.cmd = Some(String::new());
            return Outcome::None;
        }
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::Revisions => Focus::Files,
                Focus::Files => Focus::Revisions,
            };
            return Outcome::None;
        }
        match self.focus {
            Focus::Files => match self.files.key(key) {
                files::Outcome::Open(path) => Outcome::Edit(path),
                files::Outcome::None => Outcome::None,
            },
            Focus::Revisions => {
                let outcome = self.picker.key(key);
                self.picked(outcome)
            }
        }
    }

    fn picked(&self, outcome: picker::Outcome) -> Outcome {
        match outcome {
            picker::Outcome::Open(t) => Outcome::Open(t),
            picker::Outcome::EditOpen { rev, target } => Outcome::EditOpen { rev, target },
            picker::Outcome::Versions(v) => Outcome::Versions(v),
            picker::Outcome::None => Outcome::None,
        }
    }

    /// Where the menu over the key bar goes: in jj mode over the files side, clear of the log.
    pub fn menu_area(&self, body: Rect) -> Rect {
        if !self.jj_mode() {
            return body;
        }
        let x = body.x + self.drawn_width + 2;
        Rect::new(x, body.y, body.right().saturating_sub(x), body.height)
    }

    /// jj mode's badge for the key bar, gray while a jj action runs.
    pub fn badge(&self) -> Option<(&'static str, ratatui::style::Color)> {
        self.jj_mode().then_some(("jj", self.jj_color()))
    }

    fn jj_color(&self) -> ratatui::style::Color {
        if self.busy.is_some() { MUTED } else { AMBER }
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        if self.jj_mode() {
            let esc = if self.picker.anchored() {
                "clear"
            } else {
                "leave"
            };
            return vec![
                ("Space", "leave", false),
                ("j k", "move", false),
                ("J K", "range", false),
                ("Enter", "edit", false),
                ("r", "review", false),
                ("Esc", esc, false),
            ];
        }
        match self.focus {
            Focus::Revisions => {
                let mut hints = vec![("Tab", "files", false)];
                hints.extend(self.picker.hints());
                hints
            }
            Focus::Files => {
                let mut hints = vec![
                    ("Tab", "revisions", false),
                    ("Enter", "open", false),
                    ("↑ ↓", "move", false),
                ];
                if self.files.query.is_empty() {
                    hints.push(("← →", "close, open", false));
                } else {
                    hints.push(("Esc", "clear", false));
                }
                hints.push((":q", "quit", false));
                hints
            }
        }
    }

    pub fn draw(&mut self, c: &mut Canvas, body: Rect) {
        let left_w = width::fit(
            self.revisions_width,
            body.width / 2,
            SIDE_MIN,
            body.width.saturating_sub(SIDE_MIN + 1),
        );
        self.drawn_width = left_w;
        let left = Rect::new(body.x, body.y, left_w, body.height);
        let right = Rect::new(
            body.x + left_w + 1,
            body.y,
            body.width.saturating_sub(left_w + 1),
            body.height,
        );
        {
            let mut sub = Canvas::new(&mut *c.buf, left);
            self.picker
                .draw(&mut sub, left, self.focus == Focus::Revisions);
            let style = theme::title(self.focus == Focus::Revisions);
            sub.put(left.x + 1, left.y, "Revisions", style.bg(BG));
            if self.jj_mode() {
                self.draw_jj_frame(&mut sub, left);
            }
            if let Some(busy) = &self.busy {
                sub.put_right(left.y, 1, busy, theme::fg(CYAN).bg(BG));
            }
        }
        // jj mode's frame stands in for the divider.
        if !self.jj_mode() {
            for y in body.y..body.bottom() {
                c.put(body.x + left_w, y, "│", theme::fg(HAIR).bg(BG));
            }
        }
        let mut sub = Canvas::new(&mut *c.buf, right);
        let x0 = right.x + 1;
        let focused = self.focus == Focus::Files;
        sub.put(x0, right.y, "Files", theme::title(focused).bg(BG));
        if right.height < 4 {
            return;
        }
        let x = sub.put(x0, right.y + 1, "› ", theme::bold(ACCENT).bg(BG));
        let x = sub.put(x, right.y + 1, &self.files.query, theme::fg(FG).bg(BG));
        if focused {
            sub.put(x, right.y + 1, " ", theme::fg(FG).bg(ACCENT));
        }
        if let Some(count) = self.files.count() {
            sub.put_right(right.y + 1, 2, &count, theme::fg(MUTED).bg(BG));
        }
        let tree = Rect::new(right.x, right.y + 3, right.width, right.height - 3);
        self.files.draw(&mut sub, tree, focused, &|_| false);
    }
}

impl Home {
    /// jj mode's frame around the revisions side, its title tagged jj.
    fn draw_jj_frame(&self, c: &mut Canvas, r: Rect) {
        if r.width < 4 || r.height < 2 {
            return;
        }
        let line = theme::fg(self.jj_color()).bg(BG);
        let (right, bottom) = (r.right() - 1, r.bottom() - 1);
        let across = "─".repeat(usize::from(r.width - 2));
        c.put(r.x, r.y, &format!("╭{across}╮"), line);
        c.put(r.x, bottom, &format!("╰{across}╯"), line);
        for y in r.y + 1..bottom {
            c.put(r.x, y, "│", line);
            c.put(right, y, "│", line);
        }
        let x = c.put(r.x + 2, r.y, " Revisions ", theme::bold(FG).bg(BG));
        c.put(x, r.y, " jj ", theme::bold(BG).bg(self.jj_color()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyModifiers;

    fn press(h: &mut Home, keys: &str) -> Vec<Outcome> {
        crate::edit::spec::keys(keys)
            .into_iter()
            .map(|k| h.key(KeyEvent::new(k.code, KeyModifiers::NONE)))
            .collect()
    }

    #[test]
    fn colon_q_quits_from_either_side_and_an_unknown_command_says_so() {
        let mut h = Home::default();
        assert!(matches!(
            press(&mut h, ":q<ret>").last(),
            Some(Outcome::Quit)
        ));
        press(&mut h, "<tab>");
        assert!(matches!(
            press(&mut h, ":q<ret>").last(),
            Some(Outcome::Quit)
        ));
        press(&mut h, ":zz<ret>");
        assert_eq!(h.notice.as_deref(), Some("no command zz"));
        // With a search typed, `:` is part of it.
        press(&mut h, "a:");
        assert_eq!(h.files.query, "a:");
    }

    fn with_log(changes: &[&str]) -> Home {
        let rows = changes
            .iter()
            .map(|change| lgtm_core::jj::LogRow {
                graph: vec!["○  ".into()],
                commit: Some(lgtm_core::jj::LogCommit {
                    commit_id: format!("{change}-commit"),
                    change_id: (*change).into(),
                    unique: 2,
                    description: String::new(),
                    bookmarks: vec![],
                    author: String::new(),
                    time: 0.0,
                    working_copy: false,
                    immutable: false,
                    conflict: false,
                    divergent: false,
                    empty: false,
                }),
            })
            .collect();
        let mut h = Home::default();
        h.picker.loaded(Ok(rows));
        h
    }

    fn ran(outcomes: Vec<Outcome>) -> Vec<JjAction> {
        outcomes
            .into_iter()
            .filter_map(|o| match o {
                Outcome::Jj(a) => Some(a),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn jj_mode_runs_motions_on_each_commit_the_cursor_moves_to_and_stays_on() {
        let mut h = with_log(&["aaa", "bbb"]);
        press(&mut h, "<space>");
        assert!(h.jj_mode());
        let actions = ran(press(&mut h, "najna"));
        assert_eq!(
            actions,
            vec![
                JjAction::NewAfter { rev: "aaa".into() },
                JjAction::NewAfter { rev: "bbb".into() },
            ]
        );
        assert!(h.jj_mode());
        press(&mut h, "<space>");
        assert!(!h.jj_mode());
    }

    #[test]
    fn jj_mode_ignores_motions_and_moves_while_an_action_runs() {
        let mut h = with_log(&["aaa", "bbb"]);
        press(&mut h, "<space>");
        h.busy = Some("making a change".into());
        assert!(ran(press(&mut h, "jna")).is_empty());
        h.busy = None;
        assert_eq!(
            ran(press(&mut h, "na")),
            vec![JjAction::NewAfter { rev: "aaa".into() }]
        );
    }

    #[test]
    fn tab_leaves_jj_mode_so_letters_search_the_files() {
        let mut h = with_log(&["aaa"]);
        press(&mut h, "<space><tab>");
        assert!(!h.jj_mode());
        assert!(ran(press(&mut h, "e")).is_empty());
        assert_eq!(h.files.query, "e");
    }

    #[test]
    fn esc_in_jj_mode_clears_a_range_before_leaving() {
        let mut h = with_log(&["aaa", "bbb"]);
        press(&mut h, "<space>J<esc>");
        assert!(h.jj_mode());
        assert!(!h.picker.anchored());
        press(&mut h, "<esc>");
        assert!(!h.jj_mode());
    }
}
