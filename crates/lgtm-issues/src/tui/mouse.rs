//! Clicks on the list: one selects, a second on the same row opens it. A question or a
//! folded group acts on the first click.

use std::time::{Duration, Instant};

use ratatui::crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::list::Row;
use super::{Issues, Screen};
use crate::issue::Status;

/// Two clicks on a row within this make a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

impl Issues {
    pub fn mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::ScrollDown => self.scroll(3),
            MouseEventKind::ScrollUp => self.scroll(-3),
            MouseEventKind::Down(MouseButton::Left) if self.prompt.is_none() => {
                self.click_at(ev.row, Instant::now())
            }
            _ => {}
        }
    }

    fn click_at(&mut self, y: u16, now: Instant) {
        self.space = false;
        self.status_for = None;
        let area = self.list_area;
        if !matches!(self.screen, Screen::List) || y < area.y || y >= area.bottom() {
            return;
        }
        let n = self.top + (y - area.y) as usize;
        let Some(row) = self.rows().get(n).cloned().filter(Row::pickable) else {
            return;
        };
        let double = self
            .click
            .take()
            .is_some_and(|(was, when)| was == n && now - when < DOUBLE_CLICK);
        self.sel = n;
        match row {
            Row::Question { id, qid } => self.screen = Screen::Question { id, qid, top: 0 },
            Row::Folded(status, ..) => self.toggle(status),
            Row::Issue(id) if double => self.open(id),
            _ => self.click = Some((n, now)),
        }
    }

    /// Lists done or dropped issues under their count, or folds them back.
    pub(super) fn toggle(&mut self, status: Status) {
        if let Some(p) = self.expanded.iter().position(|s| *s == status) {
            self.expanded.remove(p);
        } else {
            self.expanded.push(status);
        }
        self.clamp();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::folder::Folder;
    use crate::issue::Issue;
    use ratatui::layout::Rect;

    fn screen() -> (tempfile::TempDir, Issues) {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        for (title, status) in [("Open one", Status::Open), ("Done one", Status::Done)] {
            let mut i = Issue::new(0, title, None, "", 0);
            i.status = status;
            folder.create(i).unwrap();
        }
        let mut issues = Issues::new(folder, "jordan".into());
        issues.list_area = Rect::new(0, 2, 80, 20);
        (dir, issues)
    }

    #[test]
    fn a_click_selects_a_second_opens_and_the_done_group_toggles() {
        let (_dir, mut s) = screen();
        let t = Instant::now();
        // Rows: open header, issue 1, gap, done (folded).
        s.click_at(3, t);
        assert_eq!((s.sel, matches!(s.screen, Screen::List)), (1, true));
        s.click_at(5, t);
        assert_eq!(s.rows()[4], Row::Issue(2));
        s.click_at(5, t);
        assert_eq!(s.rows().len(), 4);
        s.click_at(3, t);
        s.click_at(3, t + DOUBLE_CLICK * 2);
        assert!(matches!(s.screen, Screen::List));
        s.click_at(3, t + DOUBLE_CLICK * 2 + Duration::from_millis(100));
        assert!(matches!(s.screen, Screen::View(_)));
    }
}
