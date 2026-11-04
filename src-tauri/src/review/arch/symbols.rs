//! The app side of `lgtm_arch::symbols`.

use crate::ctx::RepoCtx;
pub use lgtm_arch::symbols::*;

impl RepoCtx {
    /// `component`'s symbols, the symbols elsewhere that use them or that they use, and
    /// which uses which.
    pub async fn system_symbols(
        &self,
        workspace: Option<&str>,
        component: String,
    ) -> anyhow::Result<SymbolsDto> {
        self.explore(workspace, move |root, snap, comp| {
            component_symbols(root, snap, comp, &component)
        })
        .await
    }
}
