//! Fixing intents with a headless agent that edits the working copy.

use anyhow::ensure;

use crate::agent::{self, Progress, Runs};
use crate::review::{NOT_AT_WORKING_COPY, RepoCtx};

fn run_key(review: u32) -> String {
    format!("arch-fix-{review}")
}

impl RepoCtx {
    /// Runs an editing agent on the working copy to meet intents `ids`, streaming its reply
    /// through `activity`, and snapshots the working copy after. Returns its reply, `None`
    /// when stopped.
    pub async fn fix_intents(
        &self,
        review: u32,
        ids: &[u32],
        runs: &Runs,
        activity: impl Fn(String) + Send + Sync + 'static,
    ) -> anyhow::Result<Option<String>> {
        let run = runs.register(&self.agent_key(&run_key(review)))?;
        let (ws, revset) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            (r.ws.clone(), r.revset.clone())
        };
        ensure!(
            ws.jj.includes_working_copy(&revset).await?,
            NOT_AT_WORKING_COPY
        );
        let prompt = self.intent_prompt(review, ids).await?;
        let _busy = ws.claim()?;
        let task = agent::Task {
            cwd: &ws.root,
            prompt: &prompt,
            resume: None,
            fork: false,
            edit: true,
            schema: None,
            purpose: agent::Purpose::Ask,
        };
        let reply = agent::run_registered(run, task, |p| {
            if let Progress::Text(t) = p {
                activity(t)
            }
        })
        .await;
        // Show the edits now rather than waiting on the watcher.
        ws.snapshot().await.ok();
        let reply = reply?;
        Ok((!reply.stopped).then_some(reply.text))
    }

    pub fn stop_intent_fix(&self, review: u32, runs: &Runs) {
        runs.stop(&self.agent_key(&run_key(review)));
    }
}
