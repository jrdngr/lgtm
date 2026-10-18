//! The review's architecture diff and history.

use std::collections::{BTreeSet, HashSet};

use super::compute::{self, Stored};
use super::group::Grouping;
use super::inputs::{self, Built};
use super::model::{self, Model};
use super::{ArchDto, ArchRange, HistoryDto, IntentKind, assess, state};
use crate::config::Config;
use crate::ctx::RepoCtx;
use crate::jj::Range;
use crate::review::View;
use crate::review::build::{ReviewFile, Sides};
use crate::workspace::Workspace;

impl RepoCtx {
    /// The review's architecture diff, or the diff between `range`'s commits.
    pub async fn arch(
        &self,
        review: u32,
        transitive: bool,
        range: Option<ArchRange>,
    ) -> anyhow::Result<ArchDto> {
        // `whole` is the review's base and head as loaded, which the assessment must be of to
        // be current.
        let (ws, revset, whole, own) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            let own = (r.view == View::Combined && range.is_none()).then(|| r.files.clone());
            (r.ws.clone(), r.revset.clone(), r.whole.clone(), own)
        };
        let (range, files, indexed, extra) = match own {
            Some(files) => (whole.clone(), files, true, vec![]),
            None => {
                let (range, extra) = match range {
                    Some(r) => {
                        let range = Range {
                            from: r.from,
                            to: r.to,
                        };
                        // Files the review changes differ in the working copy, so the range's
                        // unchanged ones are read at its end instead.
                        let in_range: HashSet<String> = ws
                            .jj
                            .diff(&range)
                            .await?
                            .into_iter()
                            .map(|d| d.path)
                            .collect();
                        let extra = ws
                            .jj
                            .diff(&whole)
                            .await?
                            .into_iter()
                            .map(|d| d.path)
                            .filter(|p| !in_range.contains(p))
                            .collect();
                        (range, extra)
                    }
                    None => (whole.clone(), vec![]),
                };
                (
                    range.clone(),
                    self.files_of(&ws, whole.from.as_deref(), &range).await?,
                    false,
                    extra,
                )
            }
        };
        let stored = self.stored(&ws, &revset, &whole)?;
        let move_paths: Vec<String> = stored
            .intents
            .iter()
            .filter_map(|(_, k)| match k {
                IntentKind::Move { path, .. } => Some(path.clone()),
                _ => None,
            })
            .collect();
        let changed = inputs::changed_of(&files, indexed);
        let Built { mut model, roots } = inputs::build(
            &ws.jj,
            &ws.root,
            &repo_name(&self.root),
            &range,
            changed,
            extra,
        )
        .await?;
        let lens = state::lens(&self.store.lock().unwrap())?;
        let root = ws.root.clone();
        let repo = repo_name(&self.root);
        Ok(tokio::task::spawn_blocking(move || {
            inputs::add_graph(&mut model, &files, &root);
            inputs::add_symbols(&mut model, &root, &move_paths);
            let g = Grouping::new(roots, lens, &repo);
            compute::compute(&model, &g, transitive, stored)
        })
        .await?)
    }

    /// The review's commits that changed the architecture, oldest first.
    pub async fn arch_history(&self, review: u32) -> anyhow::Result<Vec<HistoryDto>> {
        let (ws, revset) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            (r.ws.clone(), r.revset.clone())
        };
        let lens = state::lens(&self.store.lock().unwrap())?;
        let repo = repo_name(&self.root);
        // Files the review changes differ in the working copy, so each commit reads its
        // unchanged ones at the commit.
        let whole = ws.jj.resolve(&revset).await?;
        let reviewed: Vec<String> = ws
            .jj
            .diff(&whole)
            .await?
            .into_iter()
            .map(|d| d.path)
            .collect();
        let mut out = Vec::new();
        for c in ws.jj.changes(&revset).await? {
            let parent = ws
                .jj
                .commit_ids(&format!("parents({})", c.commit_id))
                .await?
                .into_iter()
                .next();
            let range = Range {
                from: parent.clone(),
                to: c.commit_id.clone(),
            };
            let changed = inputs::changed_of_diffs(&ws.jj.diff(&range).await?);
            if changed.is_empty() {
                continue;
            }
            let extra = reviewed
                .iter()
                .filter(|p| !changed.iter().any(|c| c.path == **p))
                .cloned()
                .collect();
            let Built { model, roots } =
                inputs::build(&ws.jj, &ws.root, &repo, &range, changed, extra).await?;
            let g = Grouping::new(roots, lens.clone(), &repo);
            let d = delta(&model, &g);
            if d != [0; 4] {
                out.push(HistoryDto {
                    change_id: c.change_id,
                    commit_id: c.commit_id,
                    parent,
                    description: c.description,
                    components_added: d[0],
                    components_removed: d[1],
                    edges_added: d[2],
                    edges_removed: d[3],
                });
            }
        }
        Ok(out)
    }

    /// Builds the files `range` changes, skipping by the config at the review's `base`.
    async fn files_of(
        &self,
        ws: &Workspace,
        base: Option<&str>,
        range: &Range,
    ) -> anyhow::Result<Vec<ReviewFile>> {
        let diffs = ws.jj.diff(range).await?;
        let sides = Sides {
            old: range.from.clone(),
            new: range.to.clone(),
        };
        Ok(self
            .build(sides, false, diffs, Config::at_base(&ws.jj, base).await?)
            .await?
            .0)
    }

    fn stored(&self, ws: &Workspace, revset: &str, whole: &Range) -> anyhow::Result<Stored> {
        let store = self.store.lock().unwrap();
        Ok(Stored {
            intents: state::intents(&store)?,
            checked: state::checked(&store, ws.key(), revset)?
                .into_iter()
                .collect(),
            assessed: assess::assessed(&store, ws.key(), revset, whole)?,
        })
    }
}

pub(super) fn repo_name(root: &std::path::Path) -> String {
    root.file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned())
}

/// Components added and removed, then edges added and removed.
fn delta(m: &Model, g: &Grouping) -> [u32; 4] {
    fn count<T: Ord>(a: &BTreeSet<T>, b: &BTreeSet<T>) -> u32 {
        a.difference(b).count() as u32
    }
    let (cb, ch) = (model::comps(&m.base, g), model::comps(&m.head, g));
    let eb: BTreeSet<_> = model::edges(&m.base, g).into_keys().collect();
    let eh: BTreeSet<_> = model::edges(&m.head, g).into_keys().collect();
    [
        count(&ch, &cb),
        count(&cb, &ch),
        count(&eh, &eb),
        count(&eb, &eh),
    ]
}
