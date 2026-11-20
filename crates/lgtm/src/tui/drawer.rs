//! The revisions drawer over the review's file list: the log to pick the next review from,
//! with jj's motions on the cursor's commit. `space j` opens it.

use crate::tui::help::Group;
use ratatui::crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::keybar::{self, Hint};
use super::load::Target;
use super::motions::{self, Menu};
use super::paint::Canvas;
use super::picker::{self, Picker};
use super::theme::{self, AMBER, BG, CYAN, FG, MUTED};
use lgtm_core::jj::{JjAction, PushPlan};
use lgtm_tui_kit::mode::{Leader, Mode, Press};

/// The screens `space m` switches to from the drawer, which is in the review.
const MODES: &[Mode] = &[Mode::Editor, Mode::Issues];

pub enum Outcome {
    None,
    /// Review this, the drawer closed.
    Open(Target),
    Versions(Vec<String>),
    /// Run a jj menu motion.
    Jj(JjAction),
    /// Load the bookmarks nearest this commit, for the jj menu.
    Bookmarks(String),
    /// Find what a push of these commits' bookmarks would do, for the jj menu.
    PushPlan(String),
    /// Switch to this screen, the drawer closed.
    Mode(Mode),
}

#[derive(Default)]
pub struct Drawer {
    pub picker: Picker,
    open: bool,
    /// The jj menu on the cursor's commit, while the drawer is open on a jj log.
    menu: Option<Menu>,
    /// What a running jj action is doing, with its spinner frame, shown in the title.
    pub busy: Option<String>,
    /// A failure to show until the next key.
    pub notice: Option<String>,
    leader: Leader,
}

/// The drawer's keys, for `?`. A git log has fewer.
pub fn help_groups(git: bool) -> Vec<Group> {
    let keys: &[(&str, &str)] = match git {
        true => &[
            ("j k", "move"),
            ("J K", "range"),
            ("Enter", "review"),
            ("Esc", "clear range, close"),
        ],
        false => &[
            ("j k", "move"),
            ("J K", "range"),
            ("Enter", "review"),
            ("v", "snapshots"),
            ("a", "every snapshot"),
            ("Esc", "clear range, close"),
        ],
    };
    vec![Group::new("Revisions", keys)]
}

impl Drawer {
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the drawer with no range and the cursor on `newest`, the commit id of the
    /// review's newest commit.
    pub fn show(&mut self, newest: Option<&str>) {
        self.picker.focus(newest.unwrap_or_default());
        self.open = true;
        self.notice = None;
        self.renew_menu();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.menu = None;
    }

    /// A fresh jj menu on the cursor's commit, as the log may have changed under the old.
    fn renew_menu(&mut self) {
        self.menu = (self.open && !self.picker.git)
            .then(|| self.picker.motion_target().map(Menu::new))
            .flatten();
    }

    /// What `space` goes on to, while it waits.
    pub fn leader_menu(&self) -> Vec<(String, &'static str)> {
        self.leader.menu(MODES, &[])
    }

    /// What the jj menu asks for on the bottom line.
    pub fn prompt(&self) -> Option<String> {
        self.menu.as_ref().and_then(Menu::prompt)
    }

    /// The jj menu's motions, or what it's picking from.
    pub fn menu_items(&self) -> Vec<(String, &'static str)> {
        self.menu.as_ref().map(Menu::items).unwrap_or_default()
    }

    /// Takes a jj menu step. A finished or dropped motion leaves a fresh menu, ready for the
    /// next.
    fn menu_step(&mut self, step: motions::Step) -> Outcome {
        match step {
            motions::Step::Stay => Outcome::None,
            motions::Step::Close => {
                self.renew_menu();
                Outcome::None
            }
            motions::Step::Run(action) => {
                self.renew_menu();
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
                self.renew_menu();
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
                self.renew_menu();
                Outcome::None
            }
            Ok(names) => {
                let step = menu.bookmarks(names);
                self.menu_step(step)
            }
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                self.renew_menu();
                Outcome::None
            }
        }
    }

    /// Whether a motion is under way, so its keys go to the menu.
    fn in_motion(&self) -> bool {
        self.menu.as_ref().is_some_and(|m| !m.idle())
    }

    /// Whether keys are going into a line being typed.
    pub fn typing(&self) -> bool {
        self.prompt().is_some() || self.leader.active()
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        if !self.in_motion() && self.prompt().is_none() {
            match self.leader.key(&key, MODES) {
                Press::Pass => {}
                Press::Switch(to) => {
                    self.close();
                    return Outcome::Mode(to);
                }
                Press::Held | Press::Other(_) => return Outcome::None,
            }
        }
        if self.in_motion() {
            let step = self.menu.as_mut().map(|m| m.key(key));
            return step.map_or(Outcome::None, |s| self.menu_step(s));
        }
        match key.code {
            KeyCode::Esc if self.picker.anchored() => {
                self.picker.key(key);
                self.renew_menu();
            }
            KeyCode::Esc => self.close(),
            // While a jj action runs, only Esc does anything.
            _ if self.busy.is_some() => {}
            KeyCode::Char('j' | 'k' | 'J' | 'K' | 'v' | 'a') | KeyCode::Up | KeyCode::Down => {
                let outcome = self.picker.key(key);
                self.renew_menu();
                return self.picked(outcome);
            }
            KeyCode::Enter => {
                let outcome = self.picker.key(key);
                return self.picked(outcome);
            }
            _ => {
                let step = self.menu.as_mut().map(|m| m.key(key));
                return step.map_or(Outcome::None, |s| self.menu_step(s));
            }
        }
        Outcome::None
    }

    /// A click picks a commit or snapshot, a double click reviews it, and the wheel moves
    /// through them.
    pub fn mouse(&mut self, ev: MouseEvent) -> Outcome {
        if self.in_motion() || self.busy.is_some() {
            return Outcome::None;
        }
        let outcome = match ev.kind {
            MouseEventKind::ScrollDown => {
                self.picker.scroll(true);
                picker::Outcome::None
            }
            MouseEventKind::ScrollUp => {
                self.picker.scroll(false);
                picker::Outcome::None
            }
            MouseEventKind::Down(MouseButton::Left) => self.picker.click(ev.row),
            _ => picker::Outcome::None,
        };
        self.renew_menu();
        self.picked(outcome)
    }

    /// Whether `(x, y)` is on the drawer as last drawn.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.open && self.picker.contains(x, y)
    }

    fn picked(&mut self, outcome: picker::Outcome) -> Outcome {
        match outcome {
            picker::Outcome::Open(t) => {
                self.close();
                Outcome::Open(t)
            }
            picker::Outcome::Versions(v) => Outcome::Versions(v),
            picker::Outcome::None => Outcome::None,
        }
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
        let mut hints = vec![
            ("j k", "move", false),
            ("J K", "range", false),
            ("Enter", "review", false),
        ];
        if !self.picker.git {
            hints.push(("v", "snapshots", false));
        }
        hints.push(("Esc", "close", false));
        hints
    }

    /// The drawer's place in `body`, half of it on the file list's side, and the rest.
    fn split(body: Rect) -> (Rect, Rect) {
        let (side, _, rest) = crate::tui::side::split(body, body.width / 2);
        (side, rest)
    }

    /// What the drawer leaves of `body`, for the diff.
    pub fn beside(body: Rect) -> Rect {
        Self::split(body).1
    }

    /// Draws the drawer on the file list's side of `body`, framed, with the jj menu beside
    /// it.
    pub fn draw(&mut self, c: &mut Canvas, body: Rect) {
        let (side, _) = Self::split(body);
        {
            let mut sub = Canvas::new(&mut *c.buf, side);
            self.picker.draw(&mut sub, side, true);
            self.draw_frame(&mut sub, side);
        }
        self.draw_menu(c, body, side);
    }

    fn color(&self) -> ratatui::style::Color {
        if self.busy.is_some() { MUTED } else { AMBER }
    }

    /// The frame around the log, titled Revisions, with a running action on the right.
    fn draw_frame(&self, c: &mut Canvas, r: Rect) {
        if r.width < 4 || r.height < 2 {
            return;
        }
        let line = theme::fg(self.color()).bg(BG);
        let (right, bottom) = (r.right() - 1, r.bottom() - 1);
        let across = "─".repeat(usize::from(r.width - 2));
        c.put(r.x, r.y, &format!("╭{across}╮"), line);
        c.put(r.x, bottom, &format!("╰{across}╯"), line);
        for y in r.y + 1..bottom {
            c.put(r.x, y, "│", line);
            c.put(right, y, "│", line);
        }
        let end = c.put(r.x + 2, r.y, " Revisions ", theme::bold(FG).bg(BG));
        if let Some(busy) = &self.busy {
            let w = busy.chars().count() as u16;
            let x = right.saturating_sub(w + 2).max(end + 1);
            c.put(x, r.y, &format!(" {busy} "), theme::fg(CYAN).bg(BG));
        }
    }

    /// The jj menu's box, at the bottom, just off the drawer's inner edge.
    fn draw_menu(&self, c: &mut Canvas, body: Rect, side: Rect) {
        let items = self.menu_items();
        if items.is_empty() {
            return;
        }
        let w = keybar::menu_width(&items);
        let x = match crate::tui::side::files_left() {
            true => side.right() + 1,
            false => side.x.saturating_sub(w + 1).max(body.x),
        };
        let area = Rect::new(
            x,
            body.y,
            w.min(body.right().saturating_sub(x)),
            body.height,
        );
        keybar::menu(c, area, &items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::jj::{LogCommit, LogRow};
    use ratatui::crossterm::event::KeyModifiers;

    fn press(d: &mut Drawer, keys: &str) -> Vec<Outcome> {
        crate::edit::spec::keys(keys)
            .into_iter()
            .map(|k| d.key(KeyEvent::new(k.code, KeyModifiers::NONE)))
            .collect()
    }

    fn with_log(changes: &[&str]) -> Drawer {
        let rows = changes
            .iter()
            .map(|change| LogRow {
                graph: vec!["○  ".into()],
                commit: Some(LogCommit {
                    commit_id: format!("{change}-commit"),
                    change_id: (*change).into(),
                    unique: 2,
                    name: None,
                    description: String::new(),
                    bookmarks: vec![],
                    author: String::new(),
                    time: 0.0,
                    working_copy: false,
                    immutable: false,
                    conflict: false,
                    divergent: false,
                    empty: false,
                    workspaces: vec![],
                }),
            })
            .collect();
        let mut d = Drawer::default();
        d.picker.loaded(Ok(rows));
        d
    }

    fn reviewed(outcomes: Vec<Outcome>) -> Vec<String> {
        outcomes
            .into_iter()
            .filter_map(|o| match o {
                Outcome::Open(Target::Revset(r)) => Some(r),
                _ => None,
            })
            .collect()
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
    fn it_opens_on_the_reviews_newest_change_and_enter_reviews_the_range_and_closes() {
        let mut d = with_log(&["aaa", "bbb", "ccc", "ddd"]);
        d.show(Some("bbb-commit"));
        assert!(d.is_open());
        assert_eq!(reviewed(press(&mut d, "jJ<ret>")), ["ddd::ccc"]);
        assert!(!d.is_open());
    }

    #[test]
    fn space_m_switches_mode_and_closes_without_running_a_motion() {
        let mut d = with_log(&["aaa", "bbb"]);
        d.show(None);
        press(&mut d, " ");
        assert_eq!(d.leader_menu(), [("m".to_string(), "mode")]);
        let out = press(&mut d, "me");
        assert!(matches!(
            out[..],
            [Outcome::None, Outcome::Mode(Mode::Editor)]
        ));
        assert!(!d.is_open());
        d.show(None);
        // `d` after an unfinished leader isn't Describe.
        assert!(ran(press(&mut d, " d")).is_empty());
    }

    #[test]
    fn esc_clears_a_range_before_closing() {
        let mut d = with_log(&["aaa", "bbb"]);
        d.show(None);
        press(&mut d, "J<esc>");
        assert!(d.is_open() && !d.picker.anchored());
        press(&mut d, "<esc>");
        assert!(!d.is_open());
    }

    #[test]
    fn motions_run_on_each_commit_the_cursor_moves_to_and_the_drawer_stays_open() {
        let mut d = with_log(&["aaa", "bbb"]);
        d.show(Some("aaa-commit"));
        assert_eq!(
            ran(press(&mut d, "najna")),
            [
                JjAction::NewAfter { rev: "aaa".into() },
                JjAction::NewAfter { rev: "bbb".into() },
            ]
        );
        assert!(d.is_open());
        press(&mut d, "d");
        assert!(d.typing());
        assert!(reviewed(press(&mut d, "x<ret>")).is_empty());
        assert!(d.is_open() && !d.typing());
    }

    #[test]
    fn nothing_but_esc_works_while_an_action_runs() {
        let mut d = with_log(&["aaa", "bbb"]);
        d.show(Some("aaa-commit"));
        d.busy = Some("making a change".into());
        assert!(ran(press(&mut d, "jna")).is_empty());
        assert!(reviewed(press(&mut d, "<ret>")).is_empty());
        d.busy = None;
        assert_eq!(
            ran(press(&mut d, "na")),
            [JjAction::NewAfter { rev: "aaa".into() }]
        );
        press(&mut d, "<esc>");
        assert!(!d.is_open());
    }

    #[test]
    fn a_git_log_has_no_motions_or_snapshots() {
        let mut d = with_log(&["aaa", "bbb"]);
        d.picker.git = true;
        d.show(Some("aaa-commit"));
        assert!(d.menu_items().is_empty());
        assert!(ran(press(&mut d, "na")).is_empty());
        assert!(!d.hints().iter().any(|(k, ..)| *k == "v"));
        assert_eq!(reviewed(press(&mut d, "j<ret>")), ["bbb"]);
    }

    #[test]
    fn it_takes_the_file_lists_side_with_the_menu_beside_it() {
        use ratatui::buffer::Buffer;
        let mut d = with_log(&["aaa"]);
        d.show(None);
        let area = Rect::new(0, 0, 100, 30);
        let mut buf = Buffer::empty(area);
        d.draw(&mut Canvas::new(&mut buf, area), area);
        let rest = Drawer::beside(area);
        assert_eq!((rest.x, rest.width), (0, 49));
        let row = |y: u16, from: u16| {
            (from..100)
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        };
        assert!(row(0, 50).starts_with("╭─ Revisions ─"));
        assert!(!row(0, 0).contains("jj"));
        let menu_x = 50 - 1 - keybar::menu_width(&d.menu_items());
        assert!(row(29, menu_x).starts_with('╰'));
        assert!(row(29, menu_x - 1).starts_with(' '));
    }
}
