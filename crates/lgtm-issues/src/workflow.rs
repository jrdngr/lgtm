//! A tracker's workflow: its statuses, one per folder under `issues/`, each described by an
//! optional `STATUS.md`, and `issues/workflow.md`, which orders them, says where the verbs
//! move issues, and lists the header fields.

mod default;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::fields::{self, Field};
use crate::header::{self, Pairs};
use crate::issue::Issue;

pub use default::write as write_default;

pub const FILE: &str = "workflow.md";
pub const STATUS_FILE: &str = "STATUS.md";
pub const TEMPLATE_FILE: &str = "TEMPLATE.md";

/// The `requires` entry that means no blocker is still open.
pub const UNBLOCKED: &str = "unblocked";

#[derive(Clone, Debug, PartialEq)]
pub enum Sort {
    /// Newest first.
    Touched,
    Id,
    /// By a choice field's values, last value first, then newest first.
    Field(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Status {
    /// The folder's name.
    pub id: String,
    pub label: String,
    /// A theme color's name.
    pub color: String,
    pub glyph: String,
    /// Its key in the status menu: 1 to 9 in order, then letters.
    pub key: char,
    pub done: bool,
    /// Listed as one count line until expanded.
    pub fold: bool,
    pub sort: Sort,
    /// Fields an issue needs set, or `unblocked`, before it can move here.
    pub requires: Vec<String>,
    /// The lgterm terminal told when an issue moves here.
    pub notify: Option<String>,
    /// What the status means, for agents.
    pub body: String,
    /// The header and body new issues filed here start from.
    pub template: Option<String>,
}

impl Status {
    fn read(dir: &Path, id: &str) -> Result<Status> {
        let text = fs::read_to_string(dir.join(STATUS_FILE)).unwrap_or_default();
        let (pairs, body) =
            header::split(&text).map_err(|e| anyhow::anyhow!("{id}/{STATUS_FILE}: {e}"))?;
        let get = |k| header::get(&pairs, k);
        let done = get("done") == Some("true");
        Ok(Status {
            id: id.to_owned(),
            label: get("label").map_or_else(|| id.replace('-', " "), str::to_owned),
            color: get("color").unwrap_or("muted").to_owned(),
            glyph: get("glyph")
                .unwrap_or(if done { "✓" } else { "○" })
                .to_owned(),
            key: ' ',
            done,
            fold: get("fold").map_or(done, |f| f == "true"),
            sort: match get("sort") {
                None | Some("touched") => Sort::Touched,
                Some("id") => Sort::Id,
                Some(f) => Sort::Field(f.to_owned()),
            },
            requires: get("requires").map(header::list).unwrap_or_default(),
            notify: get("notify").map(str::to_owned),
            body: body.trim().to_owned(),
            template: fs::read_to_string(dir.join(TEMPLATE_FILE)).ok(),
        })
    }
}

/// The moves agents know by name.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verb {
    New,
    Claim,
    Close,
    Drop,
    Reopen,
}

impl Verb {
    fn key(self) -> &'static str {
        match self {
            Verb::New => "new",
            Verb::Claim => "claim",
            Verb::Close => "close",
            Verb::Drop => "drop",
            Verb::Reopen => "reopen",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Workflow {
    /// In order.
    pub statuses: Vec<Status>,
    /// The header fields this tracker uses, in the order they show.
    pub fields: Vec<Field>,
    /// How the tracker works, for agents.
    pub body: String,
    pairs: Pairs,
}

/// The status folders in `issues`: every folder not starting with `.`.
pub fn folders(issues: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let entries = match fs::read_dir(issues) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() && !name.starts_with('.') {
            out.push(name);
        }
    }
    out.sort();
    Ok(out)
}

impl Workflow {
    /// Reads the workflow of the `issues` folder.
    pub fn load(issues: &Path) -> Result<Workflow> {
        let text = fs::read_to_string(issues.join(FILE)).unwrap_or_default();
        let (pairs, body) =
            header::split(&text).map_err(|e| anyhow::anyhow!("issues/{FILE}: {e}"))?;
        let mut ids = folders(issues)?;
        let order = header::get(&pairs, "order").map(header::list);
        let mut ordered: Vec<String> = order
            .unwrap_or_default()
            .into_iter()
            .filter(|s| ids.contains(s))
            .collect();
        ids.retain(|s| !ordered.contains(s));
        ordered.extend(ids);
        let mut statuses = Vec::new();
        for (n, id) in ordered.iter().enumerate() {
            let mut s = Status::read(&issues.join(id), id)?;
            s.key = key(n);
            statuses.push(s);
        }
        let names = header::get(&pairs, "fields")
            .map_or_else(|| fields::DEFAULT.map(str::to_owned).to_vec(), header::list);
        let fields = names
            .iter()
            .map(|n| Field::new(n, header::get(&pairs, n)))
            .collect();
        Ok(Workflow {
            statuses,
            fields,
            body: body.trim().to_owned(),
            pairs,
        })
    }

    pub fn status(&self, id: &str) -> Option<&Status> {
        self.statuses.iter().find(|s| s.id == id)
    }

    /// A status by its folder name or its label, for commands.
    pub fn find(&self, name: &str) -> Result<&Status> {
        let lower = name.to_lowercase();
        self.statuses
            .iter()
            .find(|s| s.id == name || s.label.to_lowercase() == lower)
            .with_context(|| format!("no status {name}: {}", self.ids().join(", ")))
    }

    fn ids(&self) -> Vec<&str> {
        self.statuses.iter().map(|s| s.id.as_str()).collect()
    }

    pub fn is_done(&self, id: &str) -> bool {
        self.status(id).is_some_and(|s| s.done)
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Where a verb moves an issue: the status `workflow.md` names, else its default.
    pub fn verb(&self, verb: Verb) -> Result<&Status> {
        if let Some(named) = header::get(&self.pairs, verb.key()) {
            return self.status(named).with_context(|| {
                format!(
                    "{FILE} says {}: {named}, but there's no issues/{named}",
                    verb.key()
                )
            });
        }
        let done: Vec<&Status> = self.statuses.iter().filter(|s| s.done).collect();
        let found = match verb {
            Verb::New => self.statuses.first(),
            Verb::Reopen => return self.verb(Verb::New),
            Verb::Claim => {
                let new = self.verb(Verb::New)?;
                self.statuses
                    .iter()
                    .skip_while(|s| s.id != new.id)
                    .skip(1)
                    .find(|s| !s.done)
            }
            Verb::Close => done.first().copied(),
            Verb::Drop => done.get(1).or(done.first()).copied(),
        };
        found.with_context(|| format!("no status for {}; name one in issues/{FILE}", verb.key()))
    }

    /// What `issue` lacks to move into `to`: field names, or `unblocked`.
    pub fn unmet(&self, issue: &Issue, all: &[Issue], to: &Status) -> Vec<String> {
        to.requires
            .iter()
            .filter(|r| {
                if *r == UNBLOCKED {
                    !issue.open_blockers(all, |s| self.is_done(s)).is_empty()
                } else {
                    issue.field(r).is_none()
                }
            })
            .cloned()
            .collect()
    }
}

/// The status menu key for the status at `n` in order.
fn key(n: usize) -> char {
    match n {
        0..9 => char::from(b'1' + n as u8),
        _ => char::from(b'a' + (n - 9).min(25) as u8),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(statuses: &[(&str, &str)], workflow: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (id, text) in statuses {
            let d = dir.path().join(id);
            fs::create_dir_all(&d).unwrap();
            if !text.is_empty() {
                fs::write(d.join(STATUS_FILE), text).unwrap();
            }
        }
        if !workflow.is_empty() {
            fs::write(dir.path().join(FILE), workflow).unwrap();
        }
        dir
    }

    #[test]
    fn statuses_come_from_folders_in_the_named_order_then_the_rest() {
        let d = dir(
            &[
                (
                    "review",
                    "---\nlabel: In review\ncolor: cyan\nrequires: revision\n---\nRead it.",
                ),
                ("done", "---\ndone: true\n---\n"),
                ("triage", ""),
                ("in-progress", ""),
            ],
            "---\norder: triage, review, gone\n---\n",
        );
        fs::create_dir_all(d.path().join(".hidden")).unwrap();
        let w = Workflow::load(d.path()).unwrap();
        let shown: Vec<(&str, char)> = w.statuses.iter().map(|s| (s.id.as_str(), s.key)).collect();
        assert_eq!(
            shown,
            [
                ("triage", '1'),
                ("review", '2'),
                ("done", '3'),
                ("in-progress", '4')
            ]
        );
        let review = w.status("review").unwrap();
        assert_eq!(
            (
                review.label.as_str(),
                review.color.as_str(),
                review.body.as_str()
            ),
            ("In review", "cyan", "Read it.")
        );
        assert_eq!(review.requires, ["revision"]);
        let done = w.status("done").unwrap();
        assert_eq!((done.glyph.as_str(), done.fold), ("✓", true));
        assert_eq!(w.status("in-progress").unwrap().label, "in progress");
        assert_eq!(w.find("In Review").unwrap().id, "review");
        assert!(w.find("later").is_err());
    }

    #[test]
    fn verbs_default_from_the_order_unless_named() {
        let d = dir(
            &[
                ("a-new", ""),
                ("b-done", "---\ndone: true\n---\n"),
                ("c-work", ""),
                ("d-gone", "---\ndone: true\n---\n"),
            ],
            "---\norder: a-new, c-work, b-done, d-gone\n---\n",
        );
        let w = Workflow::load(d.path()).unwrap();
        let at = |v| w.verb(v).unwrap().id.clone();
        assert_eq!(
            [
                Verb::New,
                Verb::Claim,
                Verb::Close,
                Verb::Drop,
                Verb::Reopen
            ]
            .map(at),
            ["a-new", "c-work", "b-done", "d-gone", "a-new"]
        );
        fs::write(
            d.path().join(FILE),
            "---\nclose: d-gone\nclaim: nowhere\n---\n",
        )
        .unwrap();
        let w = Workflow::load(d.path()).unwrap();
        assert_eq!(w.verb(Verb::Close).unwrap().id, "d-gone");
        assert!(w.verb(Verb::Claim).is_err());
    }

    #[test]
    fn fields_default_to_the_built_in_four_and_take_values_from_their_lines() {
        let w = Workflow::load(dir(&[], "").path()).unwrap();
        let names: Vec<&str> = w.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, fields::DEFAULT);
        let d = dir(
            &[],
            "---\nfields: area, severity\narea: ui, core\nseverity: s1, s2\n---\n",
        );
        let w = Workflow::load(d.path()).unwrap();
        assert_eq!(w.field("area").unwrap().rank("core"), Some(1));
        assert_eq!(w.field("severity").unwrap().rank("s2"), Some(1));
        assert!(w.field("revision").is_none());
    }

    #[test]
    fn requires_names_fields_or_unblocked() {
        let d = dir(
            &[
                ("open", ""),
                (
                    "done",
                    "---\ndone: true\nrequires: revision, unblocked\n---\n",
                ),
            ],
            "",
        );
        let w = Workflow::load(d.path()).unwrap();
        let done = w.status("done").unwrap();
        let mut blocker = Issue::new(1, "Blocker", None, "", 0);
        blocker.status = "open".into();
        let mut issue = Issue::new(2, "Fix", None, "", 0);
        issue.blocked_by = vec![1];
        let all = vec![blocker.clone(), issue.clone()];
        assert_eq!(w.unmet(&issue, &all, done), ["revision", UNBLOCKED]);
        issue.revision = Some("wytk".into());
        blocker.status = "done".into();
        assert!(w.unmet(&issue, &[blocker, issue.clone()], done).is_empty());
    }
}
