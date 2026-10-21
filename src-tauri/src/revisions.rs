//! The revisions log: each workspace's `jj log` rows, under the revset the user or an agent
//! set for it, else jj's default.

use serde::Serialize;
use specta::Type;

use crate::ctx::RepoCtx;
use crate::jj::LogRow;

#[derive(Serialize, Type)]
pub struct LogView {
    /// The workspace's log revset; `None` is jj's default.
    pub revset: Option<String>,
    pub rows: Vec<LogRow>,
    /// jj's error for `revset`, when the rows fell back to jj's default.
    pub error: Option<String>,
}

impl RepoCtx {
    pub fn log_revset(&self, workspace: Option<&str>) -> anyhow::Result<Option<String>> {
        Ok(self.store.lock().unwrap().log_revset(workspace.unwrap_or(""))?)
    }

    /// The workspace's log. A revset jj no longer takes, such as one naming a deleted
    /// bookmark, shows jj's default rows with jj's error.
    pub async fn log_view(&self, workspace: Option<&str>) -> anyhow::Result<LogView> {
        let jj = &self.workspace(workspace).await?.jj;
        let revset = self.log_revset(workspace)?;
        let (rows, error) = match &revset {
            None => (jj.log_rows(None).await?, None),
            Some(r) => match jj.log_rows(Some(r)).await {
                Ok(rows) => (rows, None),
                Err(e) => (jj.log_rows(None).await?, Some(format!("{e:#}"))),
            },
        };
        Ok(LogView { revset, rows, error })
    }

    /// Sets the workspace's log revset once jj takes it. Blank goes back to jj's default.
    pub async fn set_log_revset(&self, workspace: Option<&str>, revset: &str) -> anyhow::Result<()> {
        let revset = Some(revset.trim()).filter(|r| !r.is_empty());
        if let Some(r) = revset {
            self.workspace(workspace).await?.jj.log_rows(Some(r)).await?;
        }
        Ok(self
            .store
            .lock()
            .unwrap()
            .set_log_revset(workspace.unwrap_or(""), revset)?)
    }
}

#[cfg(test)]
mod tests {
    use crate::review::tests::{jj, repo};

    fn descriptions(view: &super::LogView) -> Vec<String> {
        view.rows
            .iter()
            .filter_map(|r| r.commit.as_ref())
            .map(|c| c.description.trim().to_string())
            .collect()
    }

    #[tokio::test]
    async fn the_log_lists_its_revset_and_refuses_one_jj_cant_take() {
        let (dir, ctx) = repo();
        jj(dir.path(), &["describe", "-m", "one"]);
        jj(dir.path(), &["new", "-m", "two"]);
        let all = descriptions(&ctx.log_view(None).await.unwrap());
        assert!(all.contains(&"one".into()) && all.contains(&"two".into()));

        ctx.set_log_revset(None, " @- ").await.unwrap();
        let view = ctx.log_view(None).await.unwrap();
        assert_eq!(view.revset.as_deref(), Some("@-"));
        assert_eq!(descriptions(&view), ["one"]);

        let refused = ctx.set_log_revset(None, "author(").await.unwrap_err();
        assert!(!format!("{refused:#}").contains("-T"), "{refused:#}");
        assert_eq!(ctx.log_revset(None).unwrap().as_deref(), Some("@-"));

        ctx.set_log_revset(None, "").await.unwrap();
        assert_eq!(descriptions(&ctx.log_view(None).await.unwrap()), all);
    }

    #[tokio::test]
    async fn a_saved_revset_jj_no_longer_takes_shows_the_default_log_with_its_error() {
        let (dir, ctx) = repo();
        jj(dir.path(), &["bookmark", "create", "gone", "-r", "@"]);
        ctx.set_log_revset(None, "gone").await.unwrap();
        jj(dir.path(), &["bookmark", "delete", "gone"]);
        let view = ctx.log_view(None).await.unwrap();
        assert_eq!(view.revset.as_deref(), Some("gone"));
        assert!(view.error.as_deref().unwrap().contains("gone"));
        assert!(!view.rows.is_empty());
    }
}
