//! What lgtm's review names mean in git: `@` the working copy, `@-` its parent, `A::B`
//! the commits from A to B, `A..B` git's range, a local branch its commits since trunk, and
//! anything else git can name one commit.

use anyhow::{Context, bail};

use super::Git;
use crate::jj::Range;

/// The name of the working copy, as in jj.
pub const WORKING_COPY: &str = "@";

impl Git {
    /// The git revision for one name: `@` the working copy's commit, `@-` `HEAD`, `@--`
    /// `HEAD~1` and so on, anything else as it is.
    pub(super) async fn rev(&self, name: &str) -> anyhow::Result<String> {
        let Some(dashes) = name.strip_prefix(WORKING_COPY) else {
            return Ok(name.to_string());
        };
        if !dashes.chars().all(|c| c == '-') {
            return Ok(name.to_string());
        }
        Ok(match dashes.len() {
            0 => self.snapshot().await?,
            1 => "HEAD".into(),
            n => format!("HEAD~{}", n - 1),
        })
    }

    /// The commit `name` names, or an error saying it names none.
    async fn named(&self, name: &str) -> anyhow::Result<String> {
        let rev = self.rev(name).await?;
        self.commit(&rev)
            .await
            .with_context(|| format!("`{name}` names no commit"))
    }

    /// `commit`'s first parent, `None` for a root commit.
    async fn parent(&self, commit: &str) -> Option<String> {
        self.commit(&format!("{commit}^")).await
    }

    /// The branch that's trunk: the remote's default, else `main`, `master` or `trunk`.
    pub(super) async fn trunk(&self) -> Option<String> {
        if let Ok(out) = self
            .read(&["symbolic-ref", "--quiet", "refs/remotes/origin/HEAD"])
            .await
        {
            let r = out.trim();
            if !r.is_empty() {
                return Some(r.to_string());
            }
        }
        for name in ["main", "master", "trunk"] {
            for r in [
                format!("refs/remotes/origin/{name}"),
                format!("refs/heads/{name}"),
            ] {
                if self.commit(&r).await.is_some() {
                    return Some(r);
                }
            }
        }
        None
    }

    async fn merge_base(&self, a: &str, b: &str) -> Option<String> {
        let out = self.read(&["merge-base", a, b]).await.ok()?;
        Some(out.trim().to_string()).filter(|s| !s.is_empty())
    }

    /// The diff endpoints of the review `name`.
    pub async fn resolve(&self, name: &str) -> anyhow::Result<Range> {
        if let Some((a, b)) = name.split_once("::") {
            let (a, to) = tokio::try_join!(self.named(a), self.named(b))?;
            return Ok(Range {
                from: self.parent(&a).await,
                to,
            });
        }
        if let Some((a, b)) = name.split_once("..") {
            let (a, to) = tokio::try_join!(self.named(a), self.named(b))?;
            let from = self.merge_base(&a, &to).await;
            if from.is_none() {
                bail!("`{name}` shares no history");
            }
            return Ok(Range { from, to });
        }
        let to = self.named(name).await?;
        if let Some(from) = self.branch_base(name, &to).await {
            return Ok(Range {
                from: Some(from),
                to,
            });
        }
        Ok(Range {
            from: self.parent(&to).await,
            to,
        })
    }

    /// Where the local branch `name`, at `tip`, leaves trunk, when it has commits of its own.
    async fn branch_base(&self, name: &str, tip: &str) -> Option<String> {
        self.commit(&format!("refs/heads/{name}")).await?;
        let trunk = self.trunk().await?;
        let base = self.merge_base(&trunk, tip).await?;
        (base != tip).then_some(base)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{git, repo, write};

    #[tokio::test]
    async fn a_branch_reviews_its_commits_since_trunk_and_a_commit_reviews_itself() {
        let (dir, g) = repo();
        let d = dir.path();
        let trunk = git(d, &["rev-parse", "HEAD"]);
        git(d, &["switch", "-q", "-c", "topic"]);
        for n in ["b", "c"] {
            write(d, n, "x\n");
            git(d, &["add", "."]);
            git(d, &["commit", "-q", "-m", n]);
        }
        let tip = git(d, &["rev-parse", "HEAD"]);
        let one = git(d, &["rev-parse", "HEAD~1"]);

        let branch = g.resolve("topic").await.unwrap();
        assert_eq!(
            (branch.from.as_deref(), branch.to.as_str()),
            (Some(trunk.as_str()), tip.as_str())
        );
        let commit = g.resolve(&tip).await.unwrap();
        assert_eq!(commit.from.as_deref(), Some(one.as_str()));
        let range = g.resolve(&format!("{one}::topic")).await.unwrap();
        assert_eq!(range.from.as_deref(), Some(trunk.as_str()));
        assert_eq!(g.resolve("main").await.unwrap().from, None);
        assert!(g.resolve("nothing").await.is_err());
    }

    #[tokio::test]
    async fn the_working_copy_sits_on_head() {
        let (dir, g) = repo();
        write(dir.path(), "a.txt", "b\n");
        let head = git(dir.path(), &["rev-parse", "HEAD"]);
        let wc = g.resolve("@").await.unwrap();
        assert_eq!(wc.from.as_deref(), Some(head.as_str()));
        assert_ne!(wc.to, head);
        assert_eq!(g.resolve("@-").await.unwrap().to, head);
    }
}
