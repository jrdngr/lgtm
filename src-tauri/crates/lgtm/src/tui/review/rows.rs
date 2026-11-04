//! The review's screen lines: each file's header, then its hunks' rows, unified or paired side
//! by side, with a divider line above each hunk where a file has more than one.

use std::collections::{HashMap, HashSet};

use lgtm_core::diff::{Row, RowKind};
use lgtm_core::jj::FileDiff;

use super::wrap::code_parts;

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
        /// Which screen line of the row this is, when it wraps.
        part: usize,
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
    /// A PR thread, by its id in the store.
    Thread(i64),
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
                part: 0,
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
                ..
            } => (file, Some(hunk), new.or(old)),
            Line::Note { file, at, .. } => (file, at.map(|(h, _)| h), at.map(|(_, r)| r)),
        }
    }
}

#[cfg(test)]
pub fn lines(files: &[FileDiff], folded: &HashSet<usize>, split: bool) -> Vec<Line> {
    let order: Vec<usize> = (0..files.len()).collect();
    lines_with(files, &order, folded, split, &Notes::default(), None)
}

/// The lines of `files`, in `order`, with `notes` under their rows. A folded file shows only
/// its header. With `wrap`, code rows wrap to that many columns.
pub fn lines_with(
    files: &[FileDiff],
    order: &[usize],
    folded: &HashSet<usize>,
    split: bool,
    notes: &Notes,
    wrap: Option<usize>,
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
    for &f in order {
        let file = &files[f];
        out.push(Line::File(f));
        if folded.contains(&f) {
            continue;
        }
        note_lines(&mut out, f, None, notes.at_file.get(&f));
        for (h, hunk) in file.hunks.iter().enumerate() {
            if file.hunks.len() > 1 {
                out.push(Line::Hunk(f, h));
            }
            let height = |i: Option<usize>| match (i, wrap) {
                (Some(i), Some(w)) => code_parts(&hunk.rows[i].text, w).len(),
                _ => 1,
            };
            let pairs = if split {
                pairs(&hunk.rows)
            } else {
                (0..hunk.rows.len()).map(|i| (Some(i), None)).collect()
            };
            for (old, new) in pairs {
                let parts = height(old).max(height(new));
                out.extend((0..parts).map(|part| Line::Code {
                    file: f,
                    hunk: h,
                    old,
                    new,
                    part,
                }));
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
            ..
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
        // A file that isn't shown gives way to the next one, or the last.
        .or_else(|| lines.iter().position(|l| l.file() > file))
        .unwrap_or(lines.len().saturating_sub(1))
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
    fn a_folded_file_keeps_only_its_header_and_one_hunk_has_no_divider() {
        let files = [file(vec![vec![C, A], vec![R]]), file(vec![vec![R]])];
        let code = |file, hunk| Line::Code {
            file,
            hunk,
            old: Some(0),
            new: None,
            part: 0,
        };
        assert_eq!(
            lines(&files, &HashSet::new(), false),
            vec![
                Line::File(0),
                Line::Hunk(0, 0),
                code(0, 0),
                Line::Code {
                    file: 0,
                    hunk: 0,
                    old: Some(1),
                    new: None,
                    part: 0,
                },
                Line::Hunk(0, 1),
                code(0, 1),
                Line::File(1),
                code(1, 0),
            ]
        );
        assert_eq!(
            lines(&files, &HashSet::from([0]), false),
            vec![Line::File(0), Line::File(1), code(1, 0)]
        );
    }

    #[test]
    fn a_file_not_shown_gives_way_to_the_next_one_or_the_last() {
        let files = [
            file(vec![vec![A]]),
            file(vec![vec![A]]),
            file(vec![vec![A]]),
        ];
        let mut shown = lines(&files, &HashSet::new(), false);
        shown.retain(|l| l.file() != 1);
        assert_eq!(shown[find(&shown, (1, Some(0), Some(0)))], Line::File(2));
        shown.retain(|l| l.file() != 2);
        assert_eq!(find(&shown, (2, None, None)), shown.len() - 1);
    }

    #[test]
    fn a_row_is_found_again_after_switching_views() {
        let files = [file(vec![vec![C, R, A, A]])];
        let unified = lines(&files, &HashSet::new(), false);
        let split = lines(&files, &HashSet::new(), true);
        // The second added row is on its own line side by side.
        let at = find(&split, unified[4].spot());
        assert_eq!(
            split[at],
            Line::Code {
                file: 0,
                hunk: 0,
                old: None,
                new: Some(3),
                part: 0,
            }
        );
        // A row inside a folded file falls back to the file's header.
        let folded = lines(&files, &HashSet::from([0]), false);
        assert_eq!(folded[find(&folded, unified[4].spot())], Line::File(0));
    }
}
