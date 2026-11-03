//! The review's screen lines: each file's header, then each hunk's header and rows, unified or
//! paired side by side.

use std::collections::{HashMap, HashSet};

use lgtm_core::diff::{Row, RowKind};
use lgtm_core::jj::FileDiff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    File(usize),
    Hunk(usize, usize),
    /// Rows of a hunk by index: one row in unified view, or the old and new side's rows.
    Code {
        file: usize,
        hunk: usize,
        old: Option<usize>,
        new: Option<usize>,
    },
    /// A line of a note under a row, or at the top of a file when its code is gone.
    Note {
        file: usize,
        /// The hunk and row it's under.
        at: Option<(usize, usize)>,
        key: NoteKey,
        part: usize,
    },
}

/// What a note line belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteKey {
    /// A comment for the agent, by number.
    Comment(i64),
    /// The comment being written.
    Input,
}

/// Notes by where they go, each with how many lines it takes.
#[derive(Default)]
pub struct Notes {
    /// Under a row, by file, hunk and row.
    pub at_row: HashMap<(usize, usize, usize), Vec<(NoteKey, usize)>>,
    /// At the top of a file.
    pub at_file: HashMap<usize, Vec<(NoteKey, usize)>>,
}

impl Line {
    pub fn file(self) -> usize {
        match self {
            Line::File(f)
            | Line::Hunk(f, _)
            | Line::Code { file: f, .. }
            | Line::Note { file: f, .. } => f,
        }
    }

    pub fn hunk(self) -> Option<usize> {
        match self {
            Line::File(_) => None,
            Line::Hunk(_, h) | Line::Code { hunk: h, .. } => Some(h),
            Line::Note { at, .. } => at.map(|(h, _)| h),
        }
    }

    /// The code line a note line belongs to: its row, or its file's header.
    pub fn code(self) -> Line {
        match self {
            Line::Note {
                file,
                at: Some((hunk, row)),
                ..
            } => Line::Code {
                file,
                hunk,
                old: Some(row),
                new: None,
            },
            Line::Note { file, at: None, .. } => Line::File(file),
            line => line,
        }
    }

    /// Where the line is in the diff, the same in both views: file, hunk, and a row of it.
    pub fn spot(self) -> (usize, Option<usize>, Option<usize>) {
        match self {
            Line::File(f) => (f, None, None),
            Line::Hunk(f, h) => (f, Some(h), None),
            Line::Code {
                file,
                hunk,
                old,
                new,
            } => (file, Some(hunk), new.or(old)),
            Line::Note { file, at, .. } => (file, at.map(|(h, _)| h), at.map(|(_, r)| r)),
        }
    }
}

#[cfg(test)]
pub fn lines(files: &[FileDiff], folded: &HashSet<(usize, usize)>, split: bool) -> Vec<Line> {
    lines_with(files, folded, split, &Notes::default())
}

/// The lines of `files`, with `notes` under their rows. A row in a folded hunk shows none.
pub fn lines_with(
    files: &[FileDiff],
    folded: &HashSet<(usize, usize)>,
    split: bool,
    notes: &Notes,
) -> Vec<Line> {
    let mut out = Vec::new();
    let note_lines = |out: &mut Vec<Line>, file, at, list: Option<&Vec<(NoteKey, usize)>>| {
        for &(key, height) in list.into_iter().flatten() {
            out.extend((0..height).map(|part| Line::Note {
                file,
                at,
                key,
                part,
            }));
        }
    };
    for (f, file) in files.iter().enumerate() {
        out.push(Line::File(f));
        note_lines(&mut out, f, None, notes.at_file.get(&f));
        for (h, hunk) in file.hunks.iter().enumerate() {
            out.push(Line::Hunk(f, h));
            if folded.contains(&(f, h)) {
                continue;
            }
            let code = |(old, new)| Line::Code {
                file: f,
                hunk: h,
                old,
                new,
            };
            let pairs = if split {
                pairs(&hunk.rows)
            } else {
                (0..hunk.rows.len()).map(|i| (Some(i), None)).collect()
            };
            for (old, new) in pairs {
                out.push(code((old, new)));
                let mut rows: Vec<usize> = [old, new].into_iter().flatten().collect();
                rows.dedup();
                for row in rows {
                    note_lines(&mut out, f, Some((h, row)), notes.at_row.get(&(f, h, row)));
                }
            }
        }
    }
    out
}

/// Side-by-side rows: context on both sides, and each run of removed rows beside the added
/// rows that follow it.
pub fn pairs(rows: &[Row]) -> Vec<(Option<usize>, Option<usize>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        if rows[i].kind == RowKind::Context {
            out.push((Some(i), Some(i)));
            i += 1;
            continue;
        }
        let removed: Vec<usize> = (i..rows.len())
            .take_while(|&j| rows[j].kind == RowKind::Removed)
            .collect();
        i += removed.len();
        let added: Vec<usize> = (i..rows.len())
            .take_while(|&j| rows[j].kind == RowKind::Added)
            .collect();
        i += added.len();
        for k in 0..removed.len().max(added.len()) {
            out.push((removed.get(k).copied(), added.get(k).copied()));
        }
    }
    out
}

/// The index in `lines` nearest to `spot`: the same row, else its hunk's header, else its
/// file's header.
pub fn find(lines: &[Line], spot: (usize, Option<usize>, Option<usize>)) -> usize {
    let (file, hunk, row) = spot;
    let matches = |l: &Line| match *l {
        Line::Code {
            file: f,
            hunk: h,
            old,
            new,
        } => f == file && Some(h) == hunk && row.is_some() && (old == row || new == row),
        _ => false,
    };
    lines
        .iter()
        .position(matches)
        .or_else(|| {
            lines
                .iter()
                .position(|l| *l == hunk.map_or(Line::File(file), |h| Line::Hunk(file, h)))
        })
        .or_else(|| lines.iter().position(|l| *l == Line::File(file)))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::diff::{FileStatus, Hunk};

    fn row(kind: RowKind) -> Row {
        Row {
            kind,
            old_line: None,
            new_line: None,
            text: String::new(),
        }
    }

    fn file(hunks: Vec<Vec<RowKind>>) -> FileDiff {
        FileDiff {
            path: "a.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks: hunks
                .into_iter()
                .map(|rows| Hunk {
                    old_start: 1,
                    old_lines: 0,
                    new_start: 1,
                    new_lines: 0,
                    section: String::new(),
                    rows: rows.into_iter().map(row).collect(),
                })
                .collect(),
        }
    }

    use RowKind::{Added as A, Context as C, Removed as R};

    #[test]
    fn removed_rows_pair_with_the_added_rows_after_them() {
        let rows: Vec<Row> = [C, R, R, A, C, A, R].into_iter().map(row).collect();
        assert_eq!(
            pairs(&rows),
            vec![
                (Some(0), Some(0)),
                (Some(1), Some(3)),
                (Some(2), None),
                (Some(4), Some(4)),
                (None, Some(5)),
                (Some(6), None),
            ]
        );
    }

    #[test]
    fn a_folded_hunk_keeps_only_its_header() {
        let files = [file(vec![vec![C, A], vec![R]])];
        let folded = HashSet::from([(0, 0)]);
        assert_eq!(
            lines(&files, &folded, false),
            vec![
                Line::File(0),
                Line::Hunk(0, 0),
                Line::Hunk(0, 1),
                Line::Code {
                    file: 0,
                    hunk: 1,
                    old: Some(0),
                    new: None
                },
            ]
        );
    }

    #[test]
    fn a_row_is_found_again_after_switching_views() {
        let files = [file(vec![vec![C, R, A, A]])];
        let unified = lines(&files, &HashSet::new(), false);
        let split = lines(&files, &HashSet::new(), true);
        // The second added row is on its own line side by side.
        let at = find(&split, unified[5].spot());
        assert_eq!(
            split[at],
            Line::Code {
                file: 0,
                hunk: 0,
                old: None,
                new: Some(3)
            }
        );
        // A row inside a folded hunk falls back to the hunk's header.
        let folded = lines(&files, &HashSet::from([(0, 0)]), false);
        assert_eq!(folded[find(&folded, unified[5].spot())], Line::Hunk(0, 0));
    }
}
