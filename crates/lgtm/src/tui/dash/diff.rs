//! The live diff log's lines: each entry's file header and rows.

use lgtm_core::diff::RowKind;
use lgtm_core::jj::FileDiff;

/// A line of the live diff.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line {
    /// A file's name and counts.
    File(usize),
    Row {
        file: usize,
        hunk: usize,
        row: usize,
    },
    /// Between two hunks of a file.
    Gap,
    /// Between two files.
    Sep,
}

pub fn layout(diffs: &[FileDiff]) -> Vec<Line> {
    let mut lines = vec![];
    for (file, d) in diffs.iter().enumerate() {
        if file > 0 {
            lines.push(Line::Sep);
        }
        lines.push(Line::File(file));
        for (hunk, h) in d.hunks.iter().enumerate() {
            if hunk > 0 {
                lines.push(Line::Gap);
            }
            lines.extend((0..h.rows.len()).map(|row| Line::Row { file, hunk, row }));
        }
    }
    lines
}

/// How many lines a file adds and removes.
pub fn counts(d: &FileDiff) -> (usize, usize) {
    let rows = d.hunks.iter().flat_map(|h| &h.rows);
    rows.fold((0, 0), |(a, r), row| match row.kind {
        RowKind::Added => (a + 1, r),
        RowKind::Removed => (a, r + 1),
        RowKind::Context => (a, r),
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use lgtm_core::diff::{FileStatus, Hunk, Row};

    pub(in crate::tui::dash) fn file(path: &str, rows: &[&str]) -> FileDiff {
        let rows = rows
            .iter()
            .map(|r| {
                let (sign, text) = r.split_at(1);
                let kind = match sign {
                    "+" => RowKind::Added,
                    "-" => RowKind::Removed,
                    _ => RowKind::Context,
                };
                Row {
                    kind,
                    old_line: None,
                    new_line: None,
                    text: text.into(),
                }
            })
            .collect();
        FileDiff {
            path: path.into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 0,
                section: String::new(),
                rows,
            }],
        }
    }
}
