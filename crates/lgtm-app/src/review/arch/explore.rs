//! The app side of `lgtm_arch::explore`.

use super::{model, system};
use crate::ctx::RepoCtx;
pub use lgtm_arch::explore::*;
use std::path::Path;

impl RepoCtx {
    /// Runs `f` on `workspace`'s working copy: its root, its files' imports and their
    /// components.
    pub(super) async fn explore<T: Send + 'static>(
        &self,
        workspace: Option<&str>,
        f: impl FnOnce(&Path, &model::Snapshot, &dyn Fn(&str) -> String) -> T + Send + 'static,
    ) -> anyhow::Result<T> {
        let ws = self.workspace(workspace).await?;
        let repo = super::load::repo_name(&self.root);
        let root = ws.root.clone();
        let (_, files) = self.view_files(workspace).await?;
        Ok(tokio::task::spawn_blocking(move || {
            let (snap, g) = system::scanned(&root, &repo, &files);
            f(&root, &snap, &|p| g.comp(p))
        })
        .await?)
    }

    /// `component`'s symbols other components use most.
    pub async fn system_most_used(
        &self,
        workspace: Option<&str>,
        component: String,
    ) -> anyhow::Result<Vec<SysSymbolDto>> {
        self.explore(workspace, move |root, snap, comp| {
            most_used(root, snap, comp, &component)
        })
        .await
    }

    /// Symbols in the working copy whose names hold `query`.
    pub async fn system_find(
        &self,
        workspace: Option<&str>,
        query: String,
    ) -> anyhow::Result<Vec<SysSymbolDto>> {
        if query.trim().len() < 2 {
            return Ok(vec![]);
        }
        self.explore(workspace, move |root, snap, comp| {
            find(root, snap, comp, query.trim())
        })
        .await
    }
}
