//! Where each row of a review went after its diff changed, so selections follow the code.

use std::collections::HashMap;

use serde::Serialize;
use specta::Type;

use super::RowRef;
use super::build::{FileSide, ReviewFile};
use crate::anchor;
use crate::diff::RowKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
pub struct RowMove {
    pub from: RowRef,
    pub to: RowRef,
}

/// Maps rows of `before` onto rows of `after` that show the same line of the same file side.
/// Rows whose line is gone are left out.
pub fn map_rows(before: &[ReviewFile], after: &[ReviewFile]) -> Vec<RowMove> {
    let by_path: HashMap<&str, usize> = after
        .iter()
        .enumerate()
        .flat_map(|(i, f)| {
            [
                Some((f.path.as_str(), i)),
                f.old_path.as_deref().map(|p| (p, i)),
            ]
        })
        .flatten()
        .collect();
    let mut out = Vec::new();
    for (bi, b) in before.iter().enumerate() {
        let Some(&ai) = by_path.get(b.path.as_str()) else {
            continue;
        };
        let a = &after[ai];
        // A side loaded without contents has no line hashes to diff, so rows map only
        // when the file's diff is unchanged.
        let contentless = [&b.old, &b.new, &a.old, &a.new]
            .iter()
            .any(|s| s.commit.is_some() && s.index.is_none());
        if contentless && b.rows == a.rows && b.texts == a.texts {
            out.extend((0..b.rows.len() as u32).map(|row| RowMove {
                from: RowRef::new(bi, row),
                to: RowRef::new(ai, row),
            }));
            continue;
        }
        let old_map = line_map(&b.old, &a.old);
        let new_map = line_map(&b.new, &a.new);
        let mut old_rows = HashMap::new();
        let mut new_rows = HashMap::new();
        for (row, pos) in a.rows.iter().enumerate() {
            match pos.kind {
                RowKind::Removed => old_rows.insert(pos.old_line, row),
                _ => new_rows.insert(pos.new_line, row),
            };
        }
        for (row, pos) in b.rows.iter().enumerate() {
            let to = match pos.kind {
                RowKind::Removed => old_map(pos.old_line).and_then(|l| old_rows.get(&Some(l))),
                _ => new_map(pos.new_line).and_then(|l| new_rows.get(&Some(l))),
            };
            if let Some(&to) = to {
                out.push(RowMove {
                    from: RowRef::new(bi, row as u32),
                    to: RowRef::new(ai, to as u32),
                });
            }
        }
    }
    out
}

/// Maps a line of `before` to the same line in `after`, by line diff of the two sides. An
/// interdiff's old side has no commit or contents, so its lines can't be told apart from
/// another interdiff's and map to nothing.
fn line_map(before: &FileSide, after: &FileSide) -> impl Fn(Option<u32>) -> Option<u32> {
    let same = before.commit.is_some() && before.commit == after.commit;
    let map = match (&before.index, &after.index) {
        (Some(_), Some(_)) if same && before.path == after.path => None,
        (Some(b), Some(a)) => Some(anchor::remap_lines(&b.hashes, &a.hashes)),
        _ => Some(HashMap::new()),
    };
    move |line| {
        let line = line?;
        match &map {
            None => Some(line),
            Some(m) => m.get(&line).copied(),
        }
    }
}
