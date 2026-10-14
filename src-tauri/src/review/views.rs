//! Review views (Combined, Per change, Since last session) and the stack of changes they pick
//! from. Old commits map to current ones through evolog predecessors, since split and squash
//! break change IDs.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use specta::Type;

use super::build::Sides;
use crate::jj::{Change, FileDiff, Jj, Range};
use crate::store::Session;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum View {
    /// The whole review as one diff.
    Combined,
    /// One change's own diff.
    Change { change: String },
    /// What one change does differently from what it did last session.
    SinceLast { change: String },
}

/// How a change compares with the previous session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Since {
    /// There's no previous session.
    Unknown,
    New,
    Changed,
    Same,
    /// In the previous session but not in the review now.
    Removed,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangeDto {
    pub change_id: String,
    pub commit_id: String,
    /// Length of the shortest prefix of `change_id` that's unique in the repo.
    pub unique: u32,
    /// Whether it's the workspace's working-copy commit.
    pub working_copy: bool,
    /// Whether it has unresolved conflicts.
    pub conflict: bool,
    /// Whether it changes no files.
    pub empty: bool,
    /// Whether other visible commits share its change id.
    pub divergent: bool,
    /// Its local bookmarks, each ending in `*` when the remote lacks it or is behind.
    pub bookmarks: Vec<String>,
    pub description: String,
    pub since: Since,
}

pub struct Stack {
    pub changes: Vec<ChangeDto>,
    pub removed: Vec<ChangeDto>,
    /// Previous-session commits each current change evolved from, by index into `changes`.
    preds: Vec<Vec<String>>,
}

fn dto(c: Change, since: Since) -> ChangeDto {
    ChangeDto {
        change_id: c.change_id,
        commit_id: c.commit_id,
        unique: c.unique,
        working_copy: c.working_copy,
        conflict: c.conflict,
        empty: c.empty,
        divergent: c.divergent,
        bookmarks: c.bookmarks,
        description: c.description,
        since,
    }
}

pub async fn stack(jj: &Jj, revset: &str, prev: Option<&Session>) -> anyhow::Result<Stack> {
    let changes = jj.changes(revset).await?;
    let Some(prev) = prev.filter(|p| !p.commits.is_empty()) else {
        return Ok(Stack {
            preds: vec![vec![]; changes.len()],
            changes: changes
                .into_iter()
                .map(|c| dto(c, Since::Unknown))
                .collect(),
            removed: vec![],
        });
    };
    // Bounded, since each evolog is a jj process with its own pipes.
    let limit = tokio::sync::Semaphore::new(8);
    let evologs = futures::future::try_join_all(changes.iter().map(|c| {
        let limit = &limit;
        async move {
            let _permit = limit.acquire().await?;
            jj.evolog(&c.commit_id).await
        }
    }))
    .await?;
    let mut preds: Vec<Vec<String>> = evologs
        .iter()
        .map(|evolog| {
            let found = prev.commits.iter().filter(|old| evolog.contains(old));
            found.cloned().collect()
        })
        .collect();
    let succeeded =
        |preds: &[Vec<String>]| -> HashSet<String> { preds.iter().flatten().cloned().collect() };
    let gone_from = |succeeded: &HashSet<String>| -> Vec<String> {
        let gone = prev.commits.iter().filter(|c| !succeeded.contains(*c));
        gone.cloned().collect()
    };
    let gone = gone_from(&succeeded(&preds));
    let mut old = if gone.is_empty() {
        vec![]
    } else {
        // Hidden commits stay readable until `jj util gc`; after that only the id is left.
        jj.changes(&gone.join("|")).await.unwrap_or_else(|_| {
            gone.iter()
                .map(|c| Change {
                    change_id: String::new(),
                    commit_id: c.to_string(),
                    unique: 0,
                    working_copy: false,
                    conflict: false,
                    empty: false,
                    divergent: false,
                    bookmarks: vec![],
                    description: String::new(),
                })
                .collect()
        })
    };
    // A fetched commit has no evolog link to the one it replaced (a PR's force-push), but jj
    // keeps the change id when the author uses jj.
    for (c, p) in changes.iter().zip(preds.iter_mut()) {
        if p.is_empty() && !prev.commits.contains(&c.commit_id) {
            let same = old
                .iter()
                .filter(|o| !o.change_id.is_empty() && o.change_id == c.change_id);
            p.extend(same.map(|o| o.commit_id.clone()));
        }
    }
    let matched = succeeded(&preds);
    old.retain(|o| !matched.contains(&o.commit_id));
    let removed = old;
    let dtos: Vec<ChangeDto> = changes
        .into_iter()
        .zip(&preds)
        .map(|(c, p)| {
            let since = if prev.commits.contains(&c.commit_id) {
                Since::Same
            } else if p.is_empty() {
                Since::New
            } else {
                Since::Changed
            };
            dto(c, since)
        })
        .collect();
    Ok(Stack {
        changes: dtos,
        removed: removed
            .into_iter()
            .map(|c| dto(c, Since::Removed))
            .collect(),
        preds,
    })
}

/// What a view diffs.
pub enum Spec {
    Trees(Range),
    Interdiff { from: String, to: String },
}

impl Spec {
    pub fn sides(&self) -> Sides {
        match self {
            Spec::Trees(r) => Sides {
                old: r.from.clone(),
                new: r.to.clone(),
            },
            Spec::Interdiff { to, .. } => Sides {
                old: None,
                new: to.clone(),
            },
        }
    }

    pub fn endpoints(&self) -> (Option<String>, String) {
        match self {
            Spec::Trees(r) => (r.from.clone(), r.to.clone()),
            Spec::Interdiff { from, to } => (Some(from.clone()), to.clone()),
        }
    }

    pub async fn diff(&self, jj: &Jj) -> anyhow::Result<Vec<FileDiff>> {
        match self {
            Spec::Trees(r) => jj.diff(r).await,
            Spec::Interdiff { from, to } => jj.interdiff(from, to).await,
        }
    }
}

pub async fn spec(jj: &Jj, revset: &str, view: &View, stack: &Stack) -> anyhow::Result<Spec> {
    let find = |change: &str| {
        stack
            .changes
            .iter()
            .position(|c| c.change_id == change)
            .ok_or_else(|| anyhow::anyhow!("change {change} is no longer in the review"))
    };
    match view {
        View::Combined => Ok(Spec::Trees(jj.resolve(revset).await?)),
        View::Change { change } => {
            let c = &stack.changes[find(change)?];
            Ok(Spec::Trees(jj.resolve(&c.commit_id).await?))
        }
        View::SinceLast { change } => {
            let i = find(change)?;
            let c = &stack.changes[i];
            let preds = &stack.preds[i];
            match c.since {
                Since::Same => Ok(Spec::Trees(Range {
                    from: Some(c.commit_id.clone()),
                    to: c.commit_id.clone(),
                })),
                Since::Changed => {
                    let newest = preds.last().expect("changed change has a predecessor");
                    // After a split, compare the old commit with all of its successors.
                    let successors: Vec<usize> = (0..stack.changes.len())
                        .filter(|&j| stack.preds[j].contains(newest))
                        .collect();
                    if successors.len() > 1 {
                        let last = &stack.changes[*successors.last().unwrap()];
                        return Ok(Spec::Trees(Range {
                            from: Some(newest.clone()),
                            to: last.commit_id.clone(),
                        }));
                    }
                    Ok(Spec::Interdiff {
                        from: newest.clone(),
                        to: c.commit_id.clone(),
                    })
                }
                _ => Ok(Spec::Trees(jj.resolve(&c.commit_id).await?)),
            }
        }
    }
}
