//! Assess: a read-only agent pass that scores the review's architecture change.

use std::collections::HashSet;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use super::reply::{Known, Reply, SCHEMA, validate};
use super::{AssessmentDto, LayerDto, LookDto, prompt, state};
use crate::agent::{self, Progress, Runs};
use crate::review::{RepoCtx, now_ms};
use crate::store::Store;

const INSTRUCTIONS: &str = "Assess the architecture of this change. Check its structural \
soundness against the existing architecture: layering and dependency direction, coupling and \
cohesion, and trust boundaries. Weigh the reviewer's decisions above, and flag any the change \
breaks. Then make a security pass: for each way the change lets untrusted input reach something \
sensitive or crosses a trust boundary unchecked, add a security look item whose detail gives the \
attack path and its sites as path:line.

Use the component ids above wherever the format asks for a component. Answer with:

- score: 1 to 5, where 5 is safe to merge as is and 1 needs rework.
- risk, and reason: one line on why.
- boundaries: the trust boundaries you infer, each with the components inside it.
- layers: the change as a stack in reading order, starting from what the rest builds on. Each \
has a title, a summary of a sentence or two, the components it touches, and the line ranges that \
make it up (repository-relative paths, 1-based lines on the new side). A layer may span \
components.
- look: what a reviewer should scrutinize, most severe first. Point each at a component, an edge \
(from and to), or a path and line where you can. Leave out anything a reviewer wouldn't act on.";

/// The last Assess run on a revset, stored whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assessed {
    pub assessment: AssessmentDto,
    pub layers: Vec<LayerDto>,
    /// Agent and Security items, unchecked.
    pub look: Vec<LookDto>,
}

fn key(revset: &str) -> String {
    format!("assess:{revset}")
}

/// The last Assess run on `revset`.
pub fn assessed(store: &Store, revset: &str) -> anyhow::Result<Option<Assessed>> {
    Ok(match store.arch_state(&key(revset))? {
        Some(data) => Some(serde_json::from_str(&data)?),
        None => None,
    })
}

pub fn save_assessed(store: &Store, revset: &str, a: &Assessed) -> anyhow::Result<()> {
    Ok(store.save_arch_state(&key(revset), &serde_json::to_string(a)?)?)
}

fn run_key(review: u32) -> String {
    format!("arch-assess-{review}")
}

impl RepoCtx {
    /// Runs Assess over the review, replacing its last assessment. `activity` hears each tool
    /// call. `None` when stopped.
    pub async fn run_assess(
        &self,
        review: u32,
        runs: &Runs,
        activity: impl Fn(String) + Send + Sync + 'static,
    ) -> anyhow::Result<Option<AssessmentDto>> {
        let run = runs.register(&self.agent_key(&run_key(review)))?;
        let (ws, revset, files) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            let files: Vec<String> = r.files.iter().map(|f| f.path.clone()).collect();
            (r.ws.clone(), r.revset.clone(), files)
        };
        let arch = self.arch(review, false, None).await?;
        // The whole review, like the architecture diff, whichever view is open.
        let whole = ws.jj.resolve(&revset).await?;
        let context = crate::bots::describe(&ws, &revset, whole.from, whole.to).await?;
        let prompt = format!(
            "{context}\n\nThe architecture diff from the review's base to its head:\n\n{}\n\n\
             You're in the repository and can read any file. Don't edit files.\n\n{INSTRUCTIONS}",
            prompt::arch_text(&arch)
        );
        let task = agent::Task {
            cwd: &ws.root,
            prompt: &prompt,
            resume: None,
            fork: false,
            edit: false,
            schema: Some(SCHEMA),
            purpose: agent::Purpose::Walkthrough,
        };
        let reply = agent::run_registered(run, task, |p| {
            if let Progress::Tool(t) = p {
                activity(t)
            }
        })
        .await?;
        if reply.stopped {
            return Ok(None);
        }
        let reply: Reply = serde_json::from_value(
            reply
                .structured
                .context("the agent didn't return an assessment")?,
        )
        .context("the agent's assessment doesn't match the format")?;
        let known = Known {
            components: arch
                .components
                .iter()
                .map(|c| c.id.as_str())
                .collect::<HashSet<_>>(),
            files: &files,
        };
        let assessed = validate(reply, &known, now_ms() as f64);
        let store = self.store.lock().unwrap();
        save_assessed(&store, &revset, &assessed)?;
        state::uncheck_assessed(&store, &revset)?;
        Ok(Some(assessed.assessment))
    }

    pub fn stop_assess(&self, review: u32, runs: &Runs) {
        runs.stop(&self.agent_key(&run_key(review)));
    }
}
