//! What can happen to an issue, shared by the CLI and the TUI.

use anyhow::{Context, Result, bail};

use crate::fields::{self, Severity};
use crate::folder::Folder;
use crate::issue::{Issue, Kind, Status};
use crate::time::{self, Minute};

/// An answer to hand to the agent that asked: its terminal and the message.
pub struct Delivery {
    pub to: String,
    pub text: String,
}

pub fn file(folder: &Folder, title: &str, body: &str, who: &str, with: Vec<Field>) -> Result<u32> {
    if title.trim().is_empty() {
        bail!("an issue needs a title");
    }
    let mut issue = Issue::new(0, title, Some(who.into()), body, time::now());
    for f in with {
        check(folder, None, &f)?;
        f.apply(&mut issue);
    }
    folder.create(issue)
}

/// A header field set to a value, or cleared with None or no ids.
#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Severity(Option<Severity>),
    Revision(Option<String>),
    BlockedBy(Vec<u32>),
    DuplicateOf(Option<u32>),
}

impl Field {
    /// Reads `lgtm issues set` arguments: a field's name, then its value, none clearing it.
    pub fn parse(name: &str, value: &str) -> Result<Field> {
        let value = value.trim();
        let ids = || fields::parse_ids(value).map_err(anyhow::Error::msg);
        Ok(match name {
            "severity" => Field::Severity(match value {
                "" => None,
                v => Some(Severity::parse(v).with_context(|| {
                    format!("severity is low, medium, high or critical, not {v}")
                })?),
            }),
            "revision" => Field::Revision(Some(value.to_owned()).filter(|v| !v.is_empty())),
            "blocked-by" => Field::BlockedBy(ids()?),
            "duplicate-of" => match ids()?.as_slice() {
                [] => Field::DuplicateOf(None),
                [id] => Field::DuplicateOf(Some(*id)),
                _ => bail!("an issue duplicates one other issue"),
            },
            _ => bail!("no field {name}: severity, revision, blocked-by or duplicate-of"),
        })
    }

    fn ids(&self) -> &[u32] {
        match self {
            Field::BlockedBy(ids) => ids,
            Field::DuplicateOf(Some(id)) => std::slice::from_ref(id),
            _ => &[],
        }
    }

    /// Sets the field, and says what changed. A duplicate is dropped.
    fn apply(self, i: &mut Issue) -> String {
        match self {
            Field::Severity(s) => {
                i.severity = s;
                s.map_or("Cleared severity.".into(), |s| {
                    format!("Severity {}.", s.name())
                })
            }
            Field::Revision(r) => {
                let note = r
                    .as_ref()
                    .map_or("Cleared revision.".into(), |r| format!("Revision {r}."));
                i.revision = r;
                note
            }
            Field::BlockedBy(ids) => {
                let note = if ids.is_empty() {
                    "No longer blocked.".into()
                } else {
                    let ids: Vec<String> = ids.iter().map(|b| format!("#{b}")).collect();
                    format!("Blocked by {}.", ids.join(", "))
                };
                i.blocked_by = ids;
                note
            }
            Field::DuplicateOf(d) => {
                i.duplicate_of = d;
                match d {
                    Some(d) => {
                        i.status = Status::Dropped;
                        format!("Duplicate of #{d}.")
                    }
                    None => "Not a duplicate.".into(),
                }
            }
        }
    }
}

/// Refuses a field naming issue `id` itself or one that doesn't exist.
fn check(folder: &Folder, id: Option<u32>, field: &Field) -> Result<()> {
    for &other in field.ids() {
        if Some(other) == id {
            bail!("issue #{other} can't point at itself");
        }
        folder.path(other)?;
    }
    Ok(())
}

/// Changes a header field, logging what changed.
pub fn set(folder: &Folder, id: u32, who: &str, field: Field) -> Result<()> {
    check(folder, Some(id), &field)?;
    folder.update(id, |i| {
        let note = field.apply(i);
        i.push(who, Kind::Note, &note, time::now());
        Ok(())
    })
}

pub fn claim(folder: &Folder, id: u32, who: &str, term: Option<String>) -> Result<()> {
    folder.update(id, |i| {
        i.owner = Some(who.into());
        i.term = term.or(i.term.take());
        if i.status == Status::Open {
            i.status = Status::Working;
        }
        i.push(who, Kind::Note, "Claimed.", time::now());
        Ok(())
    })
}

pub fn note(folder: &Folder, id: u32, who: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("nothing to log");
    }
    folder.update(id, |i| {
        i.push(who, Kind::Note, text, time::now());
        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn ask(
    folder: &Folder,
    id: u32,
    qid: &str,
    after: Option<String>,
    who: &str,
    term: Option<String>,
    text: &str,
) -> Result<()> {
    if text.trim().is_empty() {
        bail!("the question is empty");
    }
    folder.update(id, |i| {
        let kind = Kind::Question {
            id: qid.into(),
            after,
        };
        i.push(who, kind, text, time::now());
        if i.term.is_none() || i.owner.as_deref() == Some(who) {
            i.term = term.or(i.term.take());
        }
        i.status = Status::NeedsInput;
        Ok(())
    })
}

/// How the user answered: an option with notes, or words instead.
#[derive(Clone, Debug, PartialEq)]
pub enum Response {
    Pick { option: usize, notes: String },
    Reply(String),
}

impl Response {
    fn entry(&self, qid: &str) -> (Kind, String) {
        match self {
            Response::Pick { option, notes } if notes.is_empty() => {
                (Kind::Answer { id: qid.into() }, option.to_string())
            }
            Response::Pick { option, notes } => (
                Kind::Answer { id: qid.into() },
                format!("{option}\n{notes}"),
            ),
            Response::Reply(text) => (Kind::Reply { id: qid.into() }, text.clone()),
        }
    }
}

/// Records the answer to `qid` and says what to send to the agent.
pub fn answer(
    folder: &Folder,
    id: u32,
    qid: &str,
    who: &str,
    response: &Response,
) -> Result<Option<Delivery>> {
    answer_at(folder, id, qid, who, response, time::now())
}

fn answer_at(
    folder: &Folder,
    id: u32,
    qid: &str,
    who: &str,
    response: &Response,
    at: Minute,
) -> Result<Option<Delivery>> {
    if matches!(response, Response::Reply(t) if t.trim().is_empty()) {
        bail!("the answer is empty");
    }
    let root = folder.root.display().to_string();
    folder.update(id, |i| {
        if !i.questions().iter().any(|(q, _)| q.qid() == Some(qid)) {
            bail!("issue #{id} has no question {qid}");
        }
        let (kind, text) = response.entry(qid);
        i.push(who, kind, &text, at.max(i.touched()));
        if i.status == Status::NeedsInput && i.waiting().is_empty() {
            i.status = Status::Working;
        }
        let next: Vec<String> = i
            .waiting()
            .iter()
            .filter_map(|q| q.qid().map(str::to_owned))
            .collect();
        Ok(i.term.clone().map(|to| Delivery {
            to,
            text: message(i, qid, response, &next, &root),
        }))
    })
}

fn message(issue: &Issue, qid: &str, response: &Response, next: &[String], root: &str) -> String {
    let head = format!("Issue #{} \"{}\" ({root})", issue.id, issue.title);
    let mut out = match response {
        Response::Pick { option, notes } if notes.is_empty() => {
            format!("{head}, answer to {qid}: option {option}.")
        }
        Response::Pick { option, notes } => {
            format!("{head}, answer to {qid}: option {option}. Notes: {notes}")
        }
        Response::Reply(text) => format!(
            "{head}, reply to {qid} instead of an option. {qid} is closed; rethink it with this \
             and ask a new question if you still need one:\n{text}"
        ),
    };
    if !next.is_empty() {
        out += &format!("\nStill waiting: {}.", next.join(", "));
    }
    out
}

pub fn set_status(folder: &Folder, id: u32, status: Status, who: &str, note: &str) -> Result<()> {
    folder.update(id, |i| {
        i.status = status;
        let text = if note.trim().is_empty() {
            match status {
                Status::Done => "Closed.",
                Status::Dropped => "Dropped.",
                _ => "Reopened.",
            }
        } else {
            note
        };
        i.push(who, Kind::Note, text, time::now());
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(option: usize) -> Response {
        Response::Pick {
            option,
            notes: String::new(),
        }
    }

    fn setup() -> (tempfile::TempDir, Folder, u32) {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        let id = file(&folder, "Login loops", "", "orchestrator", vec![]).unwrap();
        claim(&folder, id, "auth-fixer", Some("t12".into())).unwrap();
        (dir, folder, id)
    }

    #[test]
    fn asking_marks_it_needing_input_and_answering_hands_it_back() {
        let (_dir, folder, id) = setup();
        ask(
            &folder,
            id,
            "Q1",
            None,
            "auth-fixer",
            Some("t12".into()),
            "Where?\n\nOptions:\n\n1. /login",
        )
        .unwrap();
        ask(
            &folder,
            id,
            "Q2",
            Some("Q1".into()),
            "auth-fixer",
            None,
            "Toast copy",
        )
        .unwrap();
        assert_eq!(folder.get(id).unwrap().status, Status::NeedsInput);

        let sent = answer(
            &folder,
            id,
            "Q1",
            "jordan",
            &Response::Pick {
                option: 1,
                notes: "keep it short".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(sent.to, "t12");
        assert!(
            sent.text
                .contains("answer to Q1: option 1. Notes: keep it short"),
            "{}",
            sent.text
        );
        assert!(sent.text.contains("Still waiting: Q2."), "{}", sent.text);
        assert_eq!(folder.get(id).unwrap().status, Status::NeedsInput);

        let sent = answer(
            &folder,
            id,
            "Q2",
            "jordan",
            &Response::Reply("Neither".into()),
        )
        .unwrap()
        .unwrap();
        assert!(
            sent.text.contains("reply to Q2 instead of an option"),
            "{}",
            sent.text
        );
        assert!(!sent.text.contains("Still waiting"));
        assert_eq!(folder.get(id).unwrap().status, Status::Working);
    }

    #[test]
    fn answers_to_unknown_questions_and_empty_text_are_refused() {
        let (_dir, folder, id) = setup();
        assert!(answer(&folder, id, "Q9", "jordan", &pick(1)).is_err());
        assert!(answer(&folder, id, "Q1", "jordan", &Response::Reply(" ".into())).is_err());
        assert!(ask(&folder, id, "Q1", None, "a", None, "  ").is_err());
        assert!(file(&folder, " ", "", "a", vec![]).is_err());
    }

    #[test]
    fn fields_point_only_at_other_issues_that_exist_and_a_duplicate_is_dropped() {
        let (_dir, folder, id) = setup();
        let blocked = Field::BlockedBy(vec![id]);
        let other = file(&folder, "Toast copy", "", "a", vec![blocked]).unwrap();
        assert_eq!(folder.get(other).unwrap().blocked_by, [id]);
        assert!(set(&folder, other, "a", Field::BlockedBy(vec![other])).is_err());
        assert!(set(&folder, other, "a", Field::DuplicateOf(Some(99))).is_err());
        assert!(file(&folder, "x", "", "a", vec![Field::BlockedBy(vec![99])]).is_err());

        set(&folder, other, "a", Field::DuplicateOf(Some(id))).unwrap();
        let issue = folder.get(other).unwrap();
        assert_eq!(
            (issue.duplicate_of, issue.status),
            (Some(id), Status::Dropped)
        );
        assert_eq!(issue.latest().unwrap().text, format!("Duplicate of #{id}."));
    }

    #[test]
    fn set_arguments_name_a_field_and_an_empty_value_clears_it() {
        let f = |n, v| Field::parse(n, v).unwrap();
        assert_eq!(f("severity", "high"), Field::Severity(Some(Severity::High)));
        assert_eq!(f("severity", ""), Field::Severity(None));
        assert_eq!(f("blocked-by", "#3, 4"), Field::BlockedBy(vec![3, 4]));
        assert_eq!(f("duplicate-of", "#5"), Field::DuplicateOf(Some(5)));
        assert_eq!(
            f("revision", " wytk "),
            Field::Revision(Some("wytk".into()))
        );
        assert!(Field::parse("severity", "meh").is_err());
        assert!(Field::parse("duplicate-of", "3 4").is_err());
        assert!(Field::parse("owner", "x").is_err());
    }

    #[test]
    fn an_answer_written_in_the_same_minute_as_the_question_still_counts() {
        let (_dir, folder, id) = setup();
        ask(&folder, id, "Q1", None, "auth-fixer", None, "Where?").unwrap();
        answer_at(&folder, id, "Q1", "jordan", &pick(1), 0).unwrap();
        assert!(folder.get(id).unwrap().waiting().is_empty());
    }
}
