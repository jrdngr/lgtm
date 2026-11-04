//! The app side of `lgtm_arch::describe`.

use crate::ctx::RepoCtx;
use crate::events::SystemChanged;
use anyhow::Context;
pub use lgtm_arch::describe::*;

impl RepoCtx {
    /// The prompt asking a terminal agent to describe `workspace`'s system map.
    pub async fn describe_prompt(&self, workspace: Option<&str>) -> anyhow::Result<String> {
        let map = self.system_map(workspace).await?;
        Ok(format!(
            "The system map of this repository, from its imports:\n\n{}\n{INSTRUCTIONS}",
            map_text(&map)
        ))
    }

    /// Stores `reply`, in set_system's format, as the description of the system map,
    /// replacing the last one.
    pub async fn save_description(
        &self,
        workspace: Option<&str>,
        reply: serde_json::Value,
    ) -> anyhow::Result<SystemDescDto> {
        let d: Description =
            serde_json::from_value(reply).context("the description doesn't match the format")?;
        let map = self.system_map(workspace).await?;
        let ws = self.workspace(workspace).await?;
        let commit = ws
            .jj
            .commit_ids("@")
            .await?
            .into_iter()
            .next()
            .context("no working-copy commit")?;
        let store = self.store.lock().unwrap();
        let desc = validate(d, &map, commit, described(&store)?);
        store.save_arch_state(KEY, &serde_json::to_string(&desc)?)?;
        drop(store);
        self.system_changed(workspace);
        Ok(desc)
    }

    /// Has `workspace`'s window show the system map, which an agent described or toured.
    pub(super) fn system_changed(&self, workspace: Option<&str>) {
        self.notify(SystemChanged {
            workspace: workspace.map(str::to_string),
        });
    }
}
