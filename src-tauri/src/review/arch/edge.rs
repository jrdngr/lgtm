//! The app side of `lgtm_arch::edge`.

use crate::ctx::RepoCtx;
pub use lgtm_arch::edge::*;

impl RepoCtx {
    /// Where component `from` imports component `to`, and the symbols it uses there.
    pub async fn system_edge(
        &self,
        workspace: Option<&str>,
        from: String,
        to: String,
    ) -> anyhow::Result<EdgeSitesDto> {
        self.explore(workspace, move |root, snap, comp| {
            edge_sites(root, snap, comp, &from, &to)
        })
        .await
    }
}
