//! Where `e` opens the editor: the cursor's line in the new file.

use lgtm_core::diff::FileStatus;

use super::Review;
use super::rows::Line;

impl Review {
    /// The cursor's file and its line in the new file. A removed row opens where it was
    /// removed. `None` on a removed file.
    pub fn edit_target(&self) -> Option<(String, u32)> {
        let line = self.current()?.code();
        let file = &self.files[line.file()];
        if file.status == FileStatus::Removed {
            return None;
        }
        let at = match line {
            Line::Gap(_) | Line::File(_) => file.hunks.first().map_or(1, |h| h.new_start),
            Line::Hunk(_, h) => file.hunks[h].new_start,
            Line::Code { hunk, old, new, .. } => {
                let hunk = &file.hunks[hunk];
                let i = new.or(old).unwrap_or_default();
                // The next row with a new line is where a removed row was.
                hunk.rows[i..]
                    .iter()
                    .find_map(|r| r.new_line)
                    .unwrap_or(hunk.new_start + hunk.new_lines)
            }
            Line::Note { .. } => unreachable!("code() maps notes to their rows"),
        };
        Some((file.path.clone(), at.max(1)))
    }
}
