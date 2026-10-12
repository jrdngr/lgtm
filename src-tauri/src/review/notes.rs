//! Walkthrough notes on a review: placing them on rows, and saving a new walkthrough from
//! line ranges.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::comments::RowSpan;
use super::matching::Remaps;
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::diff::RowKind;
use crate::store::{Category, FlowStep, Level, Note, Source, Store};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: u32,
    pub path: String,
    pub file: u32,
    /// Absent when its code can't be found any more.
    pub rows: Option<RowSpan>,
    pub category: Category,
    /// Only for important notes.
    pub level: Option<Level>,
    pub title: String,
    /// Markdown.
    pub body: String,
    pub flow: Option<FlowStep>,
    /// Its place in the reading order, from 0.
    pub position: u32,
    pub source: Source,
}

/// A note as an agent reports it, in reading order: a path and the first and last lines on
/// the new side, or the old side for a removed file.
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct NoteInput {
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub category: Category,
    #[serde(default)]
    pub level: Option<Level>,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub flow: Option<FlowStep>,
}

impl Review {
    pub(super) fn rematch_notes(&mut self, store: &Store) -> anyhow::Result<()> {
        let mut remaps = Remaps::default();
        let mut placed: Vec<NoteDto> = store
            .notes(&self.label)?
            .into_iter()
            .filter_map(|n| {
                let (file, rows) = self.placement(store, &mut remaps, &n.path, &n.pins)?;
                Some(NoteDto {
                    id: n.id as u32,
                    path: n.path,
                    file,
                    rows,
                    category: n.category,
                    level: n.level,
                    title: n.title,
                    body: n.body,
                    flow: n.flow,
                    position: n.position,
                    source: n.source,
                })
            })
            .collect();
        placed.sort_by_key(|n| n.position);
        self.notes = placed;
        Ok(())
    }

    /// The changed rows a line range covers: every changed row from the first to the last
    /// row inside it, or the nearest changed row when none is.
    pub(super) fn rows_for(&self, path: &str, start: u32, end: u32) -> Option<Vec<RowRef>> {
        let fi = self.files.iter().position(|f| f.path == path)?;
        let rows = &self.files[fi].rows;
        let line = |i: usize| rows[i].new_line.or(rows[i].old_line);
        let inside: Vec<usize> = (0..rows.len())
            .filter(|&i| line(i).is_some_and(|l| start <= l && l <= end))
            .collect();
        let changed = |i: &usize| rows[*i].kind != RowKind::Context;
        let picked: Vec<usize> = match (inside.first(), inside.last()) {
            (Some(&lo), Some(&hi)) => (lo..=hi).filter(changed).collect(),
            _ => vec![],
        };
        let picked = if picked.is_empty() {
            (0..rows.len())
                .filter(changed)
                .min_by_key(|&i| line(i).map_or(u32::MAX, |l| l.abs_diff(start)))
                .into_iter()
                .collect()
        } else {
            picked
        };
        (!picked.is_empty()).then(|| {
            picked
                .into_iter()
                .map(|r| RowRef::new(fi, r as u32))
                .collect()
        })
    }

    /// Anchors reported notes, keeping their order. Ones outside the review's files are left
    /// out. An important note without a level counts as medium, and a level on any other
    /// note is dropped.
    fn anchor_notes(
        &self,
        store: &Store,
        inputs: &[NoteInput],
        source: Source,
    ) -> anyhow::Result<Vec<Note>> {
        let now = now_ms();
        let mut out = Vec::new();
        for n in inputs {
            let Some(rows) = self.rows_for(&n.path, n.start.min(n.end), n.start.max(n.end)) else {
                continue;
            };
            let (path, pins) = self.pins(store, &rows)?;
            let level = match n.category {
                Category::Important => Some(n.level.unwrap_or(Level::Medium)),
                _ => None,
            };
            out.push(Note {
                id: 0,
                path,
                pins,
                category: n.category,
                level,
                title: n.title.clone(),
                body: n.body.clone(),
                flow: n.flow.clone(),
                position: out.len() as u32,
                review: self.label.clone(),
                source,
                created_at: now,
            });
        }
        Ok(out)
    }
}

impl RepoCtx {
    /// Replaces the review's notes with `inputs`. Returns the new state and how many notes
    /// were left out for being outside the review's files.
    pub fn save_notes(
        &self,
        id: u32,
        inputs: &[NoteInput],
        source: Source,
    ) -> anyhow::Result<(StateDto, usize)> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let notes = review.anchor_notes(&store, inputs, source)?;
        store.replace_notes(&review.label, &notes)?;
        review.rematch_notes(&store)?;
        Ok((review.state(), inputs.len() - notes.len()))
    }
}
