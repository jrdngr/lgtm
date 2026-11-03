//! Rows of a review file as unified diff text.

use super::file::RowPos;
use crate::diff::RowKind;

/// Rows as unified diff hunks: a `@@ -old +new @@` header wherever the line numbers jump,
/// then each row with its sign.
pub fn unified<'a>(rows: impl Iterator<Item = (RowPos, &'a str)>) -> String {
    let mut out: Vec<String> = vec![];
    let mut hunk: Option<HunkHead> = None;
    for (pos, text) in rows {
        if !hunk.as_ref().is_some_and(|h| h.follows(&pos)) {
            if let Some(h) = hunk.take() {
                out[h.at] = h.header();
            }
            out.push(String::new());
            hunk = Some(HunkHead {
                at: out.len() - 1,
                ..HunkHead::default()
            });
        }
        hunk.as_mut().unwrap().add(&pos);
        let sign = match pos.kind {
            RowKind::Added => '+',
            RowKind::Removed => '-',
            RowKind::Context => ' ',
        };
        out.push(format!("{sign}{text}"));
    }
    if let Some(h) = hunk {
        out[h.at] = h.header();
    }
    out.join("\n")
}

/// A hunk being written: where its header goes, and each side's first line and line count.
#[derive(Default)]
struct HunkHead {
    at: usize,
    old: (u32, u32),
    new: (u32, u32),
}

impl HunkHead {
    fn add(&mut self, pos: &RowPos) {
        for (side, line) in [(&mut self.old, pos.old_line), (&mut self.new, pos.new_line)] {
            if let Some(l) = line {
                if side.1 == 0 {
                    side.0 = l;
                }
                side.1 += 1;
            }
        }
    }

    /// Whether `pos` comes right after the hunk's last line on each side it's on.
    fn follows(&self, pos: &RowPos) -> bool {
        let next = |(start, count): (u32, u32), line: Option<u32>| {
            count == 0 || line.is_none_or(|l| l == start + count)
        };
        next(self.old, pos.old_line) && next(self.new, pos.new_line)
    }

    fn header(&self) -> String {
        format!(
            "@@ -{},{} +{},{} @@",
            self.old.0, self.old.1, self.new.0, self.new.1
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>) -> RowPos {
        RowPos {
            kind,
            old_line: old,
            new_line: new,
            replaced: false,
        }
    }

    #[test]
    fn unified_starts_a_hunk_where_line_numbers_jump() {
        let rows = [
            (row(RowKind::Context, Some(4), Some(4)), "a"),
            (row(RowKind::Removed, Some(5), None), "b"),
            (row(RowKind::Added, None, Some(5)), "B"),
            (row(RowKind::Added, None, Some(6)), "B2"),
            (row(RowKind::Context, Some(20), Some(21)), "z"),
        ];
        assert_eq!(
            unified(rows.into_iter()),
            "@@ -4,2 +4,3 @@\n a\n-b\n+B\n+B2\n@@ -20,1 +21,1 @@\n z"
        );
    }
}
