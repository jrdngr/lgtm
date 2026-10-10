//! Matches stored marks and file checks onto a review's rows, and finds amber rows: unreviewed
//! rows that replaced a line whose mark was lost, or that a checked file didn't have.

use std::collections::{HashMap, HashSet};

use similar::DiffOp;

use super::build::{FileSide, Keyed, ReviewFile};
use super::{Review, RowRef};
use crate::anchor::{self, Candidate, MarkRef, Side};
use crate::diff::FileStatus;
use crate::store::{FileCheck, Mark, Store};

/// A file's check and whether it still covers every changed row.
#[derive(Clone)]
pub struct MatchedCheck {
    pub check: FileCheck,
    pub current: bool,
}

impl Review {
    /// Rematches marks, checks, and comments, recomputes amber rows, and remembers what was
    /// matched.
    pub(super) fn rematch(&mut self, store: &Store) -> anyhow::Result<()> {
        let marks = store.marks_for_paths(&self.paths())?;
        self.matched = self.match_marks(store, &marks);
        let checks = store.checks_for_paths(&self.paths())?;
        self.checks = self.files.iter().map(|f| match_check(f, &checks)).collect();
        self.baseline.extend(self.matched.values().map(|m| m.id));
        self.amber = self.find_amber(store, &marks);
        self.rematch_comments(store)
    }

    /// Every path a mark or check in this review could be stored under.
    pub(super) fn paths(&self) -> Vec<&str> {
        let mut paths = Vec::new();
        for f in &self.files {
            paths.push(f.path.as_str());
            paths.extend(f.old_path.as_deref());
        }
        paths
    }

    pub(super) fn removed_paths(&self) -> Vec<&str> {
        self.files
            .iter()
            .filter(|f| f.status == FileStatus::Removed)
            .map(|f| f.path.as_str())
            .collect()
    }

    fn match_marks(&self, store: &Store, marks: &[Mark]) -> HashMap<RowRef, Mark> {
        let removed_paths = self.removed_paths();
        let mut remaps = Remaps::default();
        let by_id: HashMap<i64, &Mark> = marks.iter().map(|m| (m.id, m)).collect();
        let mut matched = HashMap::new();

        for (file_idx, file) in self.files.iter().enumerate() {
            for side in [Side::Old, Side::New] {
                let Some(rows) = SideRows::new(file_idx, file, side) else {
                    continue;
                };
                let mut taken: HashMap<usize, i64> = HashMap::new();
                for pool in pools(file, side, &removed_paths) {
                    let refs: Vec<MarkRef> = marks
                        .iter()
                        .filter(|m| m.anchor.side == side && pool.contains(&m.anchor.path.as_str()))
                        .filter(|m| !taken.values().any(|id| *id == m.id))
                        .map(|m| MarkRef {
                            id: m.id,
                            anchor: &m.anchor,
                            created_at: m.created_at,
                            remap: remaps.row(store, &m.commit, &m.anchor.path, m.line, &rows),
                        })
                        .collect();
                    let open: Vec<Candidate> = rows
                        .candidates
                        .iter()
                        .filter(|c| !taken.contains_key(&c.id))
                        .cloned()
                        .collect();
                    taken.extend(anchor::match_marks(&refs, &open, self.cfg));
                }
                for (row, mark_id) in taken {
                    let rref = RowRef {
                        file: file_idx as u32,
                        row: row as u32,
                    };
                    matched.insert(rref, by_id[&mark_id].clone());
                }
            }
        }
        matched
    }

    fn find_amber(&self, store: &Store, marks: &[Mark]) -> Vec<RowRef> {
        let matched_ids: HashSet<i64> = self.matched.values().map(|m| m.id).collect();
        let mut amber = HashSet::new();
        for (file_idx, file) in self.files.iter().enumerate() {
            let at = |k: &Keyed| RowRef {
                file: file_idx as u32,
                row: k.row,
            };
            if let Some(c) = self.checks[file_idx].as_ref().filter(|c| !c.current) {
                amber.extend(
                    file.keyed
                        .iter()
                        .filter(|k| !check_covers(&c.check, k))
                        .map(at),
                );
            }
            let lost = marks.iter().filter(|m| {
                self.baseline.contains(&m.id)
                    && !matched_ids.contains(&m.id)
                    && file.side(m.anchor.side).path == m.anchor.path
            });
            for m in lost {
                let Some(lines) = replaced_lines(store, file, m) else {
                    continue;
                };
                amber.extend(
                    file.keyed
                        .iter()
                        .filter(|k| k.side == m.anchor.side && lines.contains(&k.line))
                        .map(at),
                );
            }
        }
        let mut amber: Vec<RowRef> = amber
            .into_iter()
            .filter(|r| !self.matched.contains_key(r))
            .collect();
        amber.sort_by_key(|r| (r.file, r.row));
        amber
    }
}

fn check_covers(check: &FileCheck, k: &Keyed) -> bool {
    let hashes = match k.side {
        Side::Old => &check.old_hashes,
        Side::New => &check.new_hashes,
    };
    hashes.contains(&k.anchor.line_hash)
}

/// Changed rows of one side of a file, as candidates for anchors to land on.
pub(super) struct SideRows<'a> {
    pub file_idx: usize,
    pub side: Side,
    current: &'a FileSide,
    pub candidates: Vec<Candidate>,
    row_by_line: HashMap<u32, usize>,
}

impl<'a> SideRows<'a> {
    /// `None` when the side has no changed rows.
    pub fn new(file_idx: usize, file: &'a ReviewFile, side: Side) -> Option<Self> {
        let keyed: Vec<&Keyed> = file.keyed.iter().filter(|k| k.side == side).collect();
        if keyed.is_empty() {
            return None;
        }
        Some(Self {
            file_idx,
            side,
            current: file.side(side),
            candidates: keyed
                .iter()
                .map(|k| Candidate {
                    id: k.row as usize,
                    line_hash: k.anchor.line_hash,
                    above: k.anchor.above.clone(),
                    below: k.anchor.below.clone(),
                })
                .collect(),
            row_by_line: keyed.iter().map(|k| (k.line, k.row as usize)).collect(),
        })
    }
}

/// The stored paths anchors on `side` of `file` can have, as pools tried in order.
pub(super) fn pools<'a>(
    file: &'a ReviewFile,
    side: Side,
    removed_paths: &[&'a str],
) -> Vec<Vec<&'a str>> {
    let mut paths = vec![file.side(side).path.as_str()];
    if side == Side::New {
        paths.extend(file.old_path.as_deref());
    }
    let mut pools = vec![paths];
    if side == Side::New && file.status == FileStatus::Added {
        // Renames past jj's similarity cutoff show up as a delete plus an add.
        pools.push(removed_paths.to_vec());
    }
    pools
}

/// An anchor's commit and path, and the file index and side it's remapped onto.
type RemapKey = (String, String, usize, Side);

/// Line-diff remaps from the file an anchor was taken on to a current file side, cached.
#[derive(Default)]
pub(super) struct Remaps(HashMap<RemapKey, Option<HashMap<u32, u32>>>);

impl Remaps {
    /// The row an exact remap of `line` lands on, if it's a changed row of `rows`.
    pub fn row(
        &mut self,
        store: &Store,
        commit: &str,
        path: &str,
        line: u32,
        rows: &SideRows,
    ) -> Option<usize> {
        let key = (
            commit.to_string(),
            path.to_string(),
            rows.file_idx,
            rows.side,
        );
        let map = self.0.entry(key).or_insert_with(|| {
            let old = store.line_hashes(commit, path).ok()??;
            Some(anchor::remap_lines(
                &old,
                &rows.current.index.as_ref()?.hashes,
            ))
        });
        rows.row_by_line.get(map.as_ref()?.get(&line)?).copied()
    }
}

/// The newest check stored for the file's path (or its path before a rename).
fn match_check(file: &ReviewFile, checks: &[FileCheck]) -> Option<MatchedCheck> {
    let check = checks
        .iter()
        .filter(|c| c.path == file.path || Some(&c.path) == file.old_path.as_ref())
        .max_by_key(|c| (c.created_at, c.id))?;
    Some(MatchedCheck {
        current: file.keyed.iter().all(|k| check_covers(check, k)),
        check: check.clone(),
    })
}

/// 1-based lines of the file's current side that replaced the line `mark` was made on.
fn replaced_lines(store: &Store, file: &ReviewFile, mark: &Mark) -> Option<Vec<u32>> {
    let old = store.line_hashes(&mark.commit, &mark.anchor.path).ok()??;
    let new = &file.side(mark.anchor.side).index.as_ref()?.hashes;
    let line = mark.line as usize - 1;
    similar::capture_diff_slices(similar::Algorithm::Myers, &old, new)
        .into_iter()
        .find_map(|op| match op {
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } if (old_index..old_index + old_len).contains(&line) => Some(
                (new_index..new_index + new_len)
                    .map(|i| i as u32 + 1)
                    .collect(),
            ),
            _ => None,
        })
}
