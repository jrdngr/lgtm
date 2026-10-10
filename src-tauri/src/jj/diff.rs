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
    pub hunks: Vec<Hunk>,
}

impl Jj {
    /// Resolves a review revset R to `heads(::R ~ R)` → `heads(R)`. For a stack this is the
    /// same diff as `jj diff -r R`; for R with gaps (a PR that merged trunk) it matches
    /// GitHub's PR diff. Anything else is rejected with jj's own error.
    pub async fn resolve(&self, revset: &str) -> anyhow::Result<Range> {
        let heads = self.commit_ids(&format!("heads({revset})")).await?;
        let bases = self
            .commit_ids(&format!("heads(::({revset}) ~ ({revset}))"))
            .await?;
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
                self.read(&["diff", "--stat", "-r", revset]).await?;
                bail!(
                    "revset `{revset}` has {} heads and {} bases; a review needs one of each",
                    heads.len(),
                    bases.len()
                )
            }
        }
    }

    pub async fn diff(&self, range: &Range) -> anyhow::Result<Vec<FileDiff>> {
        let from = range.from.as_deref().unwrap_or("root()");
        let base = ["diff", "--from", from, "--to", &range.to];
        let meta_args = [&base[..], &["-T", diff::META_TEMPLATE]].concat();
        let git_args = [&base[..], &["--git"]].concat();
        let (meta, git) = tokio::try_join!(self.read(&meta_args), self.read(&git_args))?;
        join(diff::parse_meta(&meta)?, diff::parse_git(&git)?)
    }

    fn range_args<'a>(range: &'a Range, extra: &[&'a str]) -> Vec<&'a str> {
        let from = range.from.as_deref().unwrap_or("root()");
        [&["diff", "--from", from, "--to", &range.to][..], extra].concat()
    }

    /// The range's diff as `jj diff --git` prints it.
    pub async fn git_diff(&self, range: &Range) -> anyhow::Result<String> {
        self.read(&Self::range_args(range, &["--git"])).await
    }

    pub async fn diff_stat(&self, range: &Range) -> anyhow::Result<String> {
        self.read(&Self::range_args(range, &["--stat"])).await
    }

    /// Each commit in the range, oldest first: its short change id and full description.
    pub async fn descriptions(&self, range: &Range) -> anyhow::Result<String> {
        let from = range.from.as_deref().unwrap_or("root()");
        let revset = format!("{from}..{}", range.to);
        self.read(&[
            "log",
            "--no-graph",
            "--reversed",
            "-r",
            &revset,
            "-T",
            r#"change_id.short(8) ++ " " ++ if(description, description, "(no description)\n") ++ "\n""#,
        ])
        .await
    }

    /// What `from`'s changes do differently from `to`'s: `from` rebased onto `to`'s parents,
    /// compared with `to`.
    pub async fn interdiff(&self, from: &str, to: &str) -> anyhow::Result<Vec<FileDiff>> {
        let git = self
            .read(&["interdiff", "--from", from, "--to", to, "--git"])
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
            let header = format!("{} {}", git_path("a/", &m.source), git_path("b/", &m.path));
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
        hunks: g.hunks,
    }
}

/// A path as git writes it in a `diff --git` header: C-quoted when it holds control
/// characters, quotes, backslashes, or non-ASCII bytes.
fn git_path(prefix: &str, path: &str) -> String {
    let full = format!("{prefix}{path}");
    if !full
        .bytes()
        .any(|b| !(0x20..0x7f).contains(&b) || b == b'"' || b == b'\\')
    {
        return full;
    }
    let mut out = String::from("\"");
    for b in full.bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            0x20..0x7f => out.push(b as char),
            _ => out.push_str(&format!("\\{b:03o}")),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_path_quotes_like_git() {
        assert_eq!(git_path("a/", "src/main.rs"), "a/src/main.rs");
        assert_eq!(git_path("a/", "with space"), "a/with space");
        assert_eq!(git_path("b/", "é\"x"), r#""b/\303\251\"x""#);
    }

    #[tokio::test]
    async fn stat_paths_are_repo_relative() {
        let dir = tempfile::tempdir().unwrap();
        let ok = std::process::Command::new("jj")
            .args(["git", "init"])
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(ok.status.success());
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/a.rs"), "x\n").unwrap();
        let jj = Jj::new(dir.path());
        jj.snapshot().await.unwrap();
        let stat = jj.diff_stat(&jj.resolve("@").await.unwrap()).await.unwrap();
        assert!(stat.starts_with("src/a.rs"), "{stat}");
    }
}
