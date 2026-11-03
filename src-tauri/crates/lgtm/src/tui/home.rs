//! The screen `lgtm` opens on: the jj log on the left to pick a review from, and the repo's
//! files on the right to open in the editor. Tab moves between them.

use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;

use super::files::{self, FileTree};
use super::keybar::Hint;
use super::load::Target;
use super::paint::Canvas;
use super::picker::{self, Picker};
use super::theme::{self, ACCENT, BG, FG, HAIR, MUTED};

pub enum Outcome {
    None,
    Open(Target),
    Versions(Vec<String>),
    Edit(String),
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
}

impl Default for Home {
    fn default() -> Self {
        Self {
            picker: Picker::default(),
            files: FileTree::default(),
            focus: Focus::Revisions,
        }
    }
}

/// The keys of the revisions side, for `?`.
pub const KEYS: &[(&str, &str)] = &[
    ("Tab", "files"),
    ("j k", "move"),
    ("g G", "first, last"),
    ("Space", "range"),
    ("Esc", "clear range"),
    ("Enter", "open"),
    ("s", "snapshots"),
    ("a", "every snapshot"),
    ("q", "quit"),
];

impl Home {
    /// Whether keys are going into the search box.
    pub fn typing(&self) -> bool {
        self.focus == Focus::Files
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
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
                hints.push(("C-c", "quit", false));
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
            sub.put(left.x + 1, left.y, "Revisions", theme::bold(FG).bg(BG));
        }
        for y in body.y..body.bottom() {
            c.put(body.x + left_w, y, "│", theme::fg(HAIR).bg(BG));
        }
        let mut sub = Canvas::new(&mut *c.buf, right);
        let x0 = right.x + 1;
        sub.put(x0, right.y, "Files", theme::bold(FG).bg(BG));
        if right.height < 4 {
            return;
        }
        let x = sub.put(x0, right.y + 1, "› ", theme::bold(ACCENT).bg(BG));
        let x = sub.put(x, right.y + 1, &self.files.query, theme::fg(FG).bg(BG));
        let focused = self.focus == Focus::Files;
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
