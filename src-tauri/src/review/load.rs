//! Loading a revset's diff and building its files, reusing cached builds where possible.

use lgtm_core::review::contents;

use crate::anchor::MatchConfig;
use crate::config::{Config, ConfigTexts};
use crate::jj::{Change, FileDiff, Range};
use crate::store::Session;
use crate::workspace::Workspace;

use super::build::{self, FileDto, ReviewFile, Sides};
use super::{RepoCtx, ReviewDto, StateDto, View, cache, views};

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
        let config =
            Config::at_base(&ws.jj, &self.state::<ConfigTexts>(), whole.from.as_deref()).await?;
        let stack = views::stack(&ws.jj, &self.reviews().evologs, listed.changes, prev).await?;
        // A view of a change that left the revset falls back to the whole review.
        let view = match view {
            View::Change { change } | View::SinceLast { change }
                if !stack.changes.iter().any(|c| c.change_id == *change) =>
            {
                &View::Combined
            }
            view => view,
        };
        // Turns and fixes that are gone fall back to the whole review too.
        let span = match view {
            View::Turn { turns } => self.turn_span(turns, &at_commit(ws).await?)?,
            View::Fix { fix } => self.fix_span(*fix)?,
            _ => None,
        };
        let view = match (view, &span) {
            (View::Turn { .. } | View::Fix { .. }, None) => &View::Combined,
            _ => view,
        };
        let spec = match span {
            Some((range, paths)) => views::Spec::Span { range, paths },
            None => views::spec(&ws.jj, &whole, view, &stack).await?,
        };
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
            let reviews = self.reviews();
            let mut cache = reviews.cache.lock().unwrap();
            keys.iter().map(|k| cache.take(k, &sides)).collect()
        };
        let misses: Vec<(usize, FileDiff)> = diffs
            .into_iter()
            .enumerate()
            .filter(|(i, _)| built[*i].is_none())
            .collect();
        let miss_diffs: Vec<&FileDiff> = misses.iter().map(|(_, d)| d).collect();
        let contents = contents::read(&self.main.jj, &sides, &miss_diffs).await?;
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
        self.reviews()
            .cache
            .lock()
            .unwrap()
            .put(keys.into_iter().zip(built.iter().cloned()));
        Ok(built.into_iter().unzip())
    }
}

/// The workspace's working-copy commit.
async fn at_commit(ws: &Workspace) -> anyhow::Result<String> {
    let at = ws.jj.commit_ids("@").await?;
    at.into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no working copy"))
}
