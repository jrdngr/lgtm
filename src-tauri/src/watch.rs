//! Watches a workspace. Working-copy edits trigger a debounced `jj util snapshot`; new
//! operations (`.jj/repo/op_heads/heads/`) are reported as repo changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ignore::gitignore::Gitignore;
use notify::{RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};

use crate::jj::{Jj, Notice, Snapshot};

const SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(300);
/// The longest a run of edits holds off a snapshot.
const SNAPSHOT_MAX_WAIT: Duration = Duration::from_secs(2);
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A jj operation landed: the repo's commits may have changed.
    RepoChanged,
    /// The working copy became stale (with jj's message) or stopped being stale.
    Stale(Option<String>),
    /// Something other than the app moved `@`.
    WorkingCopyMoved,
    /// What the workspace's jj is doing.
    Jj(Notice),
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

#[derive(Debug, PartialEq, Eq)]
enum Ping {
    Op,
    Edit,
}

/// Whether a path is ignored by the `.gitignore` files above it. Matchers load lazily.
struct Ignores {
    root: PathBuf,
    by_dir: HashMap<PathBuf, Option<Gitignore>>,
}

impl Ignores {
    fn ignored(&mut self, path: &Path) -> bool {
        let is_dir = path.is_dir();
        let Ok(rel) = path.strip_prefix(&self.root) else {
            return true;
        };
        let mut dir = self.root.clone();
        for part in rel.parent().into_iter().flat_map(Path::components) {
            if self.check(&dir, path, is_dir) {
                return true;
            }
            dir.push(part);
        }
        self.check(&dir, path, is_dir)
    }

    fn check(&mut self, dir: &Path, path: &Path, is_dir: bool) -> bool {
        let gi = self.by_dir.entry(dir.to_path_buf()).or_insert_with(|| {
            let file = dir.join(".gitignore");
            file.is_file().then(|| Gitignore::new(file).0)
        });
        gi.as_ref()
            .is_some_and(|gi| gi.matched_path_or_any_parents(path, is_dir).is_ignore())
    }
}

fn classify(root: &Path, ignores: &mut Ignores, path: &Path) -> Option<Ping> {
    let rel = path.strip_prefix(root).ok()?;
    if rel.starts_with(".jj/repo/op_heads/heads") {
        return Some(Ping::Op);
    }
    // Its own and any nested workspace's or repo's internals.
    if rel.components().any(|c| c.as_os_str() == ".jj" || c.as_os_str() == ".git") {
        return None;
    }
    if rel.file_name().is_some_and(|n| n == ".gitignore") {
        ignores.by_dir.clear();
        return Some(Ping::Edit);
    }
    (!ignores.ignored(path)).then_some(Ping::Edit)
}

/// A secondary workspace's `.jj/repo` is a file pointing at the main repo, whose operations
/// land outside the workspace. Returns that repo's op heads directory, to watch as well.
fn shared_op_heads(root: &Path) -> Option<PathBuf> {
    let link = std::fs::read_to_string(root.join(".jj/repo")).ok()?;
    let repo = root.join(".jj").join(link.trim()).canonicalize().ok()?;
    Some(repo.join("op_heads/heads"))
}

/// Starts watching `root` (canonical). `emit` runs on the async runtime.
pub fn start(
    root: &Path,
    jj: Arc<Jj>,
    stale: Arc<Mutex<Option<String>>>,
    emit: impl Fn(Event) + Send + 'static,
) -> anyhow::Result<Watch> {
    let (tx, rx) = mpsc::unbounded_channel();
    let mut ignores = Ignores {
        root: root.to_path_buf(),
        by_dir: HashMap::new(),
    };
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
    let task = tauri::async_runtime::spawn(run(rx, jj, stale, emit));
    Ok(Watch {
        _watcher: watcher,
        task,
    })
}

async fn run(
    mut rx: mpsc::UnboundedReceiver<Ping>,
    jj: Arc<Jj>,
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
    loop {
        let next = snapshot_at.into_iter().chain(changed_at).min();
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
                Some(Ping::Edit) => {
                    tracing::trace!(workspace = jj.name(), "edit");
                    let now = Instant::now();
                    let first = *edits_since.get_or_insert(now);
                    let at = (now + SNAPSHOT_DEBOUNCE).min(first + SNAPSHOT_MAX_WAIT);
                    snapshot_at = Some(retry_at.map_or(at, |r| at.max(r)));
                }
            },
            _ = sleep_until(next.unwrap_or_else(Instant::now)), if next.is_some() => {
                let now = Instant::now();
                if changed_at.is_some_and(|t| t <= now) {
                    changed_at = None;
                    tracing::trace!(workspace = jj.name(), "repo changed");
                    emit(Event::RepoChanged);
                    match jj.moved_working_copy().await {
                        Ok(true) => emit(Event::WorkingCopyMoved),
                        Ok(false) => {}
                        Err(e) => tracing::warn!(workspace = jj.name(), "read @: {e:#}"),
                    }
                }
                if snapshot_at.is_some_and(|t| t <= now) {
                    snapshot_at = None;
                    edits_since = None;
                    let now_stale = match jj.background_snapshot().await {
                        Ok(Snapshot::Done(now_stale)) => {
                            retry_at = None;
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

    #[test]
    fn classifies_ops_edits_and_ignored_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::write(root.join(".gitignore"), "/target\n*.log\n").unwrap();
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/.gitignore"), "dist/\n").unwrap();
        std::fs::create_dir_all(root.join("web/dist")).unwrap();
        let mut ignores = Ignores {
            root: root.clone(),
            by_dir: HashMap::new(),
        };
        let mut c = |p: &str| classify(&root, &mut ignores, &root.join(p));
        assert_eq!(c(".jj/repo/op_heads/heads/abc"), Some(Ping::Op));
        assert_eq!(c(".jj/repo/store/x"), None);
        assert_eq!(c(".git/index"), None);
        assert_eq!(c(".workspaces/ws/.jj/working_copy/checkout"), None);
        assert_eq!(c("vendor/lib/.git/HEAD"), None);
        assert_eq!(c("src/main.rs"), Some(Ping::Edit));
        assert_eq!(c("target/debug/x"), None);
        assert_eq!(c("a/b.log"), None);
        assert_eq!(c("web/dist/app.js"), None);
        assert_eq!(c("web/src/app.ts"), Some(Ping::Edit));
        assert_eq!(c(".gitignore"), Some(Ping::Edit));
    }

    #[test]
    fn a_secondary_workspace_also_watches_the_main_repos_op_heads() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (main, ws) = (root.join("main"), root.join("ws"));
        std::fs::create_dir_all(main.join(".jj/repo/op_heads/heads")).unwrap();
        std::fs::create_dir_all(ws.join(".jj")).unwrap();
        std::fs::write(ws.join(".jj/repo"), "../../main/.jj/repo").unwrap();
        assert_eq!(
            shared_op_heads(&ws),
            Some(main.join(".jj/repo/op_heads/heads"))
        );
        assert_eq!(shared_op_heads(&main), None);
    }

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
        let got = tokio::time::timeout(Duration::from_secs(10), rx.recv()).await;
        assert_eq!(got.ok().flatten(), Some(Event::RepoChanged));
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
