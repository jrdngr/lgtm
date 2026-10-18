//! The only code that runs jj. Reads skip the working-copy snapshot; writes take the
//! workspace's writer lock so two writing commands never overlap.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, bail};
use tokio::process::Command;
use tokio::sync::Mutex;

mod diff;
mod edit;
mod follow;
pub mod git;
pub mod init;
mod log;
mod near;
mod rewrite;
#[cfg(test)]
pub(crate) mod test_support;
pub mod workspace;
pub use diff::{FileDiff, Range};
pub use edit::JjAction;
pub use log::LogLine;
pub use near::{NearBookmark, StackSide};
pub use workspace::WorkspaceInfo;

/// True when `err` came from jj refusing to work on a stale working copy.
fn is_stale(err: &anyhow::Error) -> bool {
    format!("{err:#}").contains("working copy is stale")
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
    /// Whether it has unresolved conflicts.
    pub conflict: bool,
    /// Whether it changes no files.
    pub empty: bool,
    /// Whether other visible commits share its change id.
    pub divergent: bool,
    /// Its local bookmarks, each ending in `*` when the remote lacks it or is behind.
    pub bookmarks: Vec<String>,
    pub description: String,
}

const CHANGE_TEMPLATE: &str = r#"change_id.short(12) ++ "\t" ++ commit_id ++ "\t"
  ++ change_id.shortest().prefix().len() ++ "\t" ++ if(current_working_copy, "1", "0") ++ "\t"
  ++ if(conflict, "1", "0") ++ "\t" ++ if(empty, "1", "0") ++ "\t" ++ if(divergent, "1", "0") ++ "\t"
  ++ local_bookmarks.map(|b| b.name() ++ if(b.synced()
       && remote_bookmarks.any(|r| r.name() == b.name() && r.remote() != "git"), "", "*")).join(" ")
  ++ "\t" ++ description.first_line() ++ "\n""#;

fn parse_changes(out: &str) -> Vec<Change> {
    out.lines()
        .filter_map(|l| {
            let mut parts = l.splitn(9, '\t');
            Some(Change {
                change_id: parts.next()?.to_string(),
                commit_id: parts.next()?.to_string(),
                unique: parts.next()?.parse().ok()?,
                working_copy: parts.next()? == "1",
                conflict: parts.next()? == "1",
                empty: parts.next()? == "1",
                divergent: parts.next()? == "1",
                bookmarks: parts
                    .next()?
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
                description: parts.next().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

pub struct Jj {
    root: PathBuf,
    /// Held while the app writes. Holds the working copy's change as of the app's last look,
    /// so a move the app didn't make can be told apart.
    writer: Mutex<Option<String>>,
}

impl Jj {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            writer: Mutex::new(None),
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
        // jj prints paths relative to its cwd, and they should be relative to the repo.
        cmd.current_dir(&self.root)
            .arg("-R")
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

    /// Reads file contents jj passes through as raw bytes, so a non-UTF-8 file shows U+FFFD
    /// instead of failing the read.
    async fn read_lossy(&self, args: &[&str]) -> anyhow::Result<String> {
        Ok(String::from_utf8_lossy(&self.exec(args, true).await?).into_owned())
    }

    /// Snapshots the working copy. Fails with an error that `is_stale` recognizes when the
    /// working copy is stale.
    pub async fn snapshot(&self) -> anyhow::Result<()> {
        let _guard = self.writer.lock().await;
        self.exec(&["util", "snapshot"], false).await.map(drop)
    }

    /// Snapshots the working copy. Returns jj's message when it's stale rather than failing.
    pub async fn snapshot_or_stale(&self) -> anyhow::Result<Option<String>> {
        match self.snapshot().await {
            Ok(()) => Ok(None),
            Err(e) if is_stale(&e) => Ok(Some(format!("{e:#}"))),
            Err(e) => Err(e),
        }
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

    /// File contents at `commit`, or `None` when the file isn't there or `commit` is the root.
    pub async fn file_if_any(
        &self,
        commit: Option<&str>,
        path: &str,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        if commit.is_none() {
            return Ok(None);
        }
        match self.file(commit, path).await {
            Ok(bytes) => Ok(Some(bytes)),
            // `file show` fails this way on a missing literal path.
            Err(e) if e.to_string().contains("No such path") => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// A fileset matching exactly `path`, relative to the repo root.
fn fileset_literal(path: &str) -> String {
    format!(
        "root-file:\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\\\"")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_change_lines_with_tabs_in_descriptions() {
        let got =
            parse_changes("abc\tc1\t2\t1\t0\t0\t1\ta b*\tfix: a\tb\nxyz\tc2\t1\t0\t1\t1\t0\t\t\n");
        assert_eq!(got[0].description, "fix: a\tb");
        assert_eq!((got[0].unique, got[0].working_copy), (2, true));
        assert!(!got[0].conflict && got[1].conflict);
        assert!(!got[0].empty && got[1].empty);
        assert!(got[0].divergent && !got[1].divergent);
        assert_eq!(got[0].bookmarks, ["a", "b*"]);
        assert!(got[1].bookmarks.is_empty());
        assert_eq!(
            (got[1].change_id.as_str(), got[1].description.as_str()),
            ("xyz", "")
        );
    }

    #[tokio::test]
    async fn file_if_any_reads_a_file_or_finds_none() {
        let (dir, j) = test_support::repo();
        test_support::write(dir.path(), "a.txt", "a\n");
        j.snapshot().await.unwrap();
        let at = j.commit_ids("@").await.unwrap();
        let at = Some(at[0].as_str());
        assert_eq!(
            j.file_if_any(at, "a.txt").await.unwrap(),
            Some(b"a\n".to_vec())
        );
        assert_eq!(j.file_if_any(at, "nope.txt").await.unwrap(), None);
        assert_eq!(j.file_if_any(None, "a.txt").await.unwrap(), None);
        assert!(j.file_if_any(Some("no_such_rev"), "a.txt").await.is_err());
    }

    #[test]
    fn fileset_literal_escapes_quotes() {
        assert_eq!(fileset_literal(r#"a "b"\c"#), r#"root-file:"a \"b\"\\c""#);
    }
}
