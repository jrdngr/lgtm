//! Findings an agent in a terminal hands back through the MCP server, stored as a finished
//! run so they show and act like a persona's.

use super::output::Output;
use super::{BotOptions, BotRun, Part, Persona, Status, Step};
use crate::review::{RepoCtx, now_ms};

impl RepoCtx {
    /// Stores `review` (a verdict and findings, as a persona returns them) from `reviewer`
    /// as a run against the workspace's current review.
    pub fn submit_findings(
        &self,
        workspace: Option<&str>,
        reviewer: &str,
        review: serde_json::Value,
    ) -> anyhow::Result<BotRun> {
        let (_, (revset, from, to)) = self.with_current(workspace, |r| r.target())?;
        let output = Output::parse(Step::Reports, Some(reviewer), review)?;
        let mut run = BotRun {
            id: 0,
            workspace: workspace.map(str::to_string),
            revset,
            from,
            to,
            personas: vec![Persona {
                name: reviewer.to_string(),
                description: "an agent in a terminal".into(),
                off_lens: false,
                profile: String::new(),
            }],
            options: BotOptions::default(),
            context: String::new(),
            parts: vec![Part {
                status: Status::Done,
                output: Some(output),
                ..Part::running(Step::Reports, Some(reviewer.to_string()))
            }],
            done: vec![Step::Reports],
            created_at: now_ms() as f64,
            fixes: vec![],
            dismissed: vec![],
            synthesis: 0,
            active: false,
        };
        run.id = self.store.lock().unwrap().add_bot_run(&run)?;
        Ok(run)
    }
}
