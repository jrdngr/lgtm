//! Comments for the agent: `c` writes one under the cursor's row, numbered C1, C2, …; the
//! agent reads them over MCP and marks them done, which hides them. `d` deletes one.

use lgtm_core::anchor::Side;
use lgtm_core::diff::{Row, RowKind};
use lgtm_core::jj::FileDiff;
use lgtm_core::store::{AgentComment, Store};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::Review;
use super::rows::{Line, NoteKey, Notes};
use super::undo::Step;
use super::wrap::wrap;

/// A comment being written under a row.
pub struct Input {
    file: usize,
    hunk: usize,
    row: usize,
    /// The first row of a comment on a range, as (hunk, row).
    start: Option<(usize, usize)>,
    /// Whether this edits comment `number` rather than writing a new one.
    editing: bool,
    number: i64,
    pub text: String,
}

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
                    .filter(|c| self.show_resolved || !c.done)
                    .map(|c| {
                        let at = place(&self.files, &c);
                        (c, at)
                    })
                    .filter(|(c, at)| c.review == self.revset || self.on_change(*at))
                    .collect();
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
        self.relayout();
    }

    /// Whether a comment is on a changed row. A comment made in another review shows here
    /// only then: on context, or nowhere, it's about another change.
    fn on_change(&self, at: Option<(usize, usize, usize)>) -> bool {
        at.is_some_and(|(f, h, r)| self.files[f].hunks[h].rows[r].kind != RowKind::Context)
    }

    pub fn typing(&self) -> bool {
        self.input.is_some() || self.cmd.is_some() || self.confirm.is_some()
    }

    /// The notes under rows and at the tops of files, for the layout.
    pub(super) fn notes(&self) -> Notes {
        let mut notes = Notes::default();
        let editing = self.input.as_ref().filter(|i| i.editing).map(|i| i.number);
        for (c, at) in self
            .comments
            .iter()
            .filter(|(c, _)| Some(c.number) != editing)
        {
            let note = (NoteKey::Comment(c.number), self.comment_lines(c).len());
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
        self.thread_notes(&mut notes);
        if let Some(i) = &self.input {
            notes
                .at_row
                .entry((i.file, i.hunk, i.row))
                .or_default()
                .push((NoteKey::Input, self.input_lines().len() + 2));
        }
        notes
    }

    /// A comment's body wrapped to the diff, beside its number.
    pub(super) fn comment_lines(&self, c: &AgentComment) -> Vec<String> {
        let id = format!("C{} ", c.number).len();
        wrap(&c.body, self.width.saturating_sub(5 + id))
    }

    /// The comment being written, wrapped inside its box, with room for the cursor.
    pub(super) fn input_lines(&self) -> Vec<String> {
        let text = self.input.as_ref().map_or("", |i| i.text.as_str());
        wrap(&format!("{text} "), self.width.saturating_sub(9))
    }

    /// How many open comments a file has: the agent's, and its PR threads while they show.
    pub(super) fn open_comments(&self, file: usize) -> usize {
        let path = &self.files[file].path;
        let comments = self
            .comments
            .iter()
            .filter(|(c, _)| &c.path == path)
            .count();
        let threads = if self.hide_pr {
            0
        } else {
            self.threads.iter().filter(|t| t.file == file).count()
        };
        comments + threads
    }

    pub(super) fn comment(&self, number: i64) -> Option<&AgentComment> {
        self.comments
            .iter()
            .map(|(c, _)| c)
            .find(|c| c.number == number)
    }

    pub(super) fn input_number(&self) -> i64 {
        self.input.as_ref().map_or(0, |i| i.number)
    }

    /// Starts a comment under the cursor's row, or the selection's last row for a comment on
    /// the selected rows.
    pub(super) fn start_comment(&mut self, store: Option<&Store>) {
        let (Some((file, first, (hunk, row))), Some(store)) = (self.selected_rows(), store) else {
            return;
        };
        let start = (first != (hunk, row)).then_some(first);
        self.anchor = None;
        match store.next_agent_comment_number() {
            Ok(number) => {
                self.input = Some(Input {
                    file,
                    hunk,
                    row,
                    start,
                    editing: false,
                    number,
                    text: String::new(),
                });
                self.relayout();
            }
            Err(e) => self.notice = Some(format!("{e:#}")),
        }
    }

    /// Edits the comment the cursor is on, in a box where it is.
    pub(super) fn edit_comment(&mut self, number: i64) {
        let Some((c, Some((file, hunk, row)))) = self
            .comments
            .iter()
            .find(|(c, _)| c.number == number && !c.done)
        else {
            return;
        };
        self.input = Some(Input {
            file: *file,
            hunk: *hunk,
            row: *row,
            start: None,
            editing: true,
            number,
            text: c.body.clone(),
        });
        self.relayout();
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
        let deleted = self.comment(number).cloned();
        match store.delete_agent_comment(number) {
            Ok(()) => self.history.extend(deleted.map(Step::Deleted)),
            Err(e) => self.notice = Some(format!("{e:#}")),
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
                    let saved = if input.editing {
                        let before = self.comment(input.number).map(|c| c.body.clone());
                        store.set_agent_comment_body(input.number, body).map(|()| {
                            before.map(|before| Step::Edited {
                                number: input.number,
                                before,
                                after: body.to_string(),
                            })
                        })
                    } else {
                        store
                            .add_agent_comment(&self.comment_at(&input, body))
                            .map(|c| Some(Step::Added(c)))
                    };
                    match saved {
                        Ok(step) => self.history.extend(step),
                        Err(e) => self.notice = Some(format!("{e:#}")),
                    }
                    self.load_comments(store);
                } else {
                    self.relayout();
                }
            }
            KeyCode::Backspace => {
                input.text.pop();
                self.relayout();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.text.push(c);
                self.relayout();
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
        let start = input.start.map(|(h, r)| {
            let row = &file.hunks[h].rows[r];
            let line = match row.kind {
                RowKind::Removed => row.old_line,
                _ => row.new_line,
            };
            (line.unwrap_or(1), row.text.clone())
        });
        AgentComment {
            id: 0,
            number: 0,
            path,
            side,
            line: line.unwrap_or(1),
            line_text: row.text.clone(),
            start,
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
            start: None,
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
