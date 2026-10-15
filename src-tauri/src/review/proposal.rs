//! Code changes an agent proposes in a thread's chat. They're kept as each file's text before
//! and after, and written only when the user makes the reply final.

use std::path::Path;

use anyhow::Context;
use serde::Deserialize;

use super::{RepoCtx, StateDto};
use crate::files;
use crate::store::{Proposed, ProposedState};

/// One text replacement, as the agent returns it. `old` is empty for a new file.
#[derive(Clone, Debug, Deserialize)]
pub(super) struct EditDto {
    pub path: String,
    pub old: String,
    pub new: String,
    /// Replace every occurrence of `old` rather than exactly one.
    #[serde(default)]
    pub all: bool,
}

/// Files that run or steer agents, which a proposal may not touch.
fn protected(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    parts
        .iter()
        .any(|p| matches!(*p, ".claude" | ".git" | ".jj" | ".lgtm"))
        || parts
            .last()
            .is_some_and(|f| matches!(*f, ".mcp.json" | "CLAUDE.md" | "AGENTS.md" | ".envrc"))
}

/// A file's text, empty when it doesn't exist.
fn current(root: &Path, path: &str) -> anyhow::Result<String> {
    let abs = files::resolve(root, path)?;
    files::contain(root, &abs)?;
    if !abs.exists() {
        return Ok(String::new());
    }
    files::read(&abs, path)
}

/// Applies `edits` to the files under `root` in memory, one proposal per file in the order
/// they first appear. Returns the proposal and why any edit was left out.
pub(super) fn build(root: &Path, edits: &[EditDto]) -> (Vec<Proposed>, Vec<String>) {
    let mut out: Vec<Proposed> = vec![];
    let mut problems = vec![];
    for e in edits {
        let path = e.path.trim_start_matches("./");
        if protected(path) {
            problems.push(format!("{path} can't be changed by an agent."));
            continue;
        }
        let i = match out.iter().position(|p| p.path == path) {
            Some(i) => i,
            None => match current(root, path) {
                Ok(text) => {
                    out.push(Proposed {
                        path: path.to_string(),
                        before: text.clone(),
                        after: text,
                        state: ProposedState::Proposed,
                    });
                    out.len() - 1
                }
                Err(err) => {
                    problems.push(format!("{err:#}"));
                    continue;
                }
            },
        };
        let p = &mut out[i];
        if e.old.is_empty() {
            if p.after.is_empty() {
                p.after = e.new.clone();
            } else {
                problems.push(format!("{path} already exists, so it wasn't replaced."));
            }
            continue;
        }
        match p.after.matches(e.old.as_str()).count() {
            0 => problems.push(format!("A change to {path} didn't match the file.")),
            _ if e.all => p.after = p.after.replace(e.old.as_str(), &e.new),
            1 => p.after = p.after.replacen(e.old.as_str(), &e.new, 1),
            _ => problems.push(format!("A change to {path} matched more than one place.")),
        }
    }
    out.retain(|p| p.after != p.before);
    (out, problems)
}

/// Settles a proposal after an agent edited the files itself: a file that already reads as
/// proposed counts as applied, and one changed some other way drops out, since those edits
/// replace it.
pub(super) fn settle(root: &Path, proposal: &mut Vec<Proposed>) {
    proposal.retain_mut(|p| {
        if p.state != ProposedState::Proposed {
            return true;
        }
        let Ok(text) = current(root, &p.path) else {
            return true;
        };
        if text == p.after {
            p.state = ProposedState::Applied;
        }
        text == p.before || text == p.after
    });
}

/// Writes each proposed file whose text is still what the proposal started from, or counts it
/// applied when it already reads as proposed. Others go stale.
pub(super) fn write(root: &Path, proposal: &mut [Proposed]) -> anyhow::Result<()> {
    for p in proposal
        .iter_mut()
        .filter(|p| p.state == ProposedState::Proposed)
    {
        let text = current(root, &p.path)?;
        if text == p.after {
            p.state = ProposedState::Applied;
            continue;
        }
        if text != p.before {
            p.state = ProposedState::Stale;
            continue;
        }
        let abs = files::resolve(root, &p.path)?;
        if let Some(dir) = abs.parent() {
            std::fs::create_dir_all(dir)?;
        }
        files::contain(root, &abs)?;
        std::fs::write(&abs, &p.after).with_context(|| format!("can't save {}", p.path))?;
        p.state = ProposedState::Applied;
    }
    Ok(())
}

impl RepoCtx {
    /// Writes a thread's proposed changes to the workspace's working copy and snapshots it.
    pub async fn apply_proposal(&self, id: u32, comment: u32) -> anyhow::Result<StateDto> {
        let cid = comment as i64;
        let review = self.review(id)?;
        let ws = review.lock().unwrap().ws.clone();
        let _busy = ws.claim()?;
        let mut thread = {
            let store = self.store.lock().unwrap();
            let c = store.comment(cid)?.context("the thread is gone")?;
            c.thread.context("notes have no proposal")?
        };
        write(&ws.root, &mut thread.proposal)?;
        ws.snapshot().await.ok();
        self.comment_edit_on(
            &review,
            |_, store| Ok(store.set_thread(cid, &Some(thread))?),
        )
    }

    /// Drops the proposed change to `path`, or every proposed change when it's `None`.
    pub fn discard_proposal(
        &self,
        id: u32,
        comment: u32,
        path: Option<String>,
    ) -> anyhow::Result<StateDto> {
        self.comment_edit(id, |_, store| {
            let cid = comment as i64;
            let c = store.comment(cid)?.context("the thread is gone")?;
            let mut t = c.thread.context("notes have no proposal")?;
            t.proposal
                .retain(|p| path.as_ref().is_some_and(|path| &p.path != path));
            Ok(store.set_thread(cid, &Some(t))?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agent_edit_settles_the_proposal() {
        // The agent made `a.rs` exactly as proposed, changed `b.rs` its own way, and left
        // `c.rs` alone.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for f in ["a.rs", "b.rs", "c.rs"] {
            std::fs::write(root.join(f), "x\n").unwrap();
        }
        let (mut proposal, _) = build(
            root,
            &[edit("a.rs", "x", "A"), edit("b.rs", "x", "B"), edit("c.rs", "x", "C")],
        );
        std::fs::write(root.join("a.rs"), "A\n").unwrap();
        std::fs::write(root.join("b.rs"), "other\n").unwrap();
        settle(root, &mut proposal);
        let got: Vec<_> = proposal.iter().map(|p| (p.path.as_str(), p.state)).collect();
        use ProposedState::*;
        assert_eq!(got, [("a.rs", Applied), ("c.rs", Proposed)]);
    }

    fn edit(path: &str, old: &str, new: &str) -> EditDto {
        EditDto {
            path: path.into(),
            old: old.into(),
            new: new.into(),
            all: false,
        }
    }

    #[test]
    fn builds_one_proposal_per_file_from_exact_replacements() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.rs"), "fn a() {}\nfn b() {}\n").unwrap();
        let (proposal, problems) = build(
            root,
            &[
                edit("a.rs", "fn a()", "/// A.\nfn a()"),
                edit("a.rs", "fn b()", "fn c()"),
                edit("a.rs", "missing", "x"),
                edit("a.rs", "fn", "x"),
                EditDto {
                    all: true,
                    ..edit("a.rs", "{}", "{ }")
                },
                edit("new.md", "", "hi\n"),
                edit(".claude/settings.json", "", "{}"),
            ],
        );
        assert_eq!(proposal.len(), 2);
        assert_eq!(proposal[0].after, "/// A.\nfn a() { }\nfn c() { }\n");
        assert_eq!(
            (proposal[1].before.as_str(), proposal[1].after.as_str()),
            ("", "hi\n")
        );
        // The missing match, the ambiguous one, and the protected file.
        assert_eq!(problems.len(), 3);
    }

    #[test]
    fn writes_unchanged_files_and_leaves_changed_ones_stale() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.rs"), "a\n").unwrap();
        std::fs::write(root.join("b.rs"), "b\n").unwrap();
        let (mut proposal, _) = build(
            root,
            &[
                edit("a.rs", "a", "A"),
                edit("b.rs", "b", "B"),
                edit("src/c.rs", "", "c\n"),
            ],
        );
        std::fs::write(root.join("b.rs"), "edited\n").unwrap();
        write(root, &mut proposal).unwrap();
        let states: Vec<_> = proposal.iter().map(|p| p.state).collect();
        use ProposedState::*;
        assert_eq!(states, [Applied, Stale, Applied]);
        assert_eq!(std::fs::read_to_string(root.join("a.rs")).unwrap(), "A\n");
        assert_eq!(
            std::fs::read_to_string(root.join("b.rs")).unwrap(),
            "edited\n"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("src/c.rs")).unwrap(),
            "c\n"
        );
        // Written again, a file that reads as proposed counts as applied.
        let (mut again, _) = build(root, &[edit("b.rs", "edited", "B")]);
        std::fs::write(root.join("b.rs"), "B\n").unwrap();
        write(root, &mut again).unwrap();
        assert_eq!(again[0].state, Applied);
    }
}
