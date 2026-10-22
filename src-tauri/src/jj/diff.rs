//! Diffs between commits, from jj's template metadata joined with its `--git` output.

use std::collections::HashMap;

use anyhow::{Context, bail};

use super::Jj;
use crate::diff::{self, FileMeta, FileStatus, GitFileDiff, Hunk};
use crate::gitmeta;

/// A review's diff endpoints. `from` is `None` when the review reaches the root commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Range {
    pub from: Option<String>,
    pub to: String,
}

impl Range {
    /// `from`, or the root commit.
    fn base(&self) -> &str {
        self.from.as_deref().unwrap_or("root()")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    /// Source path for renames and copies.
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub binary: bool,
    pub conflict: bool,
    pub old_exec: bool,
    pub new_exec: bool,
    /// Abbreviated blob ids from the git `index` line, `old..new`.
    pub blobs: Option<String>,
    pub hunks: Vec<Hunk>,
}

impl Jj {
    /// Resolves a review revset R to `heads(::R ~ R)` → `heads(R)`. For a stack this is the
    /// same diff as `jj diff -r R`; for R with gaps (a PR that merged trunk) it matches
    /// GitHub's PR diff. Anything else is rejected with jj's own error.
    pub async fn resolve(&self, revset: &str) -> anyhow::Result<Range> {
        let (heads, bases) = tokio::try_join!(
            self.commit_ids(&format!("heads({revset})")),
            self.commit_ids(&format!("heads(::({revset}) ~ ({revset}))")),
        )?;
        match (heads.as_slice(), bases.as_slice()) {
            ([], _) => bail!("revset `{revset}` is empty"),
            ([to], []) => Ok(Range {
                from: None,
                to: to.clone(),
            }),
            ([to], [from]) => Ok(Range {
                from: Some(from.clone()),
                to: to.clone(),
            }),
            _ => {
                self.read_lossy(&["diff", "--stat", "-r", revset]).await?;
                bail!(
                    "revset `{revset}` has {} heads and {} bases; a review needs one of each",
                    heads.len(),
                    bases.len()
                )
            }
        }
    }

    pub async fn diff(&self, range: &Range) -> anyhow::Result<Vec<FileDiff>> {
        let meta_args = Self::range_args(range, &["-T", diff::META_TEMPLATE]);
        let git_args = Self::range_args(range, &["--git"]);
        let (meta, git) = tokio::try_join!(self.read(&meta_args), self.read_lossy(&git_args))?;
        join(diff::parse_meta(&meta)?, diff::parse_git(&git)?)
    }

    fn range_args<'a>(range: &'a Range, extra: &[&'a str]) -> Vec<&'a str> {
        [
            &["diff", "--from", range.base(), "--to", &range.to][..],
            extra,
        ]
        .concat()
    }

    /// What `from`'s changes do differently from `to`'s: `from` rebased onto `to`'s parents,
    /// compared with `to`.
    pub async fn interdiff(&self, from: &str, to: &str) -> anyhow::Result<Vec<FileDiff>> {
        let git = self
            .read_lossy(&["interdiff", "--from", from, "--to", to, "--git"])
            .await?;
        Ok(diff::parse_git(&git)?
            .into_iter()
            .map(|g| file_diff(gitmeta::meta_from_git(&g), g))
            .collect())
    }
}

/// Joins template metadata with `--git` sections by their `diff --git` header.
fn join(meta: Vec<FileMeta>, git: Vec<GitFileDiff>) -> anyhow::Result<Vec<FileDiff>> {
    let mut git: HashMap<String, GitFileDiff> =
        git.into_iter().map(|g| (g.header.clone(), g)).collect();
    meta.into_iter()
        .map(|m| {
            let header = format!("a/{} b/{}", m.source, m.path);
            let g = git
                .remove(&header)
                .with_context(|| format!("no git diff for `{header}`"))?;
            Ok(file_diff(m, g))
        })
        .collect()
}

fn file_diff(m: FileMeta, g: GitFileDiff) -> FileDiff {
    let moved = matches!(m.status, FileStatus::Renamed | FileStatus::Copied);
    FileDiff {
        old_path: moved.then_some(m.source),
        path: m.path,
        status: m.status,
        binary: g.binary,
        conflict: m.conflict,
        old_exec: m.source_exec,
        new_exec: m.target_exec,
        blobs: g.extended.iter().find_map(|l| {
            let ids = l.strip_prefix("index ")?;
            Some(ids.split(' ').next().unwrap_or(ids).to_string())
        }),
        hunks: g.hunks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("jj")
            .args(["git", "init"])
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(ok.status.success());
        dir
    }

    #[tokio::test]
    async fn diffs_unusual_names_and_non_utf8_content() {
        let dir = init();
        std::fs::write(dir.path().join("üni"), b"caf\xe9\n").unwrap();
        std::fs::write(dir.path().join("quo\"te"), "x\n").unwrap();
        let jj = Jj::new(dir.path());
        jj.snapshot().await.unwrap();
        let mut files = jj.diff(&jj.resolve("@").await.unwrap()).await.unwrap();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let paths: Vec<_> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["quo\"te", "üni"]);
        assert!(files.iter().all(|f| !f.hunks.is_empty()));
    }
}
