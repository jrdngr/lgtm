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
use super::theme::{self, ACCENT, BG, FG, HAIR, MUTED};
use lgtm_core::jj::{JjAction, PushPlan};

pub enum Outcome {
    None,
    Open(Target),
    Versions(Vec<String>),
    Edit(String),
    /// Run a jj menu motion.
    Jj(JjAction),
    /// Load the bookmarks nearest this commit, for the jj menu.
    Bookmarks(String),
    /// Find what a push would do, for the jj menu.
    PushPlan,
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
    /// The jj menu, while it's open.
    menu: Option<Menu>,
}

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
            menu: None,
        }
    }
}

/// The keys of the revisions side, for `?`.
pub const KEYS: &[(&str, &str)] = &[
    ("Tab", "files"),
    ("j k", "move"),
    ("g G", "first, last"),
    ("J K", "range"),
    ("Esc", "clear range"),
    ("Space", "jj menu"),
    ("Enter", "open"),
    ("s", "snapshots"),
    ("a", "every snapshot"),
    (":", "command"),
    ("q", "quit"),
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

    /// Takes a jj menu step.
    fn menu_step(&mut self, step: motions::Step) -> Outcome {
        match step {
            motions::Step::Stay => Outcome::None,
            motions::Step::Close => {
                self.menu = None;
                Outcome::None
            }
            motions::Step::Run(action) => {
                self.menu = None;
                Outcome::Jj(action)
            }
            motions::Step::Bookmarks(rev) => Outcome::Bookmarks(rev),
            motions::Step::PushPlan => Outcome::PushPlan,
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
                self.menu = None;
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
                self.menu = None;
                Outcome::None
            }
            Ok(names) => {
                let step = menu.bookmarks(names);
                self.menu_step(step)
            }
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                self.menu = None;
                Outcome::None
            }
        }
    }

    /// A click in the file tree opens a file or a folder, and the wheel scrolls it.
    pub fn mouse(&mut self, ev: MouseEvent) -> Outcome {
        if !self.files.contains(ev.column, ev.row) {
            return Outcome::None;
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

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        self.notice = None;
        if let Some(menu) = self.menu.as_mut() {
            let step = menu.key(key);
            return self.menu_step(step);
        }
        if self.cmd.is_some() {
            return self.command_key(key);
        }
        if key.code == KeyCode::Char(' ') && self.focus == Focus::Revisions {
            self.menu = self.picker.motion_target().map(Menu::new);
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
            Focus::Revisions => match self.picker.key(key) {
                picker::Outcome::Open(t) => Outcome::Open(t),
                picker::Outcome::Versions(v) => Outcome::Versions(v),
                picker::Outcome::Quit => Outcome::Quit,
                picker::Outcome::None => Outcome::None,
            },
        }
    }

    pub fn hints(&self) -> Vec<Hint<'static>> {
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
        let left_w = body.width / 2;
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
        }
        for y in body.y..body.bottom() {
            c.put(body.x + left_w, y, "│", theme::fg(HAIR).bg(BG));
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
}
