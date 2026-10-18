//! Assess: an agent in a terminal scores the review's architecture change and hands the
//! assessment back through lgtm's set_assessment tool.

use std::collections::HashSet;

use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};

use super::reply::{Known, Reply, validate};
use super::{AssessmentDto, LayerDto, LookDto, prompt, state};
use crate::ctx::{RepoCtx, now_ms};
use crate::jj::Range;
use crate::store::Store;

const INSTRUCTIONS: &str = "Assess the architecture of this change. Check its structural \
soundness against the existing architecture: layering and dependency direction, coupling and \
cohesion, and trust boundaries. Weigh the reviewer's decisions above, and flag any the change \
breaks. Then make a security pass: for each way the change lets untrusted input reach something \
sensitive or crosses a trust boundary unchecked, add a security look item whose detail gives the \
attack path and its sites as path:line.

Use the component ids above wherever a component is asked for. Then call lgtm's set_assessment \
tool with the revset and:

- score: 1 to 5, where 5 is safe to merge as is and 1 needs rework.
- risk, and reason: one line on why.
- boundaries: the trust boundaries you infer, each with the components inside it.
- layers: the change as a stack in reading order, starting from what the rest builds on. Each \
has a title, a summary of a sentence or two, the components it touches, and the line ranges that \
make it up (repository-relative paths, 1-based lines on the new side). A layer may span \
components.
- look: what a reviewer should scrutinize, most severe first. Point each at a component, an edge \
(from and to), or a path and line where you can. Leave out anything a reviewer wouldn't act on.

Then tell me the score and why in a line or two.";

/// The last assessment of a revset in a workspace, stored whole.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assessed {
    pub assessment: AssessmentDto,
    pub layers: Vec<LayerDto>,
    /// Agent and Security items, unchecked.
    pub look: Vec<LookDto>,
    /// The commits the revset resolved to when it was stored. Absent from older assessments.
    #[serde(default)]
    pub commits: Option<Commits>,
}

/// A revset's base and head commits. `from` is `None` at the root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commits {
    pub from: Option<String>,
    pub to: String,
}

impl From<&Range> for Commits {
    fn from(r: &Range) -> Self {
        Self {
            from: r.from.clone(),
            to: r.to.clone(),
        }
    }
}

const KIND: &str = "assess";

/// The last assessment of `revset` in workspace `ws`, marked stale unless it was of `now`'s
/// commits.
pub fn assessed(
    store: &Store,
    ws: &str,
    revset: &str,
    now: &Range,
) -> anyhow::Result<Option<Assessed>> {
    let Some(data) = store.arch_review(ws, revset, KIND)? else {
        return Ok(None);
    };
    let mut a: Assessed = serde_json::from_str(&data)?;
    a.assessment.stale = a.commits != Some(now.into());
    Ok(Some(a))
}

pub fn save_assessed(store: &Store, ws: &str, revset: &str, a: &Assessed) -> anyhow::Result<()> {
    Ok(store.save_arch_review(ws, revset, KIND, &serde_json::to_string(a)?)?)
}

impl RepoCtx {
    /// The prompt asking a terminal agent to assess the review's architecture change.
    pub async fn assess_prompt(&self, review: u32) -> anyhow::Result<String> {
        let (ws, revset) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            (r.ws.clone(), r.revset.clone())
        };
        let arch = self.arch(review, false, None).await?;
        // The whole review, like the architecture diff, whichever view is open.
        let whole = ws.jj.resolve(&revset).await?;
        let from = whole.from.as_deref().unwrap_or("root()");
        Ok(format!(
            "The change under review is `{revset}` in this jj repository. Read its diff with \
             `jj diff --from {from} --to {}`. Don't edit files.\n\nThe architecture diff from \
             the review's base to its head:\n\n{}\n\n{INSTRUCTIONS}",
            whole.to,
            prompt::arch_text(&arch)
        ))
    }

    /// Stores `reply`, in set_assessment's format, as the assessment of review `review`,
    /// replacing the last one. It must be about `revset`, the revset the review shows.
    pub async fn save_assessment(
        &self,
        review: u32,
        revset: &str,
        reply: serde_json::Value,
    ) -> anyhow::Result<AssessmentDto> {
        let (ws, shown, files) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            let files: Vec<String> = r.files.iter().map(|f| f.path.clone()).collect();
            (r.ws.clone(), r.revset.clone(), files)
        };
        ensure!(
            shown == revset,
            "lgtm's review now shows `{shown}`, not `{revset}`, so the assessment wasn't stored"
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
        assessed.commits = Some((&ws.jj.resolve(revset).await?).into());
        let store = self.store.lock().unwrap();
        save_assessed(&store, ws.key(), revset, &assessed)?;
        state::uncheck_assessed(&store, ws.key(), revset)?;
        Ok(assessed.assessment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::arch::RiskLevel;

    #[test]
    fn an_assessment_of_other_commits_is_stale() {
        let store = Store::in_memory();
        let range = |to: &str| Range {
            from: Some("base".into()),
            to: to.into(),
        };
        let mut a = Assessed {
            assessment: AssessmentDto {
                score: 4,
                risk: RiskLevel::Low,
                reason: String::new(),
                boundaries: vec![],
                created_at: 0.0,
                stale: false,
            },
            layers: vec![],
            look: vec![],
            commits: Some((&range("head")).into()),
        };
        save_assessed(&store, "w", "@", &a).unwrap();
        let stale = |r: &Range| {
            assessed(&store, "w", "@", r)
                .unwrap()
                .unwrap()
                .assessment
                .stale
        };
        assert!(!stale(&range("head")));
        assert!(stale(&range("moved")));
        assert!(assessed(&store, "", "@", &range("head")).unwrap().is_none());
        // One stored before commits were recorded can't be shown current.
        a.commits = None;
        save_assessed(&store, "w", "@", &a).unwrap();
        assert!(stale(&range("head")));
    }
}
