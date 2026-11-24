//! `lgtm review --pr <n>` with no `-r`: the PR's own revisions, as a revset.

use std::path::Path;

use anyhow::bail;
use lgtm_core::github::{canonical, pr, remote};
use lgtm_core::repo::Repo;

/// The revset of the PR `typed`, from its base to its head.
pub fn revset(root: &Path, typed: &str) -> anyhow::Result<String> {
    crate::snap::block_on(async {
        let local = Repo::open(root);
        let (repo, number) = remote::parse_pr(typed)?;
        let repo = match repo {
            Some(r) => r,
            None => remote::pick(&canonical::remotes(&local).await?, None)?.2,
        };
        let (base, head) = pr::ends(&repo, number).await?;
        let here = local.commit_ids(&head).await.is_ok_and(|c| !c.is_empty());
        if !here {
            bail!(
                "PR #{number}'s head {} isn't in this repo; fetch it first",
                &head[..head.len().min(12)]
            );
        }
        Ok(format!("{base}..{head}"))
    })
}
