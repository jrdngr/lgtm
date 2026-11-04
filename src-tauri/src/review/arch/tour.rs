//! The app side of `lgtm_arch::tour`.

use crate::ctx::RepoCtx;
use anyhow::Context;
pub use lgtm_arch::tour::*;

impl RepoCtx {
    /// Stores `reply`, in set_tour's format, as its container's or component's tour,
    /// replacing that node's last one. Returns the node and how many steps it kept.
    pub async fn save_tour(
        &self,
        workspace: Option<&str>,
        reply: serde_json::Value,
    ) -> anyhow::Result<(String, usize)> {
        let input: TourInput =
            serde_json::from_value(reply).context("the tour doesn't match the format")?;
        let map = self.system_map(workspace).await?;
        let ws = self.workspace(workspace).await?;
        let node = input.node.clone();
        let tour = check(input, &map, &ws.root)?;
        let steps = tour.steps.len();
        let store = self.store.lock().unwrap();
        let mut all = stored(&store)?;
        all.insert(node.clone(), tour);
        store.save_arch_state(KEY, &serde_json::to_string(&all)?)?;
        drop(store);
        self.system_changed(workspace);
        Ok((node, steps))
    }
}
