//! The only code that runs jj. Reads skip the working-copy snapshot; writes take the
//! workspace's writer lock so two writing commands never overlap.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, OnceLock};

use anyhow::{Context, bail};
use tokio::process::Command;
use tokio::sync::Mutex;

mod background;
mod bookmarks;
pub mod checkout;
mod child;
mod conflicts;
mod diff;
mod edit;
mod files;
pub mod git;
mod holders;
mod listing;
mod log;
mod push;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod workspace;
pub use bookmarks::{BookmarkSpot, Containing};
pub use diff::{DiffPath, FileDiff, Range};
pub use edit::JjAction;
pub use log::{LogCommit, LogRow};
pub use push::PushPlan;

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

/// A version of a change in its evolution log.
#[derive(Clone, Debug, PartialEq)]
pub struct Version {
    pub commit_id: String,
    /// When it was made, as hours and minutes.
    pub clock: String,
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
    /// Held while lgtm writes.
    writer: Mutex<()>,
    notify: OnceLock<Notify>,
    background: std::sync::Mutex<background::Strikes>,
    name: OnceLock<String>,
}

/// Reads longer than this log at INFO.
const SLOW_READ: std::time::Duration = std::time::Duration::from_secs(2);

/// Logs a jj command: changes at INFO, reads and snapshots at DEBUG, and slow reads at INFO.
/// A failure logs at the same level; the caller decides whether it's a problem.
fn log_run(
    workspace: &str,
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
            writer: Mutex::new(()),
            notify: OnceLock::new(),
            background: Default::default(),
            name: OnceLock::new(),
        }
    }

    /// The workspace's name, for logs. Its directory's name when the checkout file can't say.
    pub fn name(&self) -> &str {
        self.name.get_or_init(|| {
            checkout::name_at(&self.root).unwrap_or_else(|| {
                let dir = self.root.file_name().unwrap_or_default();
                dir.to_string_lossy().into_owned()
            })
        })
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
        log_run(
            self.name(),
            args,
            read_only,
            started.elapsed(),
            out.as_ref().err(),
        );
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
        let refused = push::refusals(args, &out.stderr);
        let stdout = self.check(args, out)?;
        self.resume();
        if let Some(why) = refused {
            bail!(why);
        }
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

    /// The commits `rev` evolved from, newest first, starting with `rev` itself, each with
    /// the time it was made.
    pub async fn versions(&self, rev: &str) -> anyhow::Result<Vec<Version>> {
        let out = self
            .read(&[
                "evolog",
                "--no-graph",
                "-r",
                rev,
                "-T",
                r#"commit.commit_id() ++ "\t" ++ commit.committer().timestamp().format("%H:%M") ++ "\n""#,
            ])
            .await?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let (id, clock) = l.split_once('\t')?;
                Some(Version {
                    commit_id: id.to_string(),
                    clock: clock.to_string(),
                })
            })
            .collect())
    }
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
}
