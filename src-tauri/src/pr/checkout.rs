//! Checking a PR out into its own workspace. Same-repo heads are fetched from the repo's
//! remote, fork heads from a `gh-<owner>` remote added on demand, and heads whose branch is
//! gone from `pull/N/head` into an `lgtm/pr-N` bookmark.

use anyhow::{Context, bail};

use super::{PrRecord, fallback_bookmark, workspace_name};
use crate::github::{self, GhRepo, PrInfo, remote};
use crate::jj::Jj;
use crate::review::RepoCtx;

/// Where a PR's base comes from.
pub(super) struct Base {
    pub remote: String,
    pub url: String,
}

/// Where a fetch got the PR's head from.
pub(super) struct Fetched {
    /// The remote the head came from, or `None` when it came from `pull/N/head`.
    pub head_remote: Option<String>,
    /// A fork remote this fetch added, for cleanup to remove.
    pub added_remote: Option<String>,
}

/// Fetches the PR's base and head.
pub(super) async fn fetch(jj: &Jj, info: &PrInfo, base: &Base) -> anyhow::Result<Fetched> {
    jj.fetch(&base.remote, &[&info.base_ref_name])
        .await
        .with_context(|| format!("fetch {} from {}", info.base_ref_name, base.remote))?;
    let mut added_remote = None;
    let head_remote = match (&info.head_repository, info.is_cross_repository) {
        (Some(_), false) => Some(base.remote.clone()),
        (Some(h), true) => {
            let name = remote::fork_remote(&h.owner.login);
            if !jj.remotes().await?.iter().any(|(n, _)| *n == name) {
                let repo = GhRepo {
                    owner: h.owner.login.clone(),
                    name: h.name.clone(),
                };
                jj.remote_add(&name, &remote::url_like(&base.url, &repo))
                    .await?;
                added_remote = Some(name.clone());
            }
            Some(name)
        }
        (None, _) => None,
    };
    if let Some(r) = &head_remote {
        // A deleted branch fails here, and the fallback below covers it.
        jj.fetch(r, &[&info.head_ref_name]).await.ok();
        if jj.has_commit(&info.head_ref_oid).await {
            return Ok(Fetched {
                head_remote,
                added_remote,
            });
        }
    }
    let refspec = format!(
        "+refs/pull/{}/head:refs/heads/{}",
        info.number,
        fallback_bookmark(info.number)
    );
    jj.git_fetch_ref(&base.remote, &refspec).await?;
    if !jj.has_commit(&info.head_ref_oid).await {
        bail!(
            "fetched PR #{} but its head {} isn't there",
            info.number,
            info.head_ref_oid
        );
    }
    Ok(Fetched {
        head_remote: None,
        added_remote,
    })
}

impl RepoCtx {
    /// The remote for `want`, or the repo PR numbers refer to.
    pub(super) async fn base_for(&self, want: Option<&GhRepo>) -> anyhow::Result<(Base, GhRepo)> {
        let remotes = github::canonical::remotes(&self.main.jj).await?;
        let want = match want {
            Some(w) => Some(github::canonical::current(w).await),
            None => None,
        };
        let (remote, url, repo) = remote::pick(&remotes, want.as_ref())?;
        Ok((Base { remote, url }, repo))
    }

    /// The PR's base remote as stored, or another for its repo when that one is gone.
    pub(super) async fn base_of(&self, pr: &PrRecord) -> anyhow::Result<Base> {
        let remotes = github::canonical::remotes(&self.main.jj).await?;
        if let Some((n, u)) = remotes.iter().find(|(n, _)| *n == pr.base_remote) {
            return Ok(Base {
                remote: n.clone(),
                url: u.clone(),
            });
        }
        let (remote, url, _) = remote::pick(&remotes, Some(&pr.repo))?;
        Ok(Base { remote, url })
    }

    /// Checks out the PR `input` names (a number or URL) into workspace `pr-<N>`, or finds the
    /// one already checked out. Returns the workspace and whether it has a setup script.
    pub async fn open_pr(&self, input: &str) -> anyhow::Result<(String, bool)> {
        let (want, number) = remote::parse_pr(input)?;
        let (base, repo) = self.base_for(want.as_ref()).await?;
        let name = workspace_name(number);
        let listed = self.list_workspaces().await?;
        if listed.iter().any(|w| w.name.as_deref() == Some(&name)) {
            return match self.pr_record(Some(&name))? {
                Some(pr) if pr.repo == repo => Ok((name, false)),
                Some(pr) => bail!("workspace {name} is already {}'s PR #{number}", pr.repo),
                None => bail!("a workspace named {name} already exists"),
            };
        }
        let info = github::pr::info(&repo, number).await?;
        let fetched = fetch(&self.main.jj, &info, &base).await?;
        let setup = self.add_workspace(&name, &info.head_ref_oid).await?;
        let record = PrRecord {
            workspace: name.clone(),
            repo,
            number,
            id: info.id,
            title: info.title,
            url: info.url,
            author: github::login(&info.author),
            base_ref: info.base_ref_name,
            base_remote: base.remote,
            head_ref: info.head_ref_name,
            head_remote: fetched.head_remote,
            added_remote: fetched.added_remote,
            head: info.head_ref_oid,
        };
        self.store.lock().unwrap().save_pr(&record)?;
        Ok((name, setup))
    }
}
