//! Comments on a review's rows: pinning rows so a comment can find them again, finding the
//! rows a comment's pins are on now, and the rows a GitHub thread's lines cover.

use std::collections::{HashMap, HashSet};

use anyhow::Context;
use serde::Serialize;

use super::RowRef;
use super::file::ReviewFile;
use super::matching::{Remaps, SideRows, pools, removed_paths};
use super::snippet::unified;
use crate::anchor::{self, FileIndex, MarkRef, MatchConfig, Side, nearest_lines, norm_for};
use crate::diff::RowKind;
use crate::store::{Pin, Store};

/// The first and last rows a comment covers, by index among its file's rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RowSpan {
    pub start: u32,
    pub end: u32,
}

/// Where a GitHub thread is on the PR head: its file, side, and lines.
pub struct ThreadLines<'a> {
    pub path: &'a str,
    pub side: Side,
    pub start: Option<u32>,
    /// `None` once outdated.
    pub line: Option<u32>,
}

pub fn hashes(text: &str, path: &str) -> Vec<i64> {
    FileIndex::new(text, norm_for(path)).hashes
}

/// A review's files, for placing comments on them.
pub struct Placer<'a> {
    pub files: &'a [ReviewFile],
    pub cfg: MatchConfig,
}

impl Placer<'_> {
    /// The file and rows that pins on `path` match, or `None` when none of the review's files
    /// is that file. The rows are absent when the code can't be found any more.
    pub fn placement(
        &self,
        store: &Store,
        remaps: &mut Remaps,
        path: &str,
        pins: &[Pin],
    ) -> Option<(u32, Option<RowSpan>)> {
        let found: Vec<RowRef> = pins
            .iter()
            .filter_map(|p| self.locate(store, remaps, path, p))
            .collect();
        let end = found.last().copied();
        let rows = end.map(|end| {
            let same: Vec<u32> = found
                .iter()
                .filter(|r| r.file == end.file)
                .map(|r| r.row)
                .collect();
            let (lo, hi) = (*same.iter().min().unwrap(), *same.iter().max().unwrap());
            (end.file, RowSpan { start: lo, end: hi })
        });
        let file = match rows {
            Some((f, _)) => f,
            None => self
                .files
                .iter()
                .position(|f| f.path == path || f.old_path.as_deref() == Some(path))?
                as u32,
        };
        Some((file, rows.map(|(_, span)| span)))
    }

    /// The changed row a pin matches best, trying the files whose path pools hold its path:
    /// its own, or else `path`, its comment's. A pin whose line was edited lands on a line that
    /// replaced it and shares some of its context.
    fn locate(&self, store: &Store, remaps: &mut Remaps, path: &str, pin: &Pin) -> Option<RowRef> {
        let path = pin.path.as_deref().unwrap_or(path);
        let anchor = pin.anchor(path);
        let sides = self.pin_sides(path, pin);
        for rows in &sides {
            let mark = MarkRef {
                id: 0,
                anchor: &anchor,
                created_at: 0,
                remap: remaps.row(store, &pin.commit, path, pin.line, rows),
            };
            if let Some((&row, _)) = anchor::match_marks(&[mark], &rows.candidates, self.cfg)
                .iter()
                .next()
            {
                return Some(RowRef::new(rows.file_idx, row as u32));
            }
        }
        // The line was edited: take the line that replaced it whose context fits best, if any
        // of its context is still there.
        sides.iter().find_map(|rows| {
            let edited = remaps.edited(store, &pin.commit, path, pin.line, rows);
            let (score, best) = edited
                .iter()
                .rev()
                .map(|c| (anchor::score(&anchor, c), c))
                .max_by_key(|(s, _)| *s)?;
            (score > 0).then(|| RowRef::new(rows.file_idx, best.id as u32))
        })
    }

    /// The changed rows of each file side a pin on `path` may be on.
    fn pin_sides(&self, path: &str, pin: &Pin) -> Vec<SideRows<'_>> {
        let removed = removed_paths(self.files);
        self.files
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                pools(f, pin.side, &removed)
                    .iter()
                    .any(|p| p.contains(&path))
            })
            .filter_map(|(i, f)| SideRows::new(i, f, pin.side))
            .collect()
    }

    /// The changed row nearest where the last pin's line is now, for a GitHub thread whose code
    /// can't be found: it stays on a line near the one GitHub shows it on.
    pub fn near_last_pin(
        &self,
        store: &Store,
        remaps: &mut Remaps,
        path: &str,
        pins: &[Pin],
    ) -> Option<RowRef> {
        let pin = pins.last()?;
        let path = pin.path.as_deref().unwrap_or(path);
        self.pin_sides(path, pin).iter().find_map(|rows| {
            let row = remaps.nearest(store, &pin.commit, path, pin.line, rows)?;
            Some(RowRef::new(rows.file_idx, row as u32))
        })
    }

    /// Pins for the first and last of `rows` that can be anchored, all in the last one's file.
    pub fn pins(&self, store: &Store, rows: &[RowRef]) -> anyhow::Result<(String, Vec<Pin>)> {
        self.try_pins(store, rows)?
            .context("these rows can't hold a comment")
    }

    /// Like [`Self::pins`], but `None` when none of `rows` can be anchored, as removed rows
    /// of an interdiff can't.
    pub fn try_pins(
        &self,
        store: &Store,
        rows: &[RowRef],
    ) -> anyhow::Result<Option<(String, Vec<Pin>)>> {
        let wanted: HashSet<RowRef> = rows.iter().copied().collect();
        let file_idx = rows.last().context("nothing to comment on")?.file;
        let file = self.files.get(file_idx as usize).context("no such file")?;
        let mut pins = Vec::new();
        for k in &file.keyed {
            if !wanted.contains(&RowRef {
                file: file_idx,
                row: k.row,
            }) {
                continue;
            }
            let Some(commit) = file.side(k.side).anchor_commit(store)? else {
                continue;
            };
            let path = file.side(k.side).path.clone();
            pins.push(Pin::new(&k.anchor, commit, k.line, Some(path)));
        }
        if pins.len() > 2 {
            pins.drain(1..pins.len() - 1);
        }
        let Some(last) = pins.last() else {
            return Ok(None);
        };
        let path = last.path.clone().unwrap_or_default();
        Ok(Some((path, pins)))
    }

    /// The code of `rows` and the context between them, as a unified diff with a `@@` header
    /// wherever the line numbers jump.
    pub fn snippet(&self, rows: &[RowRef]) -> String {
        let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
            return String::new();
        };
        let f = &self.files[last.file as usize];
        let start = if first.file == last.file {
            first.row
        } else {
            last.row
        };
        let rows = (start..=last.row).map(|r| (f.rows[r as usize], f.texts[r as usize].as_str()));
        unified(rows)
    }

    /// The changed rows of `path` with `side` lines in `start..=end`, or the nearest changed
    /// row when there are none.
    pub fn rows_on_side(
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
    /// `head_text` is its file at the PR head.
    pub fn thread_rows(&self, t: &ThreadLines, head_text: Option<&str>) -> Option<Vec<RowRef>> {
        let line = t.line?;
        let start = t.start.unwrap_or(line).min(line);
        // A line `@` edited or deleted goes where its replacement is.
        let (start, line) = match (t.side, self.head_to_review(t.path, head_text)) {
            (Side::New, Some(map)) => (*map.get(&start)?, *map.get(&line)?),
            _ => (start, line),
        };
        self.rows_on_side(t.path, t.side, start, line)
    }
}
