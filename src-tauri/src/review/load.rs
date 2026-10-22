//! Loading a revset's diff and building its files, reusing cached builds where possible.

use std::collections::HashMap;

use crate::anchor::MatchConfig;
use crate::config::Config;
use crate::diff::FileStatus;
use crate::jj::{Change, FileDiff, Range};
use crate::store::Session;
use crate::workspace::Workspace;

use super::build::{self, FileDto, ReviewFile, Sides};
use super::{RepoCtx, ReviewDto, StateDto, View, cache, views};

/// Files larger than this load without contents: no highlighting, symbols, or context.
const MAX_FILE_BYTES: usize = 4 << 20;

/// A freshly loaded diff, before it's swapped into a review.
pub(super) struct Loaded {
    pub files: Vec<ReviewFile>,
    pub dto: ReviewDto,
    pub commits: Vec<String>,
    /// The revset's base and head.
    pub whole: Range,
    pub cfg: MatchConfig,
}

/// A revset's base and head and its changes, oldest first.
pub(super) struct Listed {
    pub whole: Range,
    pub changes: Vec<Change>,
}

impl Listed {
    pub async fn of(ws: &Workspace, revset: &str) -> anyhow::Result<Self> {
        let (whole, changes) = tokio::try_join!(ws.jj.resolve(revset), ws.jj.changes(revset))?;
        Ok(Self { whole, changes })
    }

    pub fn commits(&self) -> Vec<String> {
        self.changes.iter().map(|c| c.commit_id.clone()).collect()
    }
}

impl RepoCtx {
    pub(super) async fn load(
        &self,
        ws: &Workspace,
        revset: &str,
        listed: Listed,
        view: &View,
        prev: Option<&Session>,
    ) -> anyhow::Result<Loaded> {
        let whole = listed.whole;
        let config = Config::at_base(&ws.jj, &self.reviews.configs, whole.from.as_deref()).await?;
        let stack = views::stack(&ws.jj, &self.reviews.evologs, listed.changes, prev).await?;
        // A view of a change that left the revset falls back to the whole review.
        let view = match view {
            View::Change { change } | View::SinceLast { change }
                if !stack.changes.iter().any(|c| c.change_id == *change) =>
            {
                &View::Combined
            }
            view => view,
        };
        let spec = views::spec(&ws.jj, &whole, view, &stack).await?;
        let diffs = spec.diff(&ws.jj).await?;
        let sides = spec.sides();
        let interdiff = matches!(spec, views::Spec::Interdiff { .. });
        let cfg = config.matching;
        let (files, dtos) = self.build(sides, interdiff, diffs, config).await?;
        let (from, to) = spec.endpoints();
        let head = whole.to.clone();
        Ok(Loaded {
            files,
            commits: stack.changes.iter().map(|c| c.commit_id.clone()).collect(),
            whole,
            cfg,
            dto: ReviewDto {
                id: 0,
                revset: revset.to_string(),
                view: view.clone(),
                from,
                to,
                head,
                interdiff,
                files: dtos,
                state: StateDto::default(),
                changes: stack.changes,
                removed_changes: stack.removed,
                previous_session: prev.map(|p| p.started_at as f64),
                stale: ws.stale.lock().unwrap().clone(),
            },
        })
    }

    /// Builds each file, reusing the last load's build of files whose diff is unchanged.
    pub(super) async fn build(
        &self,
        sides: Sides,
        interdiff: bool,
        diffs: Vec<FileDiff>,
        config: Config,
    ) -> anyhow::Result<(Vec<ReviewFile>, Vec<FileDto>)> {
        let keys: Vec<cache::Key> = diffs
            .iter()
            .map(|d| cache::key(&sides, interdiff, d, &config))
            .collect();
        let mut built: Vec<Option<(ReviewFile, FileDto)>> = {
            let mut cache = self.reviews.cache.lock().unwrap();
            keys.iter().map(|k| cache.take(k, &sides)).collect()
        };
        let misses: Vec<(usize, FileDiff)> = diffs
            .into_iter()
            .enumerate()
            .filter(|(i, _)| built[*i].is_none())
            .collect();
        let miss_diffs: Vec<&FileDiff> = misses.iter().map(|(_, d)| d).collect();
        let contents = self.load_contents(&sides, &miss_diffs).await?;
        let fresh = tokio::task::spawn_blocking(move || {
            misses
                .into_iter()
                .zip(contents)
                .map(|((i, d), (old, new))| (i, build::build_file(&sides, d, old, new, &config)))
                .collect::<Vec<_>>()
        })
        .await?;
        for (i, b) in fresh {
            built[i] = Some(b);
        }
        let built: Vec<(ReviewFile, FileDto)> = built.into_iter().map(Option::unwrap).collect();
        self.reviews
            .cache
            .lock()
            .unwrap()
            .put(keys.into_iter().zip(built.iter().cloned()));
        Ok(built.into_iter().unzip())
    }

    /// Old and new contents per file, `None` where a side is absent, binary, or too large.
    /// Each side is one batched read.
    async fn load_contents(
        &self,
        sides: &Sides,
        diffs: &[&FileDiff],
    ) -> anyhow::Result<Vec<(Option<String>, Option<String>)>> {
        let old_path = |d: &FileDiff| d.old_path.clone().unwrap_or_else(|| d.path.clone());
        let old_paths: Vec<String> = diffs
            .iter()
            .filter(|d| !d.binary && d.status != FileStatus::Added)
            .map(|d| old_path(d))
            .collect();
        let new_paths: Vec<String> = diffs
            .iter()
            .filter(|d| !d.binary && d.status != FileStatus::Removed)
            .map(|d| d.path.clone())
            .collect();
        let jj = &self.main.jj;
        let old = async {
            match sides.old.as_deref() {
                Some(commit) if !old_paths.is_empty() => jj.files_at(commit, &old_paths).await,
                // The root commit has no files.
                _ => Ok(HashMap::new()),
            }
        };
        let new = async {
            if new_paths.is_empty() {
                return Ok(HashMap::new());
            }
            jj.files_at(&sides.new, &new_paths).await
        };
        let (old, mut new) = tokio::try_join!(old, new)?;
        let text = |bytes: Option<Vec<u8>>| {
            bytes
                .filter(|b| b.len() <= MAX_FILE_BYTES)
                .and_then(|b| String::from_utf8(b).ok())
        };
        Ok(diffs
            .iter()
            .map(|d| {
                let o = (!d.binary && d.status != FileStatus::Added)
                    // A file copied twice shares its old side, so it's cloned, not taken.
                    .then(|| old.get(&old_path(d)).cloned())
                    .flatten();
                let n = (!d.binary && d.status != FileStatus::Removed)
                    .then(|| new.remove(&d.path))
                    .flatten();
                (text(o), text(n))
            })
            .collect())
    }
}
