//! The app side of `lgtm_arch::assess`.

use super::reply::{Known, Reply, validate};
use super::{AssessmentDto, prompt, state};
use crate::ctx::{RepoCtx, now_ms};
use crate::events::ArchChanged;
use anyhow::{Context, ensure};
pub use lgtm_arch::assess::*;
use std::collections::HashSet;

impl RepoCtx {
    /// The prompt asking a terminal agent to assess the review's architecture change.
    pub async fn assess_prompt(&self, review: u32) -> anyhow::Result<String> {
        let (revset, whole) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            (r.revset.clone(), r.whole.clone())
        };
        let arch = self.arch(review, false, None).await?;
        // The whole review as loaded, like the architecture diff, whichever view is open.
        let from = base(&whole);
        let to = &whole.to;
        Ok(format!(
            "The change under review is `{revset}` in this jj repository, from {from} to {to}. \
             Read its diff with `jj diff --from {from} --to {to}`. Don't edit files.\n\nThe \
             architecture diff from the review's base to its head:\n\n{}\n\n{INSTRUCTIONS}",
            prompt::arch_text(&arch)
        ))
    }

    /// Stores `reply`, in set_assessment's format, as the assessment of review `review`,
    /// replacing the last one. It must be about `revset`, the revset the review shows, from
    /// `from` to `to`, the commits it shows, as the Assess prompt named them.
    pub async fn save_assessment(
        &self,
        review: u32,
        revset: &str,
        (from, to): (&str, &str),
        reply: serde_json::Value,
    ) -> anyhow::Result<AssessmentDto> {
        let (ws, shown, whole, files) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            let files: Vec<String> = r.files.iter().map(|f| f.path.clone()).collect();
            (r.ws.clone(), r.revset.clone(), r.whole.clone(), files)
        };
        ensure!(
            shown == revset,
            "lgtm's review now shows `{shown}`, not `{revset}`, so the assessment wasn't stored"
        );
        ensure!(
            (from, to) == (base(&whole), whole.to.as_str()),
            "lgtm's review of `{revset}` moved to other commits since the Assess prompt, so the \
             assessment wasn't stored; tell the user to Assess again"
        );
        let reply: Reply =
            serde_json::from_value(reply).context("the assessment doesn't match the format")?;
        let arch = self.arch(review, false, None).await?;
        let known = Known {
            components: arch
                .components
                .iter()
                .map(|c| c.id.as_str())
                .collect::<HashSet<_>>(),
            files: &files,
        };
        let mut assessed = validate(reply, &known, now_ms() as f64);
        assessed.commits = Some((&whole).into());
        let store = self.store.lock().unwrap();
        save_assessed(&store, ws.key(), revset, &assessed)?;
        state::uncheck_assessed(&store, ws.key(), revset)?;
        drop(store);
        self.notify(ArchChanged {
            workspace: ws.name.clone(),
            review,
        });
        Ok(assessed.assessment)
    }
}
