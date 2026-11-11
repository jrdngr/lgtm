//! Comments (C1, C2, …) as both apps use them: placing one on a diff's rows, an agent's
//! from where it asked, listing them for an agent, and undoing changes to them.

use serde_json::{Value, json};

use crate::anchor::Side;
use crate::diff::RowKind;
use crate::review::agent::AddComment;
use crate::store::{AgentComment, Edit, Store};

/// What placing and listing a comment need of a row.
#[derive(Clone, Copy)]
pub struct RowText<'a> {
    pub kind: RowKind,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    pub text: &'a str,
}

impl RowText<'_> {
    /// The row's line on `side`, when it has one there.
    fn line(&self, side: Side) -> Option<u32> {
        match side {
            Side::New if self.kind != RowKind::Removed => self.new_line,
            Side::Old if self.kind != RowKind::Added => self.old_line,
            _ => None,
        }
    }
}

/// A new comment saying `body` on rows `start..=end` of the file at `path` (`old_path` before a
/// rename), or on `end` alone. It's on the old side when its last row was removed.
pub fn new_comment(
    path: &str,
    old_path: Option<&str>,
    start: Option<RowText>,
    end: RowText,
    body: &str,
    review: &str,
) -> AgentComment {
    let (side, path, line) = match end.kind {
        RowKind::Removed => (Side::Old, old_path.unwrap_or(path), end.old_line),
        _ => (Side::New, path, end.new_line),
    };
    let start = start.map(|row| {
        let line = match row.kind {
            RowKind::Removed => row.old_line,
            _ => row.new_line,
        };
        (line.unwrap_or(1), row.text.to_string())
    });
    AgentComment {
        id: 0,
        number: 0,
        path: path.to_string(),
        side,
        line: line.unwrap_or(1),
        line_text: end.text.to_string(),
        start,
        body: body.to_string(),
        review: review.to_string(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as i64),
        done: false,
        author: None,
    }
}

/// The comment an agent asked for with `add`, on the row of the file's `rows` at its line.
pub fn from_agent<'a>(
    mut rows: impl Iterator<Item = RowText<'a>>,
    path: &str,
    old_path: Option<&str>,
    add: &AddComment,
    review: &str,
) -> Result<AgentComment, String> {
    let body = add.body.trim();
    if body.is_empty() {
        return Err("the comment has no text".into());
    }
    let row = match (add.line, add.removed_line) {
        (Some(n), None) => rows.find(|r| r.kind != RowKind::Removed && r.new_line == Some(n)),
        (None, Some(n)) => rows.find(|r| r.kind == RowKind::Removed && r.old_line == Some(n)),
        _ => return Err("give either line or removedLine".into()),
    };
    let row = row.ok_or_else(|| {
        format!(
            "that line isn't in the diff of `{}`: comment on a changed line or one beside it",
            add.path
        )
    })?;
    Ok(AgentComment {
        author: Some(add.author.clone().unwrap_or_else(|| "Agent".into())),
        ..new_comment(path, old_path, None, row, body, review)
    })
}

/// Whether a review shows comment `c`, found on a row of kind `at`, or nowhere: always when
/// it was made in `review`, else only on a changed row. On context, or nowhere, it's about
/// another change.
pub fn shows_in(c: &AgentComment, review: &str, at: Option<RowKind>) -> bool {
    c.review == review || at.is_some_and(|k| k != RowKind::Context)
}

/// Where `c` is among a file's `rows`: the row on its side with its code, nearest its line.
/// A resolved comment whose code is gone goes on the row nearest its line instead.
pub fn place<'a, K: Copy>(
    rows: impl Iterator<Item = (K, RowText<'a>)> + Clone,
    c: &AgentComment,
) -> Option<K> {
    let nearest = |rows: &mut dyn Iterator<Item = (K, RowText<'a>)>| {
        rows.filter_map(|(k, r)| Some((k, r.line(c.side)?.abs_diff(c.line))))
            .min_by_key(|&(_, d)| d)
            .map(|(k, _)| k)
    };
    let same = nearest(&mut rows.clone().filter(|(_, r)| r.text == c.line_text));
    if same.is_some() || !c.done {
        return same;
    }
    nearest(&mut rows.clone())
}

/// The comment as its range's first row: its start line and that line's code.
pub fn start_of(c: &AgentComment) -> Option<AgentComment> {
    let (line, text) = c.start.clone()?;
    Some(AgentComment {
        line,
        line_text: text,
        ..c.clone()
    })
}

/// A run of rows' first line and their code as a unified diff. `None` without rows.
pub fn range_code(rows: &[RowText]) -> Option<(Option<u32>, String)> {
    let first = rows.first()?;
    let first_line = match first.kind {
        RowKind::Removed => first.old_line,
        _ => first.new_line,
    };
    let code = rows
        .iter()
        .map(|r| {
            let sign = match r.kind {
                RowKind::Added => '+',
                RowKind::Removed => '-',
                RowKind::Context => ' ',
            };
            format!("{sign}{}", r.text)
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some((first_line, code))
}

/// A comment as `get_comments` lists it: `line` is where it is now, and `range` its first
/// line and code when it's on a range of rows that's still there.
pub fn listing(c: &AgentComment, line: Option<u32>, range: Option<(Option<u32>, String)>) -> Value {
    let mut out = json!({
        "number": format!("C{}", c.number),
        "path": c.path,
        "side": if c.side == Side::Old { "old" } else { "new" },
        "line": line,
        "code": c.line_text,
        "body": c.body,
    });
    if c.done {
        out["resolved"] = json!(true);
    }
    if let Some(author) = &c.author {
        out["author"] = json!(author);
    }
    if let Some((start, code)) = range {
        out["startLine"] = json!(start);
        out["code"] = json!(code);
    }
    out
}

/// A review change that undo takes back.
#[derive(Clone)]
pub enum Step {
    /// Marks and checks, as the store applied them.
    Marks(Edit),
    Added(AgentComment),
    Deleted(AgentComment),
    Edited {
        number: i64,
        before: String,
        after: String,
    },
    /// A comment resolved, or opened again.
    Resolved {
        number: i64,
        done: bool,
    },
}

/// Puts comment `c` back, or deletes it. Returns it as stored.
fn set_comment(store: &Store, c: AgentComment, there: bool) -> anyhow::Result<AgentComment> {
    if there {
        return Ok(store.restore_agent_comment(&c)?);
    }
    store.delete_agent_comment(c.number)?;
    Ok(c)
}

impl Step {
    pub fn is_empty(&self) -> bool {
        matches!(self, Step::Marks(e) if e.is_empty())
    }

    /// The marks and checks the step changes, if any.
    pub fn marks(&self) -> Option<&Edit> {
        match self {
            Step::Marks(e) => Some(e),
            _ => None,
        }
    }

    /// Undoes the step, or redoes it. Returns it as it now stands, to go the other way.
    pub fn run(self, store: &mut Store, back: bool) -> anyhow::Result<Step> {
        Ok(match self {
            Step::Marks(edit) if back => Step::Marks(store.apply(&edit.inverse())?.inverse()),
            Step::Marks(edit) => Step::Marks(store.apply(&edit)?),
            Step::Added(c) => Step::Added(set_comment(store, c, !back)?),
            Step::Deleted(c) => Step::Deleted(set_comment(store, c, back)?),
            Step::Resolved { number, done } => {
                store.set_agent_comment_done(number, done != back)?;
                Step::Resolved { number, done }
            }
            Step::Edited {
                number,
                before,
                after,
            } => {
                store.set_agent_comment_body(number, if back { &before } else { &after })?;
                Step::Edited {
                    number,
                    before,
                    after,
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: RowKind, old: Option<u32>, new: Option<u32>, text: &str) -> RowText<'_> {
        RowText {
            kind,
            old_line: old,
            new_line: new,
            text,
        }
    }

    fn comment(line: u32, text: &str, done: bool) -> AgentComment {
        AgentComment {
            id: 0,
            number: 1,
            path: "a.rs".into(),
            side: Side::New,
            line,
            line_text: text.into(),
            start: None,
            body: String::new(),
            review: "@".into(),
            created_at: 0,
            done,
            author: None,
        }
    }

    #[test]
    fn a_comment_finds_its_code_nearest_its_line_and_a_resolved_one_outlives_it() {
        let rows = [
            row(RowKind::Added, None, Some(3), "x"),
            row(RowKind::Context, Some(8), Some(9), "y"),
            row(RowKind::Added, None, Some(20), "x"),
        ];
        let at = || rows.iter().copied().enumerate();
        assert_eq!(place(at(), &comment(18, "x", false)), Some(2));
        assert_eq!(place(at(), &comment(10, "gone", false)), None);
        assert_eq!(place(at(), &comment(10, "gone", true)), Some(1));
    }

    #[test]
    fn an_agent_comments_on_a_diff_line_by_its_number_on_either_side() {
        let rows = [
            row(RowKind::Removed, Some(19), None, "a > b"),
            row(RowKind::Added, None, Some(21), "a >= b"),
        ];
        let add = |line, removed_line| AddComment {
            revset: "@".into(),
            path: "a.rs".into(),
            line,
            removed_line,
            body: " Off by one? ".into(),
            author: Some("Claude".into()),
        };
        let on = |a: &AddComment| from_agent(rows.iter().copied(), "a.rs", None, a, "@");
        let new = on(&add(Some(21), None)).unwrap();
        assert_eq!(
            (
                new.side,
                new.line,
                new.line_text.as_str(),
                new.body.as_str()
            ),
            (Side::New, 21, "a >= b", "Off by one?")
        );
        assert_eq!(new.author.as_deref(), Some("Claude"));
        let old = on(&add(None, Some(19))).unwrap();
        assert_eq!((old.side, old.line), (Side::Old, 19));
        assert!(on(&add(Some(19), None)).is_err());
        assert!(on(&add(Some(21), Some(19))).is_err());
    }
}
