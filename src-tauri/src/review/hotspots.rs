//! Hotspots on a review: placing them on rows, and saving a new pass from line ranges.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::comments::RowSpan;
use super::matching::Remaps;
use super::{RepoCtx, Review, RowRef, StateDto, now_ms};
use crate::diff::RowKind;
use crate::store::{Hotspot, Level, Source, Store};

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HotspotDto {
    pub id: u32,
    pub path: String,
    pub file: u32,
    /// Absent when its code can't be found any more.
    pub rows: Option<RowSpan>,
    pub level: Level,
    pub reason: String,
    pub source: Source,
}

/// A hotspot as an agent reports it: a path and the first and last lines on the new side, or
/// the old side for a removed file.
#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub struct HotspotInput {
    pub path: String,
    pub start: u32,
    pub end: u32,
    pub level: Level,
    pub reason: String,
}

impl Review {
    pub(super) fn rematch_hotspots(&mut self, store: &Store) -> anyhow::Result<()> {
        let mut remaps = Remaps::default();
        let mut placed: Vec<HotspotDto> = store
            .hotspots(&self.label)?
            .into_iter()
            .filter_map(|h| {
                let (file, rows) = self.placement(store, &mut remaps, &h.path, &h.pins)?;
                Some(HotspotDto {
                    id: h.id as u32,
                    path: h.path,
                    file,
                    rows,
                    level: h.level,
                    reason: h.reason,
                    source: h.source,
                })
            })
            .collect();
        placed.sort_by_key(|h| (h.level, h.file, h.rows.map_or(u32::MAX, |r| r.start)));
        self.hotspots = placed;
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

    /// Anchors reported hotspots. Ones outside the review's files are left out.
    fn anchor_hotspots(
        &self,
        store: &Store,
        inputs: &[HotspotInput],
        source: Source,
    ) -> anyhow::Result<Vec<Hotspot>> {
        let now = now_ms();
        let mut out = Vec::new();
        for h in inputs {
            let Some(rows) = self.rows_for(&h.path, h.start.min(h.end), h.start.max(h.end)) else {
                continue;
            };
            let (path, pins) = self.pins(store, &rows)?;
            out.push(Hotspot {
                id: 0,
                path,
                pins,
                level: h.level,
                reason: h.reason.clone(),
                review: self.label.clone(),
                source,
                created_at: now,
            });
        }
        Ok(out)
    }
}

impl RepoCtx {
    /// Replaces the review's hotspots with `inputs`. Returns the new state and how many
    /// hotspots were left out for being outside the review's files.
    pub fn save_hotspots(
        &self,
        id: u32,
        inputs: &[HotspotInput],
        source: Source,
    ) -> anyhow::Result<(StateDto, usize)> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let mut store = self.store.lock().unwrap();
        let hotspots = review.anchor_hotspots(&store, inputs, source)?;
        store.replace_hotspots(&review.label, &hotspots)?;
        review.rematch_hotspots(&store)?;
        Ok((review.state(), inputs.len() - hotspots.len()))
    }
}
