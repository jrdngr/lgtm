//! What the workspace's agent is doing, for the Agent sidebar tab: its newest turn, each of
//! that turn's edits, and its todo list.

use crate::ctx::{RepoCtx, now_ms};
use crate::events::TurnsChanged;
use crate::store::{AgentActivity, Todo};

impl RepoCtx {
    /// Keeps the agent's todo list for its session.
    pub async fn set_todos(
        &self,
        workspace: Option<&str>,
        session: &str,
        todos: &[Todo],
    ) -> anyhow::Result<()> {
        let ws = self.workspace(workspace).await?;
        self.store
            .lock()
            .unwrap()
            .set_todos(session, todos, now_ms())?;
        self.notify(TurnsChanged {
            workspace: ws.name.clone(),
        });
        Ok(())
    }

    /// The workspace's newest agent turn, if it has one.
    pub async fn agent_activity(
        &self,
        workspace: Option<&str>,
    ) -> anyhow::Result<Option<AgentActivity>> {
        let ws = self.workspace(workspace).await?;
        Ok(self.store.lock().unwrap().agent_activity(ws.key())?)
    }
}
