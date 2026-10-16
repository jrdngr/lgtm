//! GitHub review threads on a review's rows: placing pulled threads, and where a local comment
//! goes on the PR's diff when it's published. GitHub's lines are the PR head's, and the review's
//! new side can differ from it by edits in the working copy, so lines are remapped between them.

use std::collections::HashMap;

use anyhow::{Context, bail};

use super::{RepoCtx, Review, RowRef, StateDto, now_ms, pulled_stage};
use crate::anchor::{FileIndex, Side, nearest_lines, norm_for, remap_lines};
use crate::diff::RowKind;
use crate::store::{Comment, Conversation, GhLink, ThreadState};

/// A thread pulled from GitHub, to become a comment block.
pub struct PulledThread {
    pub link: GhLink,
    pub path: String,
    pub side: Side,
    pub start: Option<u32>,
    /// `None` once outdated.
    pub line: Option<u32>,
    /// GitHub's diff hunk, the snippet when the code can't be found.
    pub hunk: String,
    /// The viewer had the last word.
    pub answered: bool,
}

/// Where a comment goes on the PR head's diff.
#[derive(Debug, PartialEq, Eq)]
pub struct Spot {
    pub path: String,
    pub side: Side,
    pub line: u32,
    pub start: Option<(Side, u32)>,
}

/// Maps 1-based lines of `from` to lines of `to`.
fn line_map(from: &[i64], to: &[i64]) -> HashMap<u32, u32> {
    remap_lines(from, to)
}

fn hashes(text: &str, path: &str) -> Vec<i64> {
    FileIndex::new(text, norm_for(path)).hashes
}

impl Review {
    /// The changed rows of `path` with `side` lines in `start..=end`, or the nearest changed
    /// row when there are none.
    pub(super) fn rows_on_side(
        &self,
        path: &str,
        side: Side,
        start: u32,
        end: u32,
    ) -> Option<Vec<RowRef>> {
        let fi = self.files.iter().position(|f| f.path == path)?;
        let rows = &self.files[fi].rows;
        let line = |i: usize| match side {
            Side::Old => rows[i].old_line,
            Side::New => rows[i].new_line,
        };
        let changed = |i: &usize| rows[*i].kind != RowKind::Context;
        let inside: Vec<usize> = (0..rows.len())
            .filter(|&i| line(i).is_some_and(|l| start <= l && l <= end))
            .collect();
        let mut picked: Vec<usize> = match (inside.first(), inside.last()) {
            (Some(&lo), Some(&hi)) => (lo..=hi).filter(changed).collect(),
            _ => vec![],
        };
        if picked.is_empty() {
            picked.extend(
                (0..rows.len())
                    .filter(changed)
                    .min_by_key(|&i| line(i).map_or(u32::MAX, |l| l.abs_diff(start))),
            );
        }
        (!picked.is_empty()).then(|| {
            picked
                .into_iter()
                .map(|r| RowRef::new(fi, r as u32))
                .collect()
        })
    }

    /// The review's nearest new-side lines for `head` lines of `path`, when `head` is known
    /// and the review shows another commit.
    fn head_to_review(&self, path: &str, head: Option<&str>) -> Option<HashMap<u32, u32>> {
        let f = self.files.iter().find(|f| f.path == path)?;
        let ours = f.new.index.as_ref()?;
        Some(nearest_lines(&hashes(head?, path), &ours.hashes))
    }

    /// Rows a thread covers, or `None` when it's outdated or its file isn't in the review.
    fn thread_rows(&self, t: &PulledThread, head_text: Option<&str>) -> Option<Vec<RowRef>> {
        let line = t.line?;
        let start = t.start.unwrap_or(line).min(line);
        // A line `@` edited or deleted goes where its replacement is.
        let (start, line) = match (t.side, self.head_to_review(&t.path, head_text)) {
            (Side::New, Some(map)) => (*map.get(&start)?, *map.get(&line)?),
            _ => (start, line),
        };
        self.rows_on_side(&t.path, t.side, start, line)
    }

    /// Where the comment's first and last changed rows are on the PR head. `head_text` is its
    /// file at the head.
    fn spot(&self, comment: u32, head_text: Option<&str>) -> anyhow::Result<Spot> {
        let c = self
            .comments
            .iter()
            .find(|c| c.id == comment)
            .context("the comment isn't in this review")?;
        let span = c
            .rows
            .context("the code this comment was on is gone, so it can't go on GitHub")?;
        let f = &self.files[c.file as usize];
        let changed: Vec<usize> = (span.start..=span.end)
            .map(|r| r as usize)
            .filter(|&r| f.rows.get(r).is_some_and(|p| p.kind != RowKind::Context))
            .collect();
        let back = match (head_text, f.new.index.as_ref()) {
            (Some(text), Some(ours)) => Some(line_map(&ours.hashes, &hashes(text, &f.path))),
            _ => None,
        };
        let at = |r: usize| -> anyhow::Result<(Side, u32)> {
            let p = &f.rows[r];
            if p.kind == RowKind::Removed {
                return Ok((Side::Old, p.old_line.context("removed row without a line")?));
            }
            let line = p.new_line.context("added row without a line")?;
            match &back {
                Some(map) => map.get(&line).map(|l| (Side::New, *l)).with_context(|| {
                    format!(
                        "line {line} of {} isn't in the PR's head, so it can't go on GitHub",
                        f.path
                    )
                }),
                None => Ok((Side::New, line)),
            }
        };
        let (Some(&first), Some(&last)) = (changed.first(), changed.last()) else {
            bail!("the comment covers no changed rows");
        };
        let (side, line) = at(last)?;
        let start = (first != last).then(|| at(first)).transpose()?;
        Ok(Spot {
            path: f.path.clone(),
            side,
            line,
            start,
        })
    }
}

impl RepoCtx {
    /// Adds pulled threads as comments, and updates the ones already pulled. `head_texts` are
    /// the threads' files at the PR head.
    pub fn add_threads(
        &self,
        id: u32,
        threads: Vec<PulledThread>,
        head_texts: &HashMap<String, String>,
        label: &str,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |review, store| {
            // Each known thread's comment.
            let mut known: HashMap<String, Comment> = HashMap::new();
            if let Some(first) = threads.first() {
                for c in store.pr_threads(&first.link.repo, first.link.pr)? {
                    if let Some(g) = &c.github {
                        known.insert(g.thread.clone(), c);
                    }
                }
            }
            let now = now_ms();
            for t in threads {
                let rows = review.thread_rows(&t, head_texts.get(&t.path).map(String::as_str));
                if let Some(c) = known.get(&t.link.thread) {
                    let (Some(old), Some(link)) = (&c.thread, &c.github) else {
                        continue;
                    };
                    let mut state = old.clone();
                    state.stage = pulled_stage(Some((old, link)), &t.link, t.answered);
                    // A reply posted from elsewhere, like a terminal agent, isn't left to send.
                    let sent = t.link.posts.last().is_some_and(|p| {
                        t.answered && !c.body.trim().is_empty() && p.body.trim() == c.body.trim()
                    });
                    if sent {
                        store.set_comment_body(c.id, "", now)?;
                    }
                    store.set_thread(c.id, &Some(state))?;
                    store.set_github(c.id, &Some(t.link))?;
                    // A thread pulled while its code wasn't in the review gets placed now.
                    if let (true, false, Some(rows)) = (c.pins.is_empty(), t.path.is_empty(), &rows)
                    {
                        let (path, pins) = review.pins(store, rows)?;
                        store.set_comment_pins(c.id, &path, &pins, &review.snippet(rows))?;
                    }
                    continue;
                }
                let (path, pins) = match &rows {
                    Some(rows) => review.pins(store, rows)?,
                    None => (t.path.clone(), vec![]),
                };
                let snippet = rows.map_or(t.hunk.clone(), |r| review.snippet(&r));
                let stage = pulled_stage(None, &t.link, t.answered);
                let thread = ThreadState::new(&t.link.repo, t.link.pr, stage);
                store.add_comment(&Comment {
                    id: 0,
                    path,
                    pins,
                    body: String::new(),
                    snippet,
                    review: label.to_string(),
                    created_at: now,
                    updated_at: now,
                    conversation: Conversation::default(),
                    github: Some(t.link),
                    thread: Some(thread),
                    done: false,
                })?;
            }
            Ok(())
        })
    }

    /// Where a comment goes on the PR head's diff. `head_text` is its file at the head.
    pub fn comment_spot(
        &self,
        id: u32,
        comment: u32,
        head_text: Option<&str>,
    ) -> anyhow::Result<Spot> {
        self.review(id)?.lock().unwrap().spot(comment, head_text)
    }
}
