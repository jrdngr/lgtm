//! The PR's threads from GitHub, read-only: each under the last line it's on, numbered with
//! the id the desktop app shows (#12), or at the top of its file once outdated. `p` pulls
//! them again, and `H` hides them.

use lgtm_core::review::matching::Remaps;
use lgtm_core::store::{Comment, Store};

use super::Review;
use super::rows::{NoteKey, Notes};
use crate::tui::github::Pulled;

/// A thread, and the file, hunk and row it's on. `None` for the row when its code is gone.
pub struct Thread {
    pub comment: Comment,
    pub file: usize,
    pub at: Option<(usize, usize)>,
}

/// A line of a thread: who wrote the post it starts, and the text.
pub struct ThreadLine<'a> {
    pub who: Option<&'a str>,
    pub text: &'a str,
}

impl Thread {
    pub fn outdated(&self) -> bool {
        self.at.is_none() || self.comment.github.as_ref().is_some_and(|g| g.outdated)
    }

    /// Every post's lines, the first of each with its author.
    pub fn lines(&self) -> Vec<ThreadLine<'_>> {
        let Some(g) = &self.comment.github else {
            return vec![];
        };
        let mut out = Vec::new();
        for p in &g.posts {
            let mut lines = p.body.trim().lines();
            out.push(ThreadLine {
                who: Some(&p.author),
                text: lines.next().unwrap_or(""),
            });
            out.extend(lines.map(|text| ThreadLine { who: None, text }));
        }
        out
    }
}

impl Review {
    /// Stores pulled threads, then shows the PR's.
    pub fn pulled(&mut self, store: &Store, result: anyhow::Result<Pulled>) {
        let pulled = match result {
            Ok(p) => p,
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                return;
            }
        };
        let Some(marks) = self.marks.as_ref() else {
            return;
        };
        let label = format!("PR #{}", pulled.number);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64);
        let added = lgtm_core::github::pulled::add_threads(
            store,
            &marks.placer(),
            pulled.threads,
            &pulled.head_texts,
            &label,
            now,
        );
        if let Err(e) = added {
            self.notice = Some(format!("{e:#}"));
        }
        self.pr = Some((pulled.repo, pulled.number));
        self.load_threads(store);
    }

    /// Places the PR's open threads on the review's rows.
    pub fn load_threads(&mut self, store: &Store) {
        let (Some((repo, number)), Some(marks)) = (&self.pr, &self.marks) else {
            return;
        };
        let comments = match store.pr_threads(repo, *number) {
            Ok(c) => c,
            Err(e) => {
                self.notice = Some(format!("{e:#}"));
                return;
            }
        };
        let placer = marks.placer();
        let mut remaps = Remaps::default();
        let mut threads: Vec<Thread> = comments
            .into_iter()
            .filter(|c| c.github.as_ref().is_some_and(|g| !g.resolved) && !c.path.is_empty())
            .filter_map(|c| {
                let (file, span) = placer.placement(store, &mut remaps, &c.path, &c.pins)?;
                let outdated = c.github.as_ref().is_some_and(|g| g.outdated);
                let row = match span {
                    Some(s) => Some(s.end),
                    None if !outdated => placer
                        .near_last_pin(store, &mut remaps, &c.path, &c.pins)
                        .filter(|r| r.file == file)
                        .map(|r| r.row),
                    None => None,
                };
                let at = row.and_then(|r| marks.hunk_row(file as usize, r));
                Some(Thread {
                    comment: c,
                    file: file as usize,
                    at,
                })
            })
            .collect();
        threads.sort_by_key(|t| (t.file, t.at, t.comment.id));
        self.threads = threads;
        self.relayout();
    }

    pub(super) fn thread(&self, id: i64) -> Option<&Thread> {
        self.threads.iter().find(|t| t.comment.id == id)
    }

    /// Adds the shown threads to `notes`.
    pub(super) fn thread_notes(&self, notes: &mut Notes) {
        if self.hide_pr {
            return;
        }
        for t in &self.threads {
            let key = NoteKey::Thread(t.comment.id);
            match t.at {
                Some((h, r)) if !t.outdated() => notes
                    .at_row
                    .entry((t.file, h, r))
                    .or_default()
                    .push((key, t.lines().len().max(1))),
                _ => notes.at_file.entry(t.file).or_default().push((key, 1)),
            }
        }
    }

    pub(super) fn toggle_pr(&mut self) {
        self.hide_pr = !self.hide_pr;
        self.relayout();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::load::Target;
    use lgtm_core::anchor::{MatchConfig, Side};
    use lgtm_core::diff::{FileStatus, Hunk, Row, RowKind};
    use lgtm_core::github::pulled::PulledThread;
    use lgtm_core::jj::FileDiff;
    use lgtm_core::review::file::{ReviewFile, Sides, Text};
    use lgtm_core::store::{GhLink, GhPost};

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>, text: &str) -> Row {
        Row {
            kind,
            old_line: old,
            new_line: new,
            text: text.into(),
        }
    }

    /// `a.rs`, whose line 10 changed.
    fn review(store: &Store) -> Review {
        let diff = FileDiff {
            path: "a.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks: vec![Hunk {
                old_start: 9,
                old_lines: 2,
                new_start: 9,
                new_lines: 2,
                section: String::new(),
                rows: vec![
                    row(RowKind::Context, Some(9), Some(9), "line 9"),
                    row(RowKind::Removed, Some(10), None, "line 10"),
                    row(RowKind::Added, None, Some(10), "line 10 new"),
                ],
            }],
        };
        let old: String = (1..=12).map(|i| format!("line {i}\n")).collect();
        let new = old.replace("line 10\n", "line 10 new\n");
        let file = ReviewFile::build(
            &Sides {
                old: Some("aaaa".into()),
                new: "bbbb".into(),
            },
            &diff,
            Text {
                text: Some(&old),
                parsed: None,
            },
            Text {
                text: Some(&new),
                parsed: None,
            },
            MatchConfig::default(),
            false,
        );
        let mut r = Review::new(1, Target::Revset("@".into()));
        r.loaded(Ok((vec![], vec![diff])));
        r.built(vec![file], MatchConfig::default(), Some(store));
        r
    }

    fn thread(id: &str, line: Option<u32>, outdated: bool) -> PulledThread {
        PulledThread {
            link: GhLink {
                repo: "o/n".into(),
                pr: 7,
                thread: id.into(),
                url: String::new(),
                resolved: false,
                outdated,
                posts: vec![GhPost {
                    author: "octo".into(),
                    body: "why?\nreally".into(),
                    created_at: String::new(),
                    pending: false,
                }],
                line,
            },
            path: "a.rs".into(),
            side: Side::New,
            start: None,
            line,
            hunk: String::new(),
            answered: false,
        }
    }

    #[test]
    fn pulled_threads_go_under_their_line_or_atop_their_file_and_hide_with_h() {
        let store = Store::in_memory();
        let mut r = review(&store);
        let pulled = Pulled {
            repo: "o/n".into(),
            number: 7,
            threads: vec![thread("T1", Some(10), false), thread("T2", None, true)],
            head_texts: Default::default(),
        };
        r.pulled(&store, Ok(pulled));
        assert_eq!(r.threads.len(), 2);
        let notes = r.notes();
        let under: Vec<_> = notes.at_row.get(&(0, 0, 2)).cloned().unwrap_or_default();
        assert_eq!(under.len(), 1);
        // The thread's two lines of body.
        assert_eq!(under[0].1, 2);
        assert_eq!(notes.at_file.get(&0).map(Vec::len), Some(1));
        // Pulling again keeps each thread's id.
        let ids: Vec<i64> = r.threads.iter().map(|t| t.comment.id).collect();
        let again = Pulled {
            repo: "o/n".into(),
            number: 7,
            threads: vec![thread("T1", Some(10), false), thread("T2", None, true)],
            head_texts: Default::default(),
        };
        r.pulled(&store, Ok(again));
        let mut after: Vec<i64> = r.threads.iter().map(|t| t.comment.id).collect();
        after.sort();
        let mut before = ids;
        before.sort();
        assert_eq!(after, before);
        r.toggle_pr();
        let notes = r.notes();
        assert!(notes.at_row.is_empty() && notes.at_file.is_empty());
    }
}
