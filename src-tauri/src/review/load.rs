//! Loading a revset's diff and building its files, reusing cached builds where possible.

use crate::anchor::MatchConfig;
use crate::config::Config;
use crate::diff::FileStatus;
use crate::jj::FileDiff;
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
    pub cfg: MatchConfig,
}

impl RepoCtx {
    pub(super) async fn load(
        &self,
        ws: &Workspace,
        revset: &str,
        view: &View,
        prev: Option<&Session>,
    ) -> anyhow::Result<Loaded> {
        let config = Config::load(&ws.root)?;
        let stack = views::stack(&ws.jj, revset, prev).await?;
        // A view of a change that left the revset falls back to the whole review.
        let view = match view {
            View::Change { change } | View::SinceLast { change }
                if !stack.changes.iter().any(|c| c.change_id == *change) =>
            {
                &View::Combined
            }
            view => view,
        };
        let spec = views::spec(&ws.jj, revset, view, &stack).await?;
        let diffs = spec.diff(&ws.jj).await?;
        let sides = spec.sides();
        let interdiff = matches!(spec, views::Spec::Interdiff { .. });
        let cfg = config.matching;
        let (files, dtos) = self.build(sides, interdiff, diffs, config).await?;
        let (from, to) = spec.endpoints();
        Ok(Loaded {
            files,
            commits: stack.changes.iter().map(|c| c.commit_id.clone()).collect(),
            cfg,
            dto: ReviewDto {
                id: 0,
                revset: revset.to_string(),
                view: view.clone(),
                from,
                to,
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
            let mut cache = self.cache.lock().unwrap();
            keys.iter().map(|k| cache.take(k, &sides.new)).collect()
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
        self.cache
            .lock()
            .unwrap()
            .put(keys.into_iter().zip(built.iter().cloned()));
        Ok(built.into_iter().unzip())
    }

    /// Old and new contents per file, `None` where a side is absent, binary, or too large.
    async fn load_contents(
        &self,
        sides: &Sides,
        diffs: &[&FileDiff],
    ) -> anyhow::Result<Vec<(Option<String>, Option<String>)>> {
        let limit = tokio::sync::Semaphore::new(8);
        let load = |commit: Option<&'_ str>, path: &'_ str, present: bool| {
            let limit = &limit;
            let (commit, path) = (commit.map(str::to_string), path.to_string());
            async move {
                if !present {
                    return Ok::<_, anyhow::Error>(None);
                }
                let _permit = limit.acquire().await?;
                let bytes = self.main.jj.file(commit.as_deref(), &path).await?;
                Ok((bytes.len() <= MAX_FILE_BYTES)
                    .then(|| String::from_utf8(bytes).ok())
                    .flatten())
            }
        };
        futures::future::try_join_all(diffs.iter().map(|d| {
            let old_path = d.old_path.as_deref().unwrap_or(&d.path);
            let old_present = !d.binary && d.status != FileStatus::Added;
            let new_present = !d.binary && d.status != FileStatus::Removed;
            let old = load(sides.old.as_deref(), old_path, old_present);
            let new = load(Some(&sides.new), &d.path, new_present);
            async move { tokio::try_join!(old, new) }
        }))
        .await
    }
}
