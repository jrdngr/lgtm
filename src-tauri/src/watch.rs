//! Watches a workspace. Working-copy edits trigger a debounced `jj util snapshot`; new
//! operations (`.jj/repo/op_heads/heads/`) are reported as repo changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use notify::{RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};

use crate::jj::checkout::{op_heads, repo_dir};
use crate::jj::{Jj, Notice, Snapshot};
use lgtm_core::watch::{Ignores, Ping, classify, shared_op_heads};

/// The operation heads each repo store had after the app's last background snapshot there,
/// shared since every workspace's watcher sees every operation.
static SNAPSHOT_HEADS: LazyLock<Mutex<HashMap<PathBuf, Vec<String>>>> =
    LazyLock::new(Mutex::default);

const SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(300);
/// The longest a run of edits holds off a snapshot.
const SNAPSHOT_MAX_WAIT: Duration = Duration::from_secs(2);
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(50);
/// How often written files are passed on, at most, so a burst of writes sends one list.
const WRITTEN_EVERY: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A jj operation landed: the repo's commits may have changed. `snapshot` when the
    /// operation is the app's own background snapshot, which only rewrites a working copy.
    RepoChanged { snapshot: bool },
    /// The working copy became stale (with jj's message) or stopped being stale.
    Stale(Option<String>),
    /// Something other than the app moved `@`.
    WorkingCopyMoved,
    /// What the workspace's jj is doing.
    Jj(Notice),
    /// Files of the working copy were written, by path from the root.
    Written(Vec<String>),
    /// An agent asked about review `review`, which shows older commits than its revset has
    /// now. Sent whether or not the workspace is on screen.
    ReviewOutdated { review: u32 },
}

/// Keeps watching until dropped.
pub struct Watch {
    _watcher: notify::RecommendedWatcher,
    task: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Records the heads after a background snapshot when the snapshot is the only operation
/// since `before`. Otherwise forgets them, so the operations count as real changes.
async fn note_snapshot(jj: &Jj, repo: &Path, before: Vec<String>) {
    let after = op_heads(repo);
    if after == before {
        return;
    }
    let only = match jj.last_operation().await {
        Ok((id, parents)) => after == [id] && parents == before,
        Err(_) => false,
    };
    let mut heads = SNAPSHOT_HEADS.lock().unwrap();
    if only {
        heads.insert(repo.to_path_buf(), after);
    } else {
        heads.remove(repo);
    }
}

/// Starts watching `root` (canonical). `emit` runs on the async runtime.
pub fn start(
    root: &Path,
    jj: Arc<Jj>,
    stale: Arc<Mutex<Option<String>>>,
    emit: impl Fn(Event) + Send + 'static,
) -> anyhow::Result<Watch> {
    let (tx, rx) = mpsc::unbounded_channel();
    let mut ignores = Ignores::new(root);
    let watch_root = root.to_path_buf();
    let op_heads = shared_op_heads(root);
    let shared = op_heads.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.kind.is_access() {
            return;
        }
        for path in &event.paths {
            if shared.as_ref().is_some_and(|d| path.starts_with(d)) {
                tx.send(Ping::Op).ok();
            } else if let Some(ping) = classify(&watch_root, &mut ignores, path) {
                tx.send(ping).ok();
            }
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    if let Some(dir) = &op_heads {
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
    }
    let repo = repo_dir(root);
    let task = tauri::async_runtime::spawn(run(rx, jj, repo, stale, emit));
    Ok(Watch {
        _watcher: watcher,
        task,
    })
}

async fn run(
    mut rx: mpsc::UnboundedReceiver<Ping>,
    jj: Arc<Jj>,
    repo: Option<PathBuf>,
    stale: Arc<Mutex<Option<String>>>,
    emit: impl Fn(Event),
) {
    // Where `@` starts, so the first move is caught.
    if let Err(e) = jj.moved_working_copy().await {
        tracing::warn!(workspace = jj.name(), "read @: {e:#}");
    }
    let mut snapshot_at: Option<Instant> = None;
    // When the edits the next snapshot waits on began.
    let mut edits_since: Option<Instant> = None;
    let mut changed_at: Option<Instant> = None;
    // No snapshot before this, after one gave up waiting on a lock.
    let mut retry_at: Option<Instant> = None;
    let mut written: std::collections::BTreeSet<String> = Default::default();
    let mut written_at: Option<Instant> = None;
    loop {
        let next = snapshot_at
            .into_iter()
            .chain(changed_at)
            .chain(written_at)
            .min();
        tokio::select! {
            ping = rx.recv() => match ping {
                None => return,
                // Only a stale working copy is snapshotted after an operation, to see whether
                // the operation fixed it. Becoming stale shows with the next edit.
                Some(Ping::Op) => {
                    changed_at.get_or_insert(Instant::now() + CHANGE_DEBOUNCE);
                    if stale.lock().unwrap().is_some() {
                        let at = Instant::now() + SNAPSHOT_DEBOUNCE;
                        snapshot_at.get_or_insert(retry_at.map_or(at, |r| at.max(r)));
                    }
                }
                Some(Ping::Edit(path)) => {
                    tracing::trace!(workspace = jj.name(), "edit");
                    written.insert(path);
                    written_at.get_or_insert(Instant::now() + WRITTEN_EVERY);
                    let now = Instant::now();
                    let first = *edits_since.get_or_insert(now);
                    let at = (now + SNAPSHOT_DEBOUNCE).min(first + SNAPSHOT_MAX_WAIT);
                    snapshot_at = Some(retry_at.map_or(at, |r| at.max(r)));
                }
            },
            _ = sleep_until(next.unwrap_or_else(Instant::now)), if next.is_some() => {
                let now = Instant::now();
                if written_at.is_some_and(|t| t <= now) {
                    written_at = None;
                    emit(Event::Written(std::mem::take(&mut written).into_iter().collect()));
                }
                if changed_at.is_some_and(|t| t <= now) {
                    changed_at = None;
                    let snapshot = repo.as_ref().is_some_and(|r| {
                        SNAPSHOT_HEADS.lock().unwrap().get(r) == Some(&op_heads(r))
                    });
                    tracing::trace!(workspace = jj.name(), snapshot, "repo changed");
                    emit(Event::RepoChanged { snapshot });
                    match jj.moved_working_copy().await {
                        Ok(true) => emit(Event::WorkingCopyMoved),
                        Ok(false) => {}
                        Err(e) => tracing::warn!(workspace = jj.name(), "read @: {e:#}"),
                    }
                }
                if snapshot_at.is_some_and(|t| t <= now) {
                    snapshot_at = None;
                    edits_since = None;
                    let before = repo.as_deref().map(op_heads);
                    let now_stale = match jj.background_snapshot().await {
                        Ok(Snapshot::Done(now_stale)) => {
                            retry_at = None;
                            if let (Some(r), Some(before)) = (&repo, before) {
                                note_snapshot(&jj, r, before).await;
                            }
                            now_stale
                        }
                        Ok(Snapshot::Retry(after)) => {
                            retry_at = Some(Instant::now() + after);
                            snapshot_at = retry_at;
                            continue;
                        }
                        Ok(Snapshot::Paused) => continue,
                        Err(e) => {
                            tracing::warn!(workspace = jj.name(), "snapshot: {e:#}");
                            continue;
                        }
                    };
                    let mut current = stale.lock().unwrap();
                    if current.is_some() != now_stale.is_some() {
                        *current = now_stale.clone();
                        drop(current);
                        emit(Event::Stale(now_stale));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_edit_is_snapshotted_and_reported_as_a_repo_change() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ok = std::process::Command::new("jj")
            .args(["git", "init"])
            .arg(&root)
            .output()
            .unwrap();
        assert!(ok.status.success());
        let (tx, mut rx) = mpsc::unbounded_channel();
        let jj = Arc::new(Jj::new(&root));
        let _watch = start(&root, jj, Arc::default(), move |e| {
            tx.send(e).ok();
        })
        .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        std::fs::write(root.join("a.txt"), "hi\n").unwrap();
        let mut got = vec![];
        let changed = Event::RepoChanged { snapshot: true };
        while !got.contains(&changed) {
            let next = tokio::time::timeout(Duration::from_secs(10), rx.recv()).await;
            got.push(next.ok().flatten().expect("no repo change"));
        }
        assert!(got.contains(&Event::Written(vec!["a.txt".into()])));
    }

    #[tokio::test]
    async fn dropping_a_watch_ends_its_snapshot_waiting_on_a_lock() {
        let (dir, jj) = crate::jj::test_support::repo();
        let root = dir.path().canonicalize().unwrap();
        let lock = crate::jj::test_support::git_lock(&root);
        let held = crate::jj::test_support::hold(&lock);
        let watch = start(&root, Arc::new(jj), Arc::default(), |_| {}).unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        std::fs::write(root.join("a.txt"), "hi\n").unwrap();
        let openers = async |n: usize| {
            while crate::jj::test_support::openers(&lock).len() != n {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        // The test's hold, plus the snapshot's wait.
        tokio::time::timeout(Duration::from_secs(10), openers(2))
            .await
            .unwrap();
        drop(watch);
        // Well before the snapshot would give up on its own, a second into its wait in tests.
        tokio::time::timeout(Duration::from_millis(500), openers(1))
            .await
            .unwrap();
        drop(held);
    }

    #[tokio::test]
    async fn a_move_made_in_a_terminal_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let jj_cli = |args: &[&str]| {
            let out = std::process::Command::new("jj")
                .args(args)
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        jj_cli(&["git", "init"]);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let jj = Arc::new(Jj::new(&root));
        let _watch = start(&root, jj, Arc::default(), move |e| {
            tx.send(e).ok();
        })
        .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        jj_cli(&["new"]);
        let moved = async {
            while let Some(e) = rx.recv().await {
                if e == Event::WorkingCopyMoved {
                    return true;
                }
            }
            false
        };
        let got = tokio::time::timeout(Duration::from_secs(10), moved).await;
        assert_eq!(got.ok(), Some(true));
    }
}
