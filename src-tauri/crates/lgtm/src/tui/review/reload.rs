//! Keeping the reader's place across a reload, when edits or jj operations change the diff:
//! the cursor stays on the same line of the same file, and folded hunks stay folded.

use std::collections::HashSet;

use lgtm_core::diff::Row;
use lgtm_core::jj::FileDiff;

use super::rows::Line;

/// Where the cursor is, by file path and line numbers rather than indexes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spot {
    path: String,
    at: At,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum At {
    File,
    Hunk {
        new_start: u32,
    },
    Row {
        old: Option<u32>,
        new: Option<u32>,
        text: String,
    },
}

impl Spot {
    pub fn of(files: &[FileDiff], line: Line) -> Self {
        let file = &files[line.file()];
        let at = match line.code() {
            Line::File(_) => At::File,
            Line::Hunk(_, h) => At::Hunk {
                new_start: file.hunks[h].new_start,
            },
            Line::Code { hunk, old, new, .. } => {
                let row = &file.hunks[hunk].rows[new.or(old).unwrap_or_default()];
                At::Row {
                    old: row.old_line,
                    new: row.new_line,
                    text: row.text.clone(),
                }
            }
            Line::Note { .. } => unreachable!("code() maps notes to their rows"),
        };
        Self {
            path: file.path.clone(),
            at,
        }
    }

    /// The index in `lines` of this spot in the reloaded `files`: the row with the same code
    /// nearest where it was, else the row now at its line, else its hunk's header, else the
    /// nearest row of the file, else the file's header.
    pub fn find(&self, files: &[FileDiff], lines: &[Line]) -> usize {
        let Some(f) = files.iter().position(|d| d.path == self.path) else {
            return 0;
        };
        let header = lines.iter().position(|l| *l == Line::File(f)).unwrap_or(0);
        let hunks = &files[f].hunks;
        let (old, new, text) = match &self.at {
            At::File => return header,
            &At::Hunk { new_start } => {
                let nearest = lines
                    .iter()
                    .enumerate()
                    .filter_map(|(i, l)| match *l {
                        Line::Hunk(lf, h) if lf == f => Some((i, hunks[h].new_start)),
                        _ => None,
                    })
                    .min_by_key(|&(_, s)| s.abs_diff(new_start));
                return nearest.map_or(header, |(i, _)| i);
            }
            At::Row { old, new, text } => (*old, *new, text),
        };
        let rows_of = |l: &Line| -> Vec<&Row> {
            match *l {
                Line::Code {
                    file,
                    hunk,
                    old,
                    new,
                } if file == f => [old, new]
                    .into_iter()
                    .flatten()
                    .map(|i| &hunks[hunk].rows[i])
                    .collect(),
                _ => vec![],
            }
        };
        let target = new.or(old).unwrap_or(1);
        let side = |r: &Row| {
            if new.is_some() {
                r.new_line
            } else {
                r.old_line
            }
        };
        let moved = lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let r = rows_of(l).into_iter().find(|r| r.text == *text)?;
                Some((i, side(r)?.abs_diff(target)))
            })
            .min_by_key(|&(_, d)| d);
        if let Some((i, _)) = moved {
            return i;
        }
        let same = |r: &Row| match new {
            Some(n) => r.new_line == Some(n),
            None => r.old_line == old,
        };
        if let Some(i) = lines.iter().position(|l| rows_of(l).into_iter().any(same)) {
            return i;
        }
        // The line is gone or folded away: its hunk's header, else the nearest row.
        let in_hunk = lines.iter().position(|l| match *l {
            Line::Hunk(lf, h) if lf == f => {
                let h = &hunks[h];
                (h.new_start..h.new_start + h.new_lines.max(1)).contains(&target)
            }
            _ => false,
        });
        if let Some(i) = in_hunk {
            return i;
        }
        lines
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                let line = rows_of(l).into_iter().find_map(side)?;
                Some((i, line.abs_diff(target)))
            })
            .min_by_key(|&(_, d)| d)
            .map_or(header, |(i, _)| i)
    }
}

/// Folded hunks by file path and where they start in the old and new file.
pub fn folded_keys(
    files: &[FileDiff],
    folded: &HashSet<(usize, usize)>,
) -> Vec<(String, u32, u32)> {
    folded
        .iter()
        .filter_map(|&(f, h)| {
            let file = files.get(f)?;
            let hunk = file.hunks.get(h)?;
            Some((file.path.clone(), hunk.old_start, hunk.new_start))
        })
        .collect()
}

/// The hunks of `files` to fold again: those of a folded hunk's file that start where it did
/// on either side.
pub fn refold(files: &[FileDiff], keys: &[(String, u32, u32)]) -> HashSet<(usize, usize)> {
    let mut out = HashSet::new();
    for (f, file) in files.iter().enumerate() {
        for (h, hunk) in file.hunks.iter().enumerate() {
            let was = keys.iter().any(|(path, old, new)| {
                *path == file.path && (*old == hunk.old_start || *new == hunk.new_start)
            });
            if was {
                out.insert((f, h));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::review::rows;
    use lgtm_core::diff::{FileStatus, Hunk, RowKind};

    fn hunk(old_start: u32, new_start: u32, kinds: &[(RowKind, &str)]) -> Hunk {
        let (mut old, mut new) = (old_start, new_start);
        let rows = kinds
            .iter()
            .map(|&(kind, text)| {
                let (o, n) = match kind {
                    RowKind::Context => (Some(old), Some(new)),
                    RowKind::Removed => (Some(old), None),
                    RowKind::Added => (None, Some(new)),
                };
                old += u32::from(o.is_some());
                new += u32::from(n.is_some());
                Row {
                    kind,
                    old_line: o,
                    new_line: n,
                    text: text.into(),
                }
            })
            .collect::<Vec<_>>();
        Hunk {
            old_start,
            old_lines: old - old_start,
            new_start,
            new_lines: new - new_start,
            section: String::new(),
            rows,
        }
    }

    fn file(path: &str, hunks: Vec<Hunk>) -> FileDiff {
        FileDiff {
            path: path.into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks,
        }
    }

    use RowKind::{Added as A, Context as C};

    #[test]
    fn the_cursor_follows_its_code_when_lines_come_before_it() {
        let before = [file(
            "b.rs",
            vec![hunk(10, 10, &[(C, "a"), (A, "b"), (A, "c")])],
        )];
        let lines = rows::lines(&before, &HashSet::new(), false);
        // On `c`, new line 12.
        let spot = Spot::of(&before, lines[4]);
        let after = [
            file("a.rs", vec![hunk(1, 1, &[(A, "new file")])]),
            file(
                "b.rs",
                vec![
                    hunk(1, 1, &[(A, "x")]),
                    hunk(10, 11, &[(C, "a"), (A, "b"), (A, "c")]),
                ],
            ),
        ];
        let lines = rows::lines(&after, &HashSet::new(), false);
        let found = lines[spot.find(&after, &lines)];
        let Line::Code {
            file, hunk, old, ..
        } = found
        else {
            panic!("not a row: {found:?}");
        };
        let row = &after[file].hunks[hunk].rows[old.unwrap()];
        assert_eq!((row.text.as_str(), row.new_line), ("c", Some(13)));
    }

    #[test]
    fn a_folded_hunk_stays_folded_and_holds_the_cursor_that_was_in_it() {
        let before = [file(
            "a.rs",
            vec![hunk(1, 1, &[(A, "x")]), hunk(20, 21, &[(C, "a"), (A, "b")])],
        )];
        let folded = HashSet::from([(0, 1)]);
        let open = rows::lines(&before, &HashSet::new(), false);
        let spot = Spot::of(&before, open[5]);
        let keys = folded_keys(&before, &folded);
        // The first hunk is gone, so the folded one starts a line higher in the new file.
        let after = [file(
            "a.rs",
            vec![hunk(20, 20, &[(C, "a"), (A, "b"), (A, "c")])],
        )];
        let refolded = refold(&after, &keys);
        assert_eq!(refolded, HashSet::from([(0, 0)]));
        let lines = rows::lines(&after, &refolded, false);
        assert_eq!(lines[spot.find(&after, &lines)], Line::Hunk(0, 0));
    }
}
