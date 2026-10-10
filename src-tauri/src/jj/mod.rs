//! The only code that runs jj. Reads skip the working-copy snapshot; writes take the
//! workspace's writer lock so two writing commands never overlap.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, bail};
use tokio::process::Command;
use tokio::sync::Mutex;

use crate::diff::{self, FileMeta, FileStatus, GitFileDiff, Hunk};
use crate::gitmeta;

mod edit;
pub mod workspace;
pub use edit::JjAction;
pub use workspace::WorkspaceInfo;

/// True when `err` came from jj refusing to work on a stale working copy.
pub fn is_stale(err: &anyhow::Error) -> bool {
    format!("{err:#}").contains("working copy is stale")
}

/// One line of `jj log` output, with ANSI colors.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LogLine {
    pub text: String,
    /// Set on the first line of each commit.
    pub commit_id: Option<String>,
    pub change_id: Option<String>,
}

/// Splits out the `\x1f`-separated ids `log_graph` puts in front of each commit's template.
fn parse_log(out: &str) -> Vec<LogLine> {
    out.lines()
        .map(|line| {
            let mut parts = line.splitn(4, '\x1f');
            let prefix = parts.next().unwrap_or_default();
            match (parts.next(), parts.next(), parts.next()) {
                (Some(commit), Some(change), Some(rest)) => LogLine {
                    text: format!("{prefix}{rest}"),
                    commit_id: Some(commit.to_string()),
                    change_id: Some(change.to_string()),
                },
                _ => LogLine {
                    text: line.to_string(),
                    commit_id: None,
                    change_id: None,
                },
            }
        })
        .collect()
}

/// A change in a review's stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub change_id: String,
    pub commit_id: String,
    /// Length of the shortest prefix of `change_id` that's unique in the repo.
    pub unique: u32,
    /// Whether it's the workspace's working-copy commit.
    pub working_copy: bool,
    pub description: String,
}

const CHANGE_TEMPLATE: &str = r#"change_id.short(12) ++ "\t" ++ commit_id ++ "\t"
  ++ change_id.shortest().prefix().len() ++ "\t" ++ if(current_working_copy, "1", "0") ++ "\t"
  ++ description.first_line() ++ "\n""#;

fn parse_changes(out: &str) -> Vec<Change> {
    out.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(5, '\t');
            Some(Change {
                change_id: parts.next()?.to_string(),
                commit_id: parts.next()?.to_string(),
                unique: parts.next()?.parse().ok()?,
                working_copy: parts.next()? == "1",
                description: parts.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

pub struct Jj {
    root: PathBuf,
    writer: Mutex<()>,
}

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
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            writer: Mutex::new(()),
        }
    }

    async fn exec(&self, args: &[&str], read_only: bool) -> anyhow::Result<Vec<u8>> {
        self.exec_with(args, read_only, "never").await
    }

    async fn exec_with(
        &self,
        args: &[&str],
        read_only: bool,
        color: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let mut cmd = Command::new("jj");
        cmd.arg("-R")
            .arg(&self.root)
            .arg("--no-pager")
            .arg(format!("--color={color}"));
        if read_only {
            cmd.arg("--ignore-working-copy");
        }
        let out = cmd
            .args(args)
            .stdin(Stdio::null())
            .output()
            .await
            .context("run jj (is it installed and on PATH?)")?;
        if !out.status.success() {
            bail!(
                "jj {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(out.stdout)
    }

    async fn read(&self, args: &[&str]) -> anyhow::Result<String> {
        String::from_utf8(self.exec(args, true).await?).context("jj output is not UTF-8")
    }

    /// Snapshots the working copy. Fails with an error that `is_stale` recognizes when the
    /// working copy is stale.
    pub async fn snapshot(&self) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(&["util", "snapshot"], false).await.map(drop)
    }

    pub async fn commit_ids(&self, revset: &str) -> anyhow::Result<Vec<String>> {
        let out = self
            .read(&[
                "log",
                "--no-graph",
                "-r",
                revset,
                "-T",
                r#"commit_id ++ "\n""#,
            ])
            .await?;
        Ok(out.lines().map(str::to_string).collect())
    }

    /// Whether `revset` includes the workspace's working-copy commit.
    pub async fn includes_working_copy(&self, revset: &str) -> anyhow::Result<bool> {
        Ok(!self
            .commit_ids(&format!("@ & ({revset})"))
            .await?
            .is_empty())
    }

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

    /// The changes in `revset`, oldest first.
    pub async fn changes(&self, revset: &str) -> anyhow::Result<Vec<Change>> {
        let out = self
            .read(&[
                "log",
                "--no-graph",
                "--reversed",
                "-r",
                revset,
                "-T",
                CHANGE_TEMPLATE,
            ])
            .await?;
        Ok(parse_changes(&out))
    }

    /// Commit IDs `commit` evolved from, newest first, starting with `commit` itself.
    pub async fn evolog(&self, commit: &str) -> anyhow::Result<Vec<String>> {
        let out = self
            .read(&[
                "evolog",
                "--no-graph",
                "-r",
                commit,
                "-T",
                r#"commit.commit_id() ++ "\n""#,
            ])
            .await?;
        Ok(out.lines().map(str::to_string).collect())
    }

    /// `jj log` as the user sees it: their default revset and log template, with the graph
    /// and colors. Lines that start a commit carry its ids.
    pub async fn log_graph(&self) -> anyhow::Result<Vec<LogLine>> {
        let template = self
            .read(&["config", "get", "templates.log"])
            .await
            .map(|t| t.trim().to_string())
            .unwrap_or_else(|_| "builtin_log_compact".into());
        let template = format!(
            r#""\x1f" ++ stringify(commit_id) ++ "\x1f" ++ stringify(change_id.shortest(8)) ++ "\x1f" ++ ({template})"#
        );
        let out = self
            .exec_with(&["log", "-T", &template], true, "always")
            .await?;
        Ok(parse_log(&String::from_utf8_lossy(&out)))
    }

    /// File contents at `commit`, or empty for the root commit.
    pub async fn file(&self, commit: Option<&str>, path: &str) -> anyhow::Result<Vec<u8>> {
        let Some(commit) = commit else {
            return Ok(Vec::new());
        };
        self.exec(
            &["file", "show", "-r", commit, &fileset_literal(path)],
            true,
        )
        .await
    }
}

/// A fileset matching exactly `path`, relative to the repo root.
fn fileset_literal(path: &str) -> String {
    format!(
        "root-file:\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

/// Joins template metadata with `--git` sections by their `diff --git` header.
fn join(meta: Vec<FileMeta>, git: Vec<GitFileDiff>) -> anyhow::Result<Vec<FileDiff>> {
    let mut git: std::collections::HashMap<String, GitFileDiff> =
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

    #[test]
    fn parses_change_lines_with_tabs_in_descriptions() {
        let got = parse_changes("abc\tc1\t2\t1\tfix: a\tb\nxyz\tc2\t1\t0\t\n");
        assert_eq!(got[0].description, "fix: a\tb");
        assert_eq!((got[0].unique, got[0].working_copy), (2, true));
        assert_eq!(
            (got[1].change_id.as_str(), got[1].description.as_str()),
            ("xyz", "")
        );
    }

    #[test]
    fn log_lines_carry_ids_only_where_a_commit_starts() {
        let got = parse_log("@  \x1fc1\x1fab\x1fab desc\n│  body\n");
        assert_eq!(got[0].text, "@  ab desc");
        assert_eq!(got[0].commit_id.as_deref(), Some("c1"));
        assert_eq!(
            (got[1].text.as_str(), got[1].change_id.as_deref()),
            ("│  body", None)
        );
    }

    #[test]
    fn fileset_literal_escapes_quotes() {
        assert_eq!(fileset_literal(r#"a "b"\c"#), r#"root-file:"a \"b\"\\c""#);
    }
}
