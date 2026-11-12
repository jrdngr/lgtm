//! One issue and its markdown file: a `key: value` header between `---` lines, the
//! description, then a log of `## <time> <who>` entries. Questions and answers carry an id:
//! `## <time> <who> question Q3 [after Q2]`, `## <time> <who> answer Q3` (the option number,
//! then notes) and `## <time> <who> reply Q3` (words instead of an option).

use crate::time::{self, Minute};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Open,
    Working,
    NeedsInput,
    Done,
    Dropped,
}

impl Status {
    pub const ALL: [Status; 5] = [
        Status::Open,
        Status::Working,
        Status::NeedsInput,
        Status::Done,
        Status::Dropped,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Working => "working",
            Status::NeedsInput => "needs-input",
            Status::Done => "done",
            Status::Dropped => "dropped",
        }
    }

    pub fn parse(s: &str) -> Option<Status> {
        Status::ALL.into_iter().find(|st| st.name() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Note,
    /// A question, and the one it waits on.
    Question {
        id: String,
        after: Option<String>,
    },
    Answer {
        id: String,
    },
    /// Words instead of an option: the question is closed for a new one.
    Reply {
        id: String,
    },
}

/// Where a question stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    Waiting,
    /// Waits on another question's answer first.
    Queued,
    Answered,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub at: Minute,
    pub who: String,
    pub kind: Kind,
    pub text: String,
}

impl Entry {
    /// A question's id.
    pub fn qid(&self) -> Option<&str> {
        match &self.kind {
            Kind::Question { id, .. } => Some(id),
            _ => None,
        }
    }

    /// What a question is about: the subject of its `Q3 — <subject>` line, else its first line.
    pub fn subject(&self) -> &str {
        let marked = self.qid().and_then(|id| {
            self.text.lines().find_map(|l| {
                let rest = l.trim().strip_prefix(id)?.trim_start();
                rest.strip_prefix('—')
                    .or_else(|| rest.strip_prefix("--"))
                    .or_else(|| rest.strip_prefix('-'))
                    .map(str::trim)
            })
        });
        marked.unwrap_or_else(|| self.text.lines().next().unwrap_or_default().trim())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub id: u32,
    pub title: String,
    pub status: Status,
    /// The agent working on it, by its lgterm tab name.
    pub owner: Option<String>,
    /// The owner's lgterm terminal, where answers go.
    pub term: Option<String>,
    pub filed_by: Option<String>,
    pub created: Minute,
    pub body: String,
    pub log: Vec<Entry>,
}

impl Issue {
    pub fn new(id: u32, title: &str, filed_by: Option<String>, body: &str, at: Minute) -> Self {
        Issue {
            id,
            title: title.trim().to_owned(),
            status: Status::Open,
            owner: None,
            term: None,
            filed_by,
            created: at,
            body: body.trim().to_owned(),
            log: Vec::new(),
        }
    }

    /// When anything last happened to it.
    pub fn touched(&self) -> Minute {
        self.log
            .last()
            .map_or(self.created, |e| e.at.max(self.created))
    }

    /// Questions in the order asked, the last asking of an id winning.
    pub fn questions(&self) -> Vec<(&Entry, Asked)> {
        let mut out: Vec<&Entry> = Vec::new();
        for e in &self.log {
            if let Some(id) = e.qid() {
                out.retain(|q| q.qid() != Some(id));
                out.push(e);
            }
        }
        out.into_iter().map(|q| (q, self.asked(q))).collect()
    }

    fn asked(&self, q: &Entry) -> Asked {
        let Kind::Question { id, after } = &q.kind else {
            return Asked::Answered;
        };
        let answered = |id: &str| {
            self.answer_to(id)
                .is_some_and(|a| a.at >= self.asked_at(id).unwrap_or(0))
        };
        if answered(id) {
            Asked::Answered
        } else if after.as_deref().is_some_and(|a| !answered(a)) {
            Asked::Queued
        } else {
            Asked::Waiting
        }
    }

    fn asked_at(&self, qid: &str) -> Option<Minute> {
        self.log
            .iter()
            .rev()
            .find(|e| e.qid() == Some(qid))
            .map(|e| e.at)
    }

    /// The answer or reply given to question `qid`.
    pub fn answer_to(&self, qid: &str) -> Option<&Entry> {
        self.log
            .iter()
            .rev()
            .find(|e| matches!(&e.kind, Kind::Answer { id } | Kind::Reply { id } if id == qid))
    }

    /// Questions waiting on the user now.
    pub fn waiting(&self) -> Vec<&Entry> {
        self.questions()
            .into_iter()
            .filter(|(_, a)| *a == Asked::Waiting)
            .map(|(q, _)| q)
            .collect()
    }

    pub fn latest(&self) -> Option<&Entry> {
        self.log.last()
    }

    pub fn push(&mut self, who: &str, kind: Kind, text: &str, at: Minute) {
        self.log.push(Entry {
            at,
            who: who.trim().to_owned(),
            kind,
            text: text.trim().to_owned(),
        });
    }

    /// The file's name: `12-login-redirect-loops.md`.
    pub fn file_name(&self) -> String {
        let mut slug = String::new();
        for c in self.title.chars().flat_map(char::to_lowercase) {
            if c.is_ascii_alphanumeric() {
                slug.push(c);
            } else if !slug.ends_with('-') && !slug.is_empty() {
                slug.push('-');
            }
            if slug.len() >= 40 {
                break;
            }
        }
        format!("{}-{}.md", self.id, slug.trim_end_matches('-'))
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "---\ntitle: {}\nstatus: {}\n",
            self.title,
            self.status.name()
        );
        for (key, value) in [
            ("owner", &self.owner),
            ("term", &self.term),
            ("filed-by", &self.filed_by),
        ] {
            if let Some(v) = value {
                out += &format!("{key}: {v}\n");
            }
        }
        out += &format!("created: {}\n---\n", time::format(self.created));
        if !self.body.is_empty() {
            out += &format!("\n{}\n", self.body);
        }
        for e in &self.log {
            let kind = match &e.kind {
                Kind::Note => String::new(),
                Kind::Question { id, after: None } => format!(" question {id}"),
                Kind::Question { id, after: Some(a) } => format!(" question {id} after {a}"),
                Kind::Answer { id } => format!(" answer {id}"),
                Kind::Reply { id } => format!(" reply {id}"),
            };
            out += &format!("\n## {} {}{kind}\n{}\n", time::format(e.at), e.who, e.text);
        }
        out
    }

    /// Reads a file's text. `id` comes from the file name.
    pub fn parse(id: u32, text: &str) -> Result<Issue, String> {
        let rest = text
            .strip_prefix("---\n")
            .ok_or("doesn't start with a --- header")?;
        let (head, rest) = rest
            .split_once("\n---\n")
            .or_else(|| rest.strip_suffix("\n---").map(|h| (h, "")))
            .ok_or("its header has no closing ---")?;
        let mut issue = Issue::new(id, "", None, "", 0);
        for line in head.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim().to_owned();
            match key.trim() {
                "title" => issue.title = value,
                "status" => {
                    issue.status =
                        Status::parse(&value).ok_or_else(|| format!("unknown status {value}"))?
                }
                "owner" => issue.owner = Some(value).filter(|v| !v.is_empty()),
                "term" => issue.term = Some(value).filter(|v| !v.is_empty()),
                "filed-by" => issue.filed_by = Some(value).filter(|v| !v.is_empty()),
                "created" => issue.created = time::parse(&value).unwrap_or(0),
                _ => {}
            }
        }
        if issue.title.is_empty() {
            return Err("no title".into());
        }
        let mut body = Vec::new();
        let mut current: Option<(Entry, Vec<&str>)> = None;
        for line in rest.lines() {
            if let Some(entry) = heading(line) {
                if let Some((e, lines)) = current.take() {
                    issue.log.push(finish(e, &lines));
                }
                current = Some((entry, Vec::new()));
            } else if let Some((_, lines)) = current.as_mut() {
                lines.push(line);
            } else {
                body.push(line);
            }
        }
        if let Some((e, lines)) = current {
            issue.log.push(finish(e, &lines));
        }
        issue.body = body.join("\n").trim().to_owned();
        Ok(issue)
    }
}

fn finish(mut e: Entry, lines: &[&str]) -> Entry {
    e.text = lines.join("\n").trim().to_owned();
    e
}

/// A question or answer id: `Q` and digits, with an optional letter, like `Q4b`.
fn is_qid(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('Q') else {
        return false;
    };
    let digits = rest.trim_end_matches(|c: char| c.is_ascii_lowercase());
    !digits.is_empty()
        && digits.chars().all(|c| c.is_ascii_digit())
        && rest.len() - digits.len() <= 1
}

/// A log entry's heading, `## 2026-11-12T23:06Z auth-fixer question Q3 after Q2`.
fn heading(line: &str) -> Option<Entry> {
    let rest = line.strip_prefix("## ")?;
    let (stamp, who) = rest.split_once(' ')?;
    let at = time::parse(stamp)?;
    let words: Vec<&str> = who.split_whitespace().collect();
    let (who, kind) = match words.as_slice() {
        [w @ .., "question", id, "after", a] if is_qid(id) && is_qid(a) => (
            w,
            Kind::Question {
                id: (*id).into(),
                after: Some((*a).into()),
            },
        ),
        [w @ .., "question", id] if is_qid(id) => (
            w,
            Kind::Question {
                id: (*id).into(),
                after: None,
            },
        ),
        [w @ .., "answer", id] if is_qid(id) => (w, Kind::Answer { id: (*id).into() }),
        [w @ .., "reply", id] if is_qid(id) => (w, Kind::Reply { id: (*id).into() }),
        w => (w, Kind::Note),
    };
    if who.is_empty() {
        return None;
    }
    Some(Entry {
        at,
        who: who.join(" "),
        kind,
        text: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "---
title: Login redirect loops
status: needs-input
owner: auth fixer
term: t12
filed-by: orchestrator
created: 2026-11-12T22:40Z
---

Expired cookies loop.

## Steps
1. log in

## 2026-11-12T22:52Z auth fixer
Reproduced.

## 2026-11-12T22:55Z auth fixer question Q1
Fix scope

## 2026-11-12T22:57Z jordan answer Q1
1

## 2026-11-12T23:03Z auth fixer question Q2
Where an expired session goes

Options:

1. /login (recommended)

## 2026-11-12T23:03Z auth fixer question Q3 after Q2
Toast copy
";

    fn issue() -> Issue {
        Issue::parse(12, FILE).unwrap()
    }

    #[test]
    fn a_file_reads_back_as_written() {
        assert_eq!(issue().render(), FILE);
    }

    #[test]
    fn plain_headings_stay_in_the_description() {
        let issue = issue();
        assert_eq!(issue.body, "Expired cookies loop.\n\n## Steps\n1. log in");
        assert_eq!(issue.log[0].who, "auth fixer");
        assert_eq!(issue.log[0].kind, Kind::Note);
    }

    #[test]
    fn questions_wait_until_answered_and_queue_behind_their_after() {
        let mut issue = issue();
        let states = |i: &Issue| {
            i.questions()
                .iter()
                .map(|(q, a)| (q.qid().unwrap().to_owned(), *a))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            states(&issue),
            [
                ("Q1".into(), Asked::Answered),
                ("Q2".into(), Asked::Waiting),
                ("Q3".into(), Asked::Queued)
            ]
        );
        assert_eq!(
            issue.waiting()[0].subject(),
            "Where an expired session goes"
        );
        issue.push("jordan", Kind::Answer { id: "Q2".into() }, "1", 30_000_000);
        assert_eq!(issue.waiting()[0].qid(), Some("Q3"));
    }

    #[test]
    fn a_qq_question_is_about_its_marked_subject() {
        let text =
            "Q1: Fix the loop only\n\nQuestion 2 of 3\nQ2 — Where to check expiry\n\nContext:";
        let mut issue = issue();
        let q = Kind::Question {
            id: "Q2".into(),
            after: None,
        };
        issue.push("a", q, text, 30_000_000);
        assert_eq!(issue.waiting()[0].subject(), "Where to check expiry");
    }

    #[test]
    fn a_reply_closes_the_question_without_picking() {
        let mut issue = issue();
        issue.push(
            "jordan",
            Kind::Reply { id: "Q2".into() },
            "Neither",
            30_000_000,
        );
        assert_eq!(issue.waiting()[0].qid(), Some("Q3"));
        let text = issue.render();
        assert!(text.contains("jordan reply Q2\nNeither"), "{text}");
        assert_eq!(Issue::parse(12, &text).unwrap(), issue);
    }

    #[test]
    fn asking_again_replaces_the_question() {
        let mut issue = issue();
        let q = Kind::Question {
            id: "Q1".into(),
            after: None,
        };
        issue.push("auth fixer", q, "Fix scope, again", 30_000_000);
        assert_eq!(issue.waiting()[1].subject(), "Fix scope, again");
        assert_eq!(issue.questions().len(), 3);
    }

    #[test]
    fn words_that_only_look_like_kinds_stay_in_the_name() {
        let e = heading("## 2026-11-12T23:03Z the answer bot").unwrap();
        assert_eq!((e.who.as_str(), e.kind), ("the answer bot", Kind::Note));
        let e = heading("## 2026-11-12T23:03Z a question Q4b").unwrap();
        assert_eq!(e.qid(), Some("Q4b"));
        assert!(heading("## 2026-11-12T23:03Z").is_none());
    }

    #[test]
    fn files_without_a_header_or_title_are_refused() {
        assert!(Issue::parse(1, "# Title\n").is_err());
        assert!(Issue::parse(1, "---\nstatus: open\n---\n").is_err());
        assert!(Issue::parse(1, "---\ntitle: x\nstatus: later\n---\n").is_err());
    }

    #[test]
    fn file_names_are_id_and_slug() {
        let issue = Issue::new(7, "Fix: `lgterm ls` misses tabs!", None, "", 0);
        assert_eq!(issue.file_name(), "7-fix-lgterm-ls-misses-tabs.md");
    }
}
