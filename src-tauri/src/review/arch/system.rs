//! The app side of `lgtm_arch::system`.

use super::describe::{self};
use super::tour::{self};
use crate::ctx::RepoCtx;
use crate::jj::Range;
pub use lgtm_arch::system::*;
use std::collections::BTreeSet;

impl RepoCtx {
    /// The system map of `workspace`'s working copy.
    pub async fn system_map(&self, workspace: Option<&str>) -> anyhow::Result<SystemDto> {
        let ws = self.workspace(workspace).await?;
        let repo = super::load::repo_name(&self.root);
        let (description, stored) = {
            let store = self.store.lock().unwrap();
            (describe::described(&store)?, tour::stored(&store)?)
        };
        let root = ws.root.clone();
        let repo_name = repo.clone();
        let (view, files) = self.view_files(workspace).await?;
        let (st, path_comps) = tokio::task::spawn_blocking(move || {
            let (snap, g) = scanned(&root, &repo_name, &files);
            let st = without_small(structure(&snap, &g), &view);
            (st, move |p: &str| g.comp(p))
        })
        .await?;
        let Structure {
            containers,
            components,
            edges,
            files,
        } = st;
        let stale = match &description {
            Some(d) => {
                let range = Range {
                    from: Some(d.commit.clone()),
                    to: "@".into(),
                };
                // A commit jj can no longer read leaves nothing to compare with.
                match ws.jj.diff_paths(&range).await {
                    Ok(diffs) => diffs
                        .iter()
                        .map(|f| path_comps(&f.path))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .filter(|id| components.iter().any(|c: &SysComponentDto| c.id == *id))
                        .collect(),
                    Err(_) => vec![],
                }
            }
            None => vec![],
        };
        let tours = tour::tours(description.as_ref(), &stored, &containers, &components);
        let description = description.map(|d| viewed(d, &view));
        Ok(SystemDto {
            repo,
            containers,
            components,
            edges,
            files,
            description,
            stale,
            tours,
            grouped: view.groups,
        })
    }
}
