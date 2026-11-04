//! The app side of `lgtm_arch::view`.

use crate::ctx::RepoCtx;
pub use lgtm_arch::view::*;

impl RepoCtx {
    pub fn arch_view(&self) -> anyhow::Result<ArchViewDto> {
        view(&self.store.lock().unwrap())
    }

    pub fn set_arch_view(&self, v: &ArchViewDto) -> anyhow::Result<()> {
        save_view(&self.store.lock().unwrap(), v)
    }

    /// The map's view and the files it keeps in `workspace`. Untracked files stay when jj
    /// can't list the tracked ones.
    pub(super) async fn view_files(
        &self,
        workspace: Option<&str>,
    ) -> anyhow::Result<(ArchViewDto, Files)> {
        let view = self.arch_view()?;
        let tracked = if view.untracked {
            None
        } else {
            let ws = self.workspace(workspace).await?;
            let files = ws.jj.working_copy_files().await.ok();
            files.map(|f| f.iter().cloned().collect())
        };
        Ok((view, Files { view, tracked }))
    }
}
