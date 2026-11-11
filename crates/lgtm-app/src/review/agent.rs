//! What an agent in a terminal sees of a review and can do to it, through the MCP server.
//! Agents can read everything; what they can change is in `agent_edits`.

use lgtm_core::review::view::{About, ReviewView, Summary};

use super::{RepoCtx, Review};
use crate::settings;

impl Review {
    pub(super) fn view(&self) -> ReviewView<'_> {
        ReviewView {
            files: &self.files,
            matched: &self.matched,
            checks: &self.checks,
        }
    }

    /// The review's state, as an agent sees it, with `marks` in use or not.
    pub(super) fn summary(&self, marks: bool) -> Summary {
        let about = About {
            marks,
            revset: &self.revset,
            from: self.from.as_deref(),
            to: &self.to,
        };
        self.view().summary(about, &settings::get().marks)
    }
}

impl RepoCtx {
    /// Runs `f` on the review open in `workspace`'s current review pane.
    pub fn with_current<T>(
        &self,
        workspace: Option<&str>,
        f: impl FnOnce(&Review) -> T,
    ) -> anyhow::Result<(u32, T)> {
        let id = self.current_review(workspace)?;
        let review = self.review(id)?;
        let review = review.lock().unwrap();
        Ok((id, f(&review)))
    }

    /// The current review's state, with marks in use as the settings say.
    pub fn summary(&self, workspace: Option<&str>) -> anyhow::Result<Summary> {
        self.summary_with(workspace, settings::get().review_marks)
    }

    pub fn summary_with(&self, workspace: Option<&str>, marks: bool) -> anyhow::Result<Summary> {
        Ok(self.with_current(workspace, |r| r.summary(marks))?.1)
    }
}
