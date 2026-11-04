//! `C` resolves the comment under the cursor, or opens it again. A comment resolved while
//! resolved comments are hidden stays until the cursor leaves it.

use lgtm_core::store::{AgentComment, Store};

use super::Review;
use super::rows::{Line, NoteKey};
use super::undo::Step;

impl Review {
    /// Whether a comment shows in the diff.
    pub(super) fn shown(&self, c: &AgentComment) -> bool {
        !c.done || self.show_resolved || self.lingering == Some(c.number)
    }

    /// The comment the cursor is on.
    pub(super) fn comment_at_cursor(&self) -> Option<i64> {
        match self.current()? {
            Line::Note {
                key: NoteKey::Comment(n),
                ..
            } => Some(n),
            _ => None,
        }
    }

    /// Resolves the cursor's comment, or opens it again.
    pub(super) fn toggle_resolved(&mut self, store: Option<&Store>) {
        let (Some(number), Some(store)) = (self.comment_at_cursor(), store) else {
            return;
        };
        let Some(done) = self.comment(number).map(|c| !c.done) else {
            return;
        };
        match store.set_agent_comment_done(number, done) {
            Ok(_) => self.history.did(Step::Resolved { number, done }),
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
        self.lingering = done.then_some(number);
        let part = match self.current() {
            Some(Line::Note { part, .. }) => part,
            _ => 0,
        };
        self.load_comments(store);
        // The layout put the cursor on the comment's row; it stays on the comment.
        let on = |l: &Line| matches!(*l, Line::Note { key: NoteKey::Comment(n), part: p, .. } if n == number && p == part);
        if let Some(i) = self.lines.iter().position(on) {
            self.cursor = i;
        }
    }

    /// Hides a resolved comment once the cursor is off it.
    pub(super) fn let_go(&mut self) {
        if self.lingering.is_some() && self.comment_at_cursor() != self.lingering {
            self.lingering = None;
            self.relayout();
        }
    }
}

#[cfg(test)]
mod tests {
    use lgtm_core::anchor::Side;
    use ratatui::crossterm::event::KeyCode;

    use super::super::tests::{press, review};
    use super::*;
    use crate::agent::{Request, ResolveComment};

    #[test]
    fn c_resolves_a_comment_which_stays_until_the_cursor_leaves_and_u_undoes_it() {
        let mut store = Store::in_memory();
        let mut r = review();
        store.add_agent_comment(&comment("why?")).unwrap();
        r.load_comments(&store);
        let on = |r: &Review| {
            r.lines.iter().position(|l| {
                matches!(
                    l,
                    Line::Note {
                        key: NoteKey::Comment(1),
                        ..
                    }
                )
            })
        };
        r.cursor = on(&r).unwrap();
        r.key(key('C'), Some(&mut store));
        assert!(r.comment(1).unwrap().done);
        assert_eq!(r.comment_at_cursor(), Some(1));
        assert_eq!(r.open_comments(0), 0);
        assert_eq!(r.resolved_comments(0), 0);
        r.show_resolved = true;
        assert_eq!(r.resolved_comments(0), 1);
        r.show_resolved = false;
        press(&mut r, KeyCode::Char('k'));
        assert_eq!(on(&r), None);
        r.key(key('u'), Some(&mut store));
        assert!(!r.comment(1).unwrap().done);
        assert!(on(&r).is_some());
    }

    #[test]
    fn agents_see_resolved_comments_last_and_can_reopen_them() {
        let mut store = Store::in_memory();
        let mut r = review();
        for body in ["one", "two"] {
            store.add_agent_comment(&comment(body)).unwrap();
        }
        r.load_comments(&store);
        r.answer(
            Request::ResolveComment(ResolveComment { number: 1 }),
            Some(&mut store),
        )
        .unwrap();
        let listed: serde_json::Value =
            serde_json::from_str(&r.answer(Request::GetComments, None).unwrap()).unwrap();
        assert_eq!(listed[0]["number"], "C2");
        assert_eq!(listed[1]["number"], "C1");
        assert_eq!(listed[1]["resolved"], true);
        r.answer(
            Request::UnresolveComment(ResolveComment { number: 1 }),
            Some(&mut store),
        )
        .unwrap();
        assert!(!r.comment(1).unwrap().done);
    }

    fn comment(body: &str) -> AgentComment {
        AgentComment {
            id: 0,
            number: 0,
            path: "a.rs".into(),
            side: Side::Old,
            line: 1,
            line_text: String::new(),
            start: None,
            body: body.into(),
            review: "@".into(),
            created_at: 0,
            done: false,
        }
    }

    fn key(c: char) -> ratatui::crossterm::event::KeyEvent {
        ratatui::crossterm::event::KeyEvent::from(KeyCode::Char(c))
    }
}
