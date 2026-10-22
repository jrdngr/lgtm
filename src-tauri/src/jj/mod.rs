//! The only code that runs jj. Reads skip the working-copy snapshot; writes take the
//! workspace's writer lock so two writing commands never overlap.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};

use std::collections::HashMap;

use anyhow::{Context, bail};
use tokio::process::Command;
use tokio::sync::Mutex;

mod at;
mod background;
mod bookmarks;
mod push;
mod child;
mod diff;
mod edit;
mod follow;
pub mod git;
mod holders;
pub mod init;
mod log;
mod near;
mod rewrite;
#[cfg(test)]
pub(crate) mod test_support;
pub mod workspace;
pub use at::WorkingCopyDto;
pub use background::Snapshot;
pub use bookmarks::BookmarkSpot;
pub use child::end_all;
pub use diff::{FileDiff, Range};
pub use edit::JjAction;
pub use log::LogRow;
pub use near::{NearBookmark, StackSide};
pub use push::PushPlan;
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

/// What a workspace's jj is doing that the UI shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notice {
    /// A write has waited on a lock for a while: the processes that have it open, one per
    /// line. `None` once it stops waiting.
    Blocked(Option<String>),
    /// Background snapshots paused after giving up on a lock, or started again.
    Paused(bool),
}

pub type Notify = Arc<dyn Fn(Notice) + Send + Sync>;

pub struct Jj {
    root: PathBuf,
    /// Held while the app writes. Holds the working copy's change as of the app's last look,
    /// so a move the app didn't make can be told apart.
    writer: Mutex<Option<String>>,
    notify: OnceLock<Notify>,
    background: std::sync::Mutex<background::Strikes>,
}

/// Reads longer than this log at INFO.
const SLOW_READ: std::time::Duration = std::time::Duration::from_secs(2);

/// Logs a jj command: changes at INFO, reads and snapshots at DEBUG, and slow reads at INFO.
/// A failure logs at the same level; the caller decides whether it's a problem.
fn log_run(
    root: &Path,
    args: &[&str],
    read_only: bool,
    took: std::time::Duration,
    err: Option<&anyhow::Error>,
) {
    let mut command = args.join(" ");
    if command.len() > 200 {
        let cut = command.floor_char_boundary(200);
        command.truncate(cut);
        command.push('…');
    }
    let ms = took.as_millis() as u64;
    let workspace = root.display();
    let quiet = read_only && took < SLOW_READ || args == ["util", "snapshot"];
    match (err, quiet) {
        (None, true) => tracing::debug!(%workspace, ms, "jj {command}"),
        (None, false) => tracing::info!(%workspace, ms, "jj {command}"),
        (Some(e), true) => tracing::debug!(%workspace, ms, "jj {command} failed: {e:#}"),
        (Some(e), false) => tracing::info!(%workspace, ms, "jj {command} failed: {e:#}"),
    }
}

impl Jj {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            writer: Mutex::new(None),
            notify: OnceLock::new(),
            background: Default::default(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Sets where notices go. Only the first call counts.
    pub fn set_notify(&self, notify: Notify) {
        self.notify.set(notify).ok();
    }

    fn notify(&self, notice: Notice) {
        if let Some(notify) = self.notify.get() {
            notify(notice);
        }
    }

    fn command(&self, args: &[&str], read_only: bool, color: &str) -> Command {
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
        cmd.args(args);
        cmd
    }

    /// The command's stdout, or its stderr as an error when it failed.
    fn check(&self, args: &[&str], out: child::Output) -> anyhow::Result<Vec<u8>> {
        if !out.status.success() {
            bail!("jj {}: {}", args.join(" "), out.stderr.trim());
        }
        Ok(out.stdout)
    }

    async fn exec(&self, args: &[&str], read_only: bool) -> anyhow::Result<Vec<u8>> {
        self.exec_with(args, read_only, "never").await
    }

    /// Reads take no locks, so they can be killed at any point. Writes are never ended early,
    /// and one that succeeds starts background snapshots again.
    async fn exec_with(
        &self,
        args: &[&str],
        read_only: bool,
        color: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let started = std::time::Instant::now();
        let out = self.exec_untimed(args, read_only, color).await;
        log_run(&self.root, args, read_only, started.elapsed(), out.as_ref().err());
        out
    }

    async fn exec_untimed(
        &self,
        args: &[&str],
        read_only: bool,
        color: &str,
    ) -> anyhow::Result<Vec<u8>> {
        let mut cmd = self.command(args, read_only, color);
        if read_only {
            let out = cmd
                .stdin(Stdio::null())
                .kill_on_drop(true)
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
            return Ok(out.stdout);
        }
        let report = |holders: Option<String>| self.notify(Notice::Blocked(holders));
        let out = child::run(cmd, child::Mode::Foreground(&report)).await?;
        let stdout = self.check(args, out)?;
        self.resume();
        Ok(stdout)
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

    /// The repo-relative paths of the files in `rev`, as of the last snapshot.
    pub async fn files(&self, rev: &str) -> anyhow::Result<Vec<String>> {
        let out = self
            .read_lossy(&["file", "list", "-r", rev, "-T", r#"path ++ "\n""#])
            .await?;
        Ok(out.lines().map(str::to_string).collect())
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

    /// A content hash of each of `paths` at `commit`, in one read. Paths not there are left out.
    pub async fn file_hashes(
        &self,
        commit: &str,
        paths: &[String],
    ) -> anyhow::Result<HashMap<String, String>> {
        if paths.is_empty() {
            return Ok(HashMap::new());
        }
        let fileset = paths
            .iter()
            .map(|p| fileset_literal(p))
            .chain(["none()".to_string()])
            .collect::<Vec<_>>()
            .join(" | ");
        // Each file starts with a marker and its path, so one output splits back into files.
        let out = self
            .exec(
                &[
                    "file",
                    "show",
                    "-r",
                    commit,
                    "-T",
                    FILE_MARK_TEMPLATE,
                    &fileset,
                ],
                true,
            )
            .await?;
        Ok(split_files(&out)
            .into_iter()
            .map(|(path, bytes)| (path, blake3::hash(bytes).to_hex().to_string()))
            .collect())
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
        let Some(commit) = commit else {
            return Ok(None);
        };
        // A union makes a missing path print nothing rather than fail, and the template marks
        // the start of the file when it's there, so an empty file still shows.
        let fileset = format!("{} | none()", fileset_literal(path));
        let out = self
            .exec(
                &["file", "show", "-r", commit, "-T", r#""+""#, &fileset],
                true,
            )
            .await?;
        Ok(out.strip_prefix(b"+").map(<[u8]>::to_vec))
    }
}

const FILE_MARK: &[u8] = b"\0lgtm-file\0";
const FILE_MARK_TEMPLATE: &str = r#""\0lgtm-file\0" ++ path ++ "\0""#;

/// `jj file show` output under `FILE_MARK_TEMPLATE`, as each file's path and bytes.
fn split_files(out: &[u8]) -> Vec<(String, &[u8])> {
    let mut starts = Vec::new();
    let mut i = 0;
    while let Some(at) = out[i..]
        .windows(FILE_MARK.len())
        .position(|w| w == FILE_MARK)
    {
        starts.push(i + at);
        i += at + FILE_MARK.len();
    }
    starts
        .iter()
        .enumerate()
        .filter_map(|(n, &start)| {
            let body =
                &out[start + FILE_MARK.len()..starts.get(n + 1).copied().unwrap_or(out.len())];
            let end = body.iter().position(|&b| b == 0)?;
            Some((
                String::from_utf8_lossy(&body[..end]).into_owned(),
                &body[end + 1..],
            ))
        })
        .collect()
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
    fn splits_marked_file_output_into_paths_and_bytes() {
        let out = b"\0lgtm-file\0a.md\0one\n\0lgtm-file\0d/b.md\0\0lgtm-file\0c.md\0x";
        let got: Vec<(String, &[u8])> = split_files(out);
        assert_eq!(
            got,
            [
                ("a.md".to_string(), &b"one\n"[..]),
                ("d/b.md".to_string(), &b""[..]),
                ("c.md".to_string(), &b"x"[..]),
            ]
        );
    }

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
        test_support::write(dir.path(), "empty.txt", "");
        j.snapshot().await.unwrap();
        let at = j.commit_ids("@").await.unwrap();
        let at = Some(at[0].as_str());
        assert_eq!(
            j.file_if_any(at, "a.txt").await.unwrap(),
            Some(b"a\n".to_vec())
        );
        assert_eq!(j.file_if_any(at, "empty.txt").await.unwrap(), Some(vec![]));
        assert_eq!(j.file_if_any(at, "nope.txt").await.unwrap(), None);
        assert_eq!(j.file_if_any(None, "a.txt").await.unwrap(), None);
        assert!(j.file_if_any(Some("no_such_rev"), "a.txt").await.is_err());
    }

    #[test]
    fn fileset_literal_escapes_quotes() {
        assert_eq!(fileset_literal(r#"a "b"\c"#), r#"root-file:"a \"b\"\\c""#);
    }
}
