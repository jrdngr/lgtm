//! Comments for the agent: `c` writes one under the cursor's row, numbered C1, C2, …; the
//! agent reads them over MCP and marks them done, which hides them. `d` deletes one.

use lgtm_core::anchor::Side;
use lgtm_core::diff::{Row, RowKind};
use lgtm_core::jj::FileDiff;
use lgtm_core::store::{AgentComment, Store};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Review;
use super::rows::{Line, NoteKey, Notes};

/// A comment being written under a row.
pub struct Input {
    file: usize,
    hunk: usize,
    row: usize,
    number: i64,
    pub text: String,
}

/// Lines the comment input takes: its top border, its text, its bottom border.
pub const INPUT_HEIGHT: usize = 3;

/// A comment, and the file, hunk and row it's on now. `None` when its code is gone.
pub type Placed = (AgentComment, Option<(usize, usize, usize)>);

/// Where `c` is in `files`: the row on its side with its code, nearest its line.
pub fn place(files: &[FileDiff], c: &AgentComment) -> Option<(usize, usize, usize)> {
    let (f, file) = files
        .iter()
        .enumerate()
        .find(|(_, d)| d.path == c.path || d.old_path.as_deref() == Some(&c.path))?;
    let line = |r: &Row| match c.side {
        Side::New if r.kind != RowKind::Removed => r.new_line,
        Side::Old if r.kind != RowKind::Added => r.old_line,
        _ => None,
    };
    file.hunks
        .iter()
        .enumerate()
        .flat_map(|(h, hunk)| hunk.rows.iter().enumerate().map(move |(i, r)| (h, i, r)))
        .filter(|(_, _, r)| r.text == c.line_text)
        .filter_map(|(h, i, r)| Some((h, i, line(r)?.abs_diff(c.line))))
        .min_by_key(|&(_, _, d)| d)
        .map(|(h, i, _)| (f, h, i))
}

impl Review {
    /// Reads the open comments on the review's files from the store.
    pub fn load_comments(&mut self, store: &Store) {
        let mut paths: Vec<&str> = Vec::new();
        for f in &self.files {
            paths.push(&f.path);
            paths.extend(f.old_path.as_deref());
        }
        match store.agent_comments_for_paths(&paths) {
            Ok(found) => {
                self.comments = found
                    .into_iter()
                    .filter(|c| !c.done)
                    .map(|c| {
                        let at = place(&self.files, &c);
                        (c, at)
                    })
                    .collect();
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
        self.relayout();
    }

    pub fn typing(&self) -> bool {
        self.input.is_some()
    }

    /// The notes under rows and at the tops of files, for the layout.
    pub(super) fn notes(&self) -> Notes {
        let mut notes = Notes::default();
        for (c, at) in &self.comments {
            let note = (NoteKey::Comment(c.number), c.body.lines().count().max(1));
            match *at {
                Some(at) => notes.at_row.entry(at).or_default().push(note),
                None => {
                    let file = self
                        .files
                        .iter()
                        .position(|f| f.path == c.path || f.old_path.as_deref() == Some(&c.path));
                    if let Some(f) = file {
                        notes.at_file.entry(f).or_default().push(note);
                    }
                }
            }
        }
        if let Some(i) = &self.input {
            notes
                .at_row
                .entry((i.file, i.hunk, i.row))
                .or_default()
                .push((NoteKey::Input, INPUT_HEIGHT));
        }
        notes
    }

    pub(super) fn comment(&self, number: i64) -> Option<&AgentComment> {
        self.comments
            .iter()
            .map(|(c, _)| c)
            .find(|c| c.number == number)
    }

    pub(super) fn input(&self) -> Option<&Input> {
        self.input.as_ref()
    }

    pub(super) fn input_number(&self) -> i64 {
        self.input.as_ref().map_or(0, |i| i.number)
    }

    /// Starts a comment under the cursor's row.
    pub(super) fn start_comment(&mut self, store: Option<&Store>) {
        let (
            Some(Line::Code {
                file,
                hunk,
                old,
                new,
            }),
            Some(store),
        ) = (self.current(), store)
        else {
            return;
        };
        let Some(row) = new.or(old) else { return };
        match store.next_agent_comment_number() {
            Ok(number) => {
                self.input = Some(Input {
                    file,
                    hunk,
                    row,
                    number,
                    text: String::new(),
                });
                self.relayout();
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
    }

    /// Deletes the comment the cursor is on.
    pub(super) fn delete_comment(&mut self, store: Option<&Store>) {
        let (
            Some(Line::Note {
                key: NoteKey::Comment(number),
                ..
            }),
            Some(store),
        ) = (self.current(), store)
        else {
            return;
        };
        if let Err(e) = store.delete_agent_comment(number) {
            self.notice = Some(format!("{e:#}"));
        }
        self.load_comments(store);
    }

    /// Handles a key while writing a comment.
    pub(super) fn input_key(&mut self, key: KeyEvent, store: Option<&Store>) {
        let Some(input) = self.input.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => {
                self.input = None;
                self.relayout();
            }
            KeyCode::Enter => {
                let input = self.input.take().expect("checked above");
                let body = input.text.trim();
                if let (false, Some(store)) = (body.is_empty(), store) {
                    let saved = self.comment_at(&input, body);
                    if let Err(e) = store.add_agent_comment(&saved) {
                        self.notice = Some(format!("{e:#}"));
                    }
                    self.load_comments(store);
                } else {
                    self.relayout();
                }
            }
            KeyCode::Backspace => {
                input.text.pop();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.text.push(c);
            }
            _ => {}
        }
    }

    /// The comment `input` makes, with `body`.
    fn comment_at(&self, input: &Input, body: &str) -> AgentComment {
        let file = &self.files[input.file];
        let row = &file.hunks[input.hunk].rows[input.row];
        let (side, path, line) = match row.kind {
            RowKind::Removed => (
                Side::Old,
                file.old_path.clone().unwrap_or_else(|| file.path.clone()),
                row.old_line,
            ),
            _ => (Side::New, file.path.clone(), row.new_line),
        };
        AgentComment {
            id: 0,
            number: 0,
            path,
            side,
            line: line.unwrap_or(1),
            line_text: row.text.clone(),
            body: body.to_string(),
            review: self.revset.clone(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as i64),
            done: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgtm_core::diff::{FileStatus, Hunk};

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>, text: &str) -> Row {
        Row {
            kind,
            old_line: old,
            new_line: new,
            text: text.into(),
        }
    }

    fn comment(side: Side, line: u32, text: &str) -> AgentComment {
        AgentComment {
            id: 0,
            number: 1,
            path: "a.rs".into(),
            side,
            line,
            line_text: text.into(),
            body: String::new(),
            review: String::new(),
            created_at: 0,
            done: false,
        }
    }

    #[test]
    fn a_comment_lands_on_the_nearest_row_with_its_code_on_its_side() {
        let files = [FileDiff {
            path: "a.rs".into(),
            old_path: None,
            status: FileStatus::Modified,
            binary: false,
            conflict: false,
            old_exec: false,
            new_exec: false,
            blobs: None,
            hunks: vec![Hunk {
                old_start: 1,
                old_lines: 3,
                new_start: 1,
                new_lines: 3,
                section: String::new(),
                rows: vec![
                    row(RowKind::Context, Some(1), Some(1), "}"),
                    row(RowKind::Removed, Some(2), None, "x();"),
                    row(RowKind::Added, None, Some(2), "x();"),
                    row(RowKind::Context, Some(3), Some(3), "}"),
                ],
            }],
        }];
        assert_eq!(place(&files, &comment(Side::New, 4, "}")), Some((0, 0, 3)));
        assert_eq!(
            place(&files, &comment(Side::Old, 2, "x();")),
            Some((0, 0, 1))
        );
        assert_eq!(
            place(&files, &comment(Side::New, 2, "x();")),
            Some((0, 0, 2))
        );
        assert_eq!(place(&files, &comment(Side::New, 2, "gone();")), None);
    }
}
