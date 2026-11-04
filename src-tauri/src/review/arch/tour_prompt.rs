//! The app side of `lgtm_arch::tour_prompt`.

use crate::ctx::RepoCtx;
pub use lgtm_arch::tour_prompt::*;

impl RepoCtx {
    /// The prompt asking a terminal agent to write the overview or features tour, or the tour
    /// of a container or component.
    pub async fn tour_prompt(&self, workspace: Option<&str>, node: &str) -> anyhow::Result<String> {
        let map = self.system_map(workspace).await?;
        prompt(&map, node)
    }
}
