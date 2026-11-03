//! The diff gutter: which lines of the buffer were added or changed since a base text, and
//! where lines were deleted.

use similar::{Algorithm, DiffOp, capture_diff_slices};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Added,
    Modified,
    /// Lines were deleted just above this one.
    Deleted,
}

/// A mark for each line of `text` that differs from `base`, by line from 0.
pub fn marks(base: &str, text: &str) -> Vec<Option<Mark>> {
    let old: Vec<&str> = base.lines().collect();
    let new: Vec<&str> = text.lines().collect();
    let mut out = vec![None; new.len().max(1)];
    let last = out.len() - 1;
    for op in capture_diff_slices(Algorithm::Myers, &old, &new) {
        match op {
            DiffOp::Equal { .. } => {}
            DiffOp::Insert {
                new_index, new_len, ..
            } => out[new_index..new_index + new_len].fill(Some(Mark::Added)),
            DiffOp::Replace {
                new_index, new_len, ..
            } => out[new_index..new_index + new_len].fill(Some(Mark::Modified)),
            DiffOp::Delete { new_index, .. } => {
                let at = new_index.min(last);
                out[at].get_or_insert(Mark::Deleted);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use Mark::{Added, Deleted, Modified};

    #[test]
    fn added_changed_and_deleted_runs_are_marked() {
        let base = "a\nb\nc\nd\ne\n";
        let text = "a\nB\nc\nnew\nnew\nd\ne\n";
        assert_eq!(
            marks(base, text),
            [
                None,
                Some(Modified),
                None,
                Some(Added),
                Some(Added),
                None,
                None
            ]
        );
        // A line gone from the end marks the last line.
        assert_eq!(marks("a\nb\n", "a\n"), [Some(Deleted)]);
        assert_eq!(marks("a\nb\nc\n", "a\nc\n"), [None, Some(Deleted)]);
    }

    #[test]
    fn a_new_file_is_all_added_and_the_same_text_has_no_marks() {
        assert_eq!(marks("", "a\nb"), [Some(Added), Some(Added)]);
        assert_eq!(marks("a\nb\n", "a\nb\n"), [None, None]);
    }
}
