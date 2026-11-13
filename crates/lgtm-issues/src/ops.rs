//! What can happen to an issue, shared by the CLI and the TUI.

use anyhow::{Context, Result, bail};

use crate::fields::{self, Kind};
use crate::folder::Folder;
use crate::header;
use crate::issue::{Issue, Kind as LogKind};
use crate::time::{self, Minute};
use crate::workflow::{self, Verb, Workflow};

/// A message to hand to an agent: an answer to the one that asked, or word that an issue
/// moved to a status that notifies it. Its terminal, and the message.
#[derive(Debug)]
pub struct Delivery {
    pub to: String,
    pub text: String,
}

pub fn file(
    folder: &Folder,
    title: &str,
    body: &str,
    who: &str,
    status: Option<&str>,
    with: Vec<(String, String)>,
) -> Result<(u32, Option<Delivery>)> {
    if title.trim().is_empty() {
        bail!("an issue needs a title");
    }
    let wf = folder.workflow()?;
    let status = match status {
        Some(s) => wf.find(s)?,
        None => wf.verb(Verb::New)?,
    };
    let mut issue = Issue::new(0, title, Some(who.into()), body, time::now());
    issue.status = status.id.clone();
    if let Some(template) = &status.template {
        let (pairs, text) = header::split(template).map_err(anyhow::Error::msg)?;
        for (k, v) in pairs
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "title" | "status"))
        {
            issue.set_pair(k, v).map_err(anyhow::Error::msg)?;
        }
        if issue.body.is_empty() {
            issue.body = text.trim().to_owned();
        }
    }
    for (name, value) in with {
        apply(folder, &wf, &mut issue, &name, &value)?;
    }
    let to = wf.status(&issue.status).and_then(|s| s.notify.clone());
    let id = folder.create(issue)?;
    Ok((id, to.map(|to| notice(folder, id, title, &wf, to))))
}

/// Tells the terminal a status notifies that issue `id` moved into it.
fn notice(folder: &Folder, id: u32, title: &str, wf: &Workflow, to: String) -> Delivery {
    let label = folder
        .get(id)
        .ok()
        .and_then(|i| wf.status(&i.status).map(|s| s.label.clone()))
        .unwrap_or_default();
    Delivery {
        to,
        text: format!(
            "Issue #{id} \"{}\" ({}) moved to {label}.",
            title.trim(),
            folder.root.display()
        ),
    }
}

/// Sets field `name` on `issue`, checked against the workflow; an empty value clears it.
/// Says what changed. A duplicate moves to where dropped issues go.
fn apply(
    folder: &Folder,
    wf: &Workflow,
    issue: &mut Issue,
    name: &str,
    value: &str,
) -> Result<String> {
    let value = value.trim();
    let field = wf.field(name).with_context(|| {
        let names: Vec<&str> = wf.fields.iter().map(|f| f.name.as_str()).collect();
        format!("no field {name}: {}", names.join(", "))
    })?;
    let check = |ids: &[u32]| -> Result<()> {
        for &other in ids {
            if other == issue.id && issue.id != 0 {
                bail!("issue #{other} can't point at itself");
            }
            folder.path(other)?;
        }
        Ok(())
    };
    let some = || Some(value.to_owned()).filter(|v| !v.is_empty());
    Ok(match &field.kind {
        Kind::Revision => {
            issue.revision = some();
            some().map_or("Cleared revision.".into(), |r| format!("Revision {r}."))
        }
        Kind::BlockedBy => {
            let ids = fields::parse_ids(value).map_err(anyhow::Error::msg)?;
            check(&ids)?;
            issue.blocked_by = ids.clone();
            if ids.is_empty() {
                "No longer blocked.".into()
            } else {
                let ids: Vec<String> = ids.iter().map(|b| format!("#{b}")).collect();
                format!("Blocked by {}.", ids.join(", "))
            }
        }
        Kind::DuplicateOf => match fields::parse_ids(value).map_err(anyhow::Error::msg)?[..] {
            [] => {
                issue.duplicate_of = None;
                "Not a duplicate.".into()
            }
            [d] => {
                check(&[d])?;
                issue.duplicate_of = Some(d);
                issue.status = wf.verb(Verb::Drop)?.id.clone();
                format!("Duplicate of #{d}.")
            }
            _ => bail!("an issue duplicates one other issue"),
        },
        Kind::Choice(choices) => {
            if !value.is_empty() && field.rank(value).is_none() {
                let values: Vec<&str> = choices.iter().map(|c| c.value.as_str()).collect();
                bail!("{name} is {}, not {value}", values.join(", "));
            }
            issue.set_field(name, some());
            some().map_or(format!("Cleared {name}."), |v| {
                format!("{} {v}.", capital(name))
            })
        }
        Kind::Text => {
            issue.set_field(name, some());
            some().map_or(format!("Cleared {name}."), |v| {
                format!("{} {v}.", capital(name))
            })
        }
    })
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Changes a header field, logging what changed.
pub fn set(folder: &Folder, id: u32, who: &str, name: &str, value: &str) -> Result<()> {
    let wf = folder.workflow()?;
    folder.update(id, |i| {
        let note = apply(folder, &wf, i, name, value)?;
        i.push(who, LogKind::Note, &note, time::now());
        Ok(())
    })
}

/// Moves issue `id` to `status` (an id or a label), refusing when the status's `requires`
/// aren't met unless `force`. Logs `note` when there is one. Says whom to notify.
pub fn move_to(
    folder: &Folder,
    id: u32,
    status: &str,
    force: bool,
    note: Option<(&str, &str)>,
) -> Result<Option<Delivery>> {
    let wf = folder.workflow()?;
    let to = wf.find(status)?.clone();
    let (all, _) = folder.load()?;
    let issue = all
        .iter()
        .find(|i| i.id == id)
        .with_context(|| format!("no issue #{id}"))?;
    let unmet = wf.unmet(issue, &all, &to);
    if !force && !unmet.is_empty() {
        bail!(
            "#{id} can't move to {}: {}",
            to.label,
            needs(issue, &all, &wf, &unmet)
        );
    }
    let moved = issue.status != to.id;
    let title = issue.title.clone();
    folder.update(id, |i| {
        i.status = to.id.clone();
        if let Some((who, text)) = note {
            i.push(who, LogKind::Note, text, time::now());
        }
        Ok(())
    })?;
    Ok(to
        .notify
        .clone()
        .filter(|_| moved)
        .map(|n| notice(folder, id, &title, &wf, n)))
}

/// What an issue lacks, in words: `needs revision`, `blocked by #3`.
pub fn needs(issue: &Issue, all: &[Issue], wf: &Workflow, unmet: &[String]) -> String {
    unmet
        .iter()
        .map(|r| {
            if r == workflow::UNBLOCKED {
                let ids: Vec<String> = issue
                    .open_blockers(all, |s| wf.is_done(s))
                    .iter()
                    .map(|b| format!("#{b}"))
                    .collect();
                format!("blocked by {}", ids.join(", "))
            } else {
                format!("needs {r}")
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Moves an issue the way a verb does, logging `note` or the verb's own.
pub fn verb(
    folder: &Folder,
    id: u32,
    verb: Verb,
    who: &str,
    note: &str,
    force: bool,
) -> Result<Option<Delivery>> {
    let to = folder.workflow()?.verb(verb)?.id.clone();
    let text = match note.trim() {
        "" => match verb {
            Verb::Close => "Closed.",
            Verb::Drop => "Dropped.",
            _ => "Reopened.",
        },
        n => n,
    };
    move_to(folder, id, &to, force, Some((who, text)))
}

/// Makes the issue `who`'s, and moves it to the claim status when it's in one before that.
/// A blocked issue is refused unless `force`.
pub fn claim(
    folder: &Folder,
    id: u32,
    who: &str,
    term: Option<String>,
    force: bool,
) -> Result<Option<Delivery>> {
    let wf = folder.workflow()?;
    let (all, _) = folder.load()?;
    let issue = all
        .iter()
        .find(|i| i.id == id)
        .with_context(|| format!("no issue #{id}"))?;
    let blockers = issue.open_blockers(&all, |s| wf.is_done(s));
    if !force && !blockers.is_empty() {
        let ids: Vec<String> = blockers.iter().map(|b| format!("#{b}")).collect();
        bail!(
            "#{id} is blocked by {}; --force claims it anyway",
            ids.join(", ")
        );
    }
    let to = wf.verb(Verb::Claim)?;
    let pos = |s: &str| wf.statuses.iter().position(|x| x.id == s);
    let before = pos(&issue.status)
        .zip(pos(&to.id))
        .is_some_and(|(a, b)| a < b);
    folder.update(id, |i| {
        i.owner = Some(who.into());
        i.term = term.or(i.term.take());
        i.push(who, LogKind::Note, "Claimed.", time::now());
        Ok(())
    })?;
    if before {
        return move_to(folder, id, &to.id, true, None);
    }
    Ok(None)
}

pub fn note(folder: &Folder, id: u32, who: &str, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        bail!("nothing to log");
    }
    folder.update(id, |i| {
        i.push(who, LogKind::Note, text, time::now());
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
        let kind = LogKind::Question {
            id: qid.into(),
            after,
        };
        i.push(who, kind, text, time::now());
        if i.term.is_none() || i.owner.as_deref() == Some(who) {
            i.term = term.or(i.term.take());
        }
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
    fn entry(&self, qid: &str) -> (LogKind, String) {
        match self {
            Response::Pick { option, notes } if notes.is_empty() => {
                (LogKind::Answer { id: qid.into() }, option.to_string())
            }
            Response::Pick { option, notes } => (
                LogKind::Answer { id: qid.into() },
                format!("{option}\n{notes}"),
            ),
            Response::Reply(text) => (LogKind::Reply { id: qid.into() }, text.clone()),
        }
    }
}

/// A reply that's only one of the question's option numbers picks that option.
fn as_pick(response: &Response, question: &str) -> Response {
    if let Response::Reply(text) = response
        && let Ok(option) = text.trim().parse::<usize>()
        && crate::tui::qq::options(question)
            .iter()
            .any(|(n, _)| *n == option)
    {
        return Response::Pick {
            option,
            notes: String::new(),
        };
    }
    response.clone()
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
        let Some((q, _)) = i
            .questions()
            .into_iter()
            .find(|(q, _)| q.qid() == Some(qid))
        else {
            bail!("issue #{id} has no question {qid}");
        };
        let response = &as_pick(response, &q.text);
        let (kind, text) = response.entry(qid);
        i.push(who, kind, &text, at.max(i.touched()));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn pick(option: usize) -> Response {
        Response::Pick {
            option,
            notes: String::new(),
        }
    }

    fn new(folder: &Folder, title: &str) -> u32 {
        file(folder, title, "", "orchestrator", None, vec![])
            .unwrap()
            .0
    }

    fn setup() -> (tempfile::TempDir, Folder, u32) {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        let id = new(&folder, "Login loops");
        claim(&folder, id, "auth-fixer", Some("t12".into()), false).unwrap();
        (dir, folder, id)
    }

    /// Adds a `review` status between working and done.
    fn add_review(folder: &Folder, header: &str) {
        let dir = folder.root.join("issues/review");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("STATUS.md"), format!("---\n{header}\n---\n")).unwrap();
        let wf = folder.root.join("issues/workflow.md");
        let text = fs::read_to_string(&wf).unwrap();
        fs::write(&wf, text.replace("working, done", "working, review, done")).unwrap();
    }

    #[test]
    fn asking_and_answering_send_the_answer_without_moving_the_issue() {
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
        assert_eq!(folder.get(id).unwrap().status, "working");

        let notes = Response::Pick {
            option: 1,
            notes: "keep it short".into(),
        };
        let sent = answer(&folder, id, "Q1", "jordan", &notes)
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

        let reply = Response::Reply("Neither".into());
        let sent = answer(&folder, id, "Q2", "jordan", &reply)
            .unwrap()
            .unwrap();
        assert!(
            sent.text.contains("reply to Q2 instead of an option"),
            "{}",
            sent.text
        );
        assert!(!sent.text.contains("Still waiting"));
        assert_eq!(folder.get(id).unwrap().status, "working");
    }

    #[test]
    fn answers_to_unknown_questions_and_empty_text_are_refused() {
        let (_dir, folder, id) = setup();
        assert!(answer(&folder, id, "Q9", "jordan", &pick(1)).is_err());
        assert!(answer(&folder, id, "Q1", "jordan", &Response::Reply(" ".into())).is_err());
        assert!(ask(&folder, id, "Q1", None, "a", None, "  ").is_err());
        assert!(file(&folder, " ", "", "a", None, vec![]).is_err());
    }

    #[test]
    fn fields_are_checked_against_the_workflow_and_a_duplicate_is_dropped() {
        let (_dir, folder, id) = setup();
        let with = vec![("blocked-by".into(), id.to_string())];
        let (other, _) = file(&folder, "Toast copy", "", "a", None, with).unwrap();
        assert_eq!(folder.get(other).unwrap().blocked_by, [id]);
        assert!(set(&folder, other, "a", "blocked-by", &other.to_string()).is_err());
        assert!(set(&folder, other, "a", "duplicate-of", "99").is_err());
        assert!(set(&folder, other, "a", "severity", "meh").is_err());
        assert!(set(&folder, other, "a", "area", "ui").is_err());

        set(&folder, other, "a", "severity", "high").unwrap();
        assert_eq!(
            folder.get(other).unwrap().field("severity").as_deref(),
            Some("high")
        );
        set(&folder, other, "a", "duplicate-of", &format!("#{id}")).unwrap();
        let issue = folder.get(other).unwrap();
        assert_eq!(
            (issue.duplicate_of, issue.status.as_str()),
            (Some(id), "dropped")
        );
        assert_eq!(issue.latest().unwrap().text, format!("Duplicate of #{id}."));
    }

    #[test]
    fn a_field_the_workflow_lists_takes_its_values_from_there() {
        let (_dir, folder, id) = setup();
        let wf = folder.root.join("issues/workflow.md");
        let text = fs::read_to_string(&wf).unwrap().replace(
            "fields: severity,",
            "area: ui, core\nfields: area, notes, severity,",
        );
        fs::write(&wf, text).unwrap();
        assert!(set(&folder, id, "a", "area", "db").is_err());
        set(&folder, id, "a", "area", "core").unwrap();
        set(&folder, id, "a", "notes", "flaky on CI").unwrap();
        let issue = folder.get(id).unwrap();
        assert_eq!(issue.field("area").as_deref(), Some("core"));
        assert_eq!(issue.latest().unwrap().text, "Notes flaky on CI.");
        set(&folder, id, "a", "area", "").unwrap();
        assert_eq!(folder.get(id).unwrap().field("area"), None);
    }

    #[test]
    fn verbs_move_where_the_workflow_says_and_requires_holds_a_move_back() {
        let (_dir, folder, id) = setup();
        add_review(&folder, "requires: revision\nnotify: reviewer");
        let err = move_to(&folder, id, "review", false, None).unwrap_err();
        assert!(err.to_string().contains("needs revision"), "{err}");
        set(&folder, id, "a", "revision", "wytk").unwrap();
        let sent = move_to(&folder, id, "review", false, None)
            .unwrap()
            .unwrap();
        assert_eq!(sent.to, "reviewer");
        assert!(
            sent.text
                .starts_with(&format!("Issue #{id} \"Login loops\"")),
            "{}",
            sent.text
        );
        assert!(
            move_to(&folder, id, "review", false, None)
                .unwrap()
                .is_none()
        );

        verb(&folder, id, Verb::Close, "a", "", false).unwrap();
        let issue = folder.get(id).unwrap();
        assert_eq!(
            (issue.status.as_str(), issue.latest().unwrap().text.as_str()),
            ("done", "Closed.")
        );
        verb(&folder, id, Verb::Reopen, "a", "Came back.", false).unwrap();
        assert_eq!(folder.get(id).unwrap().status, "open");
    }

    #[test]
    fn claim_moves_only_forward_and_refuses_a_blocked_issue_unless_forced() {
        let (_dir, folder, id) = setup();
        add_review(&folder, "");
        move_to(&folder, id, "review", false, None).unwrap();
        claim(&folder, id, "reviewer", None, false).unwrap();
        let issue = folder.get(id).unwrap();
        assert_eq!(
            (issue.status.as_str(), issue.owner.as_deref()),
            ("review", Some("reviewer"))
        );

        let with = vec![("blocked-by".into(), id.to_string())];
        let (blocked, _) = file(&folder, "After", "", "a", None, with).unwrap();
        assert!(claim(&folder, blocked, "b", None, false).is_err());
        claim(&folder, blocked, "b", None, true).unwrap();
        assert_eq!(folder.get(blocked).unwrap().status, "working");
    }

    #[test]
    fn a_new_issue_starts_from_its_status_template() {
        let (_dir, folder, _) = setup();
        fs::write(
            folder.root.join("issues/open/TEMPLATE.md"),
            "---\nseverity: medium\n---\n\n## Steps\n\n## Expected\n",
        )
        .unwrap();
        let with = vec![("severity".into(), "high".into())];
        let (id, _) = file(&folder, "Crash", "", "a", None, with).unwrap();
        let issue = folder.get(id).unwrap();
        assert_eq!(issue.field("severity").as_deref(), Some("high"));
        assert_eq!(issue.body, "## Steps\n\n## Expected");
        let (id, _) = file(&folder, "Typo", "Fix it", "a", Some("working"), vec![]).unwrap();
        let issue = folder.get(id).unwrap();
        assert_eq!(
            (issue.status.as_str(), issue.body.as_str()),
            ("working", "Fix it")
        );
    }

    #[test]
    fn a_reply_of_just_an_option_number_picks_it() {
        let (_dir, folder, id) = setup();
        let q = "Q1 — Where\n\nOptions:\n\n1. /login (recommended)\n2. /home\n";
        ask(&folder, id, "Q1", None, "auth-fixer", Some("t12".into()), q).unwrap();
        ask(&folder, id, "Q2", None, "auth-fixer", None, q).unwrap();
        let sent = answer(&folder, id, "Q1", "jordan", &Response::Reply(" 2 ".into()))
            .unwrap()
            .unwrap();
        assert!(
            sent.text.contains("answer to Q1: option 2."),
            "{}",
            sent.text
        );
        let sent = answer(&folder, id, "Q2", "jordan", &Response::Reply("3".into()))
            .unwrap()
            .unwrap();
        assert!(sent.text.contains("reply to Q2"), "{}", sent.text);
    }

    #[test]
    fn an_answer_written_in_the_same_minute_as_the_question_still_counts() {
        let (_dir, folder, id) = setup();
        ask(&folder, id, "Q1", None, "auth-fixer", None, "Where?").unwrap();
        answer_at(&folder, id, "Q1", "jordan", &pick(1), 0).unwrap();
        assert!(folder.get(id).unwrap().waiting().is_empty());
    }
}
