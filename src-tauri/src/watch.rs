//! Watches a repo. Working-copy edits trigger a debounced `jj util snapshot`; new operations
//! (`.jj/repo/op_heads/heads/`) are reported as repo changes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ignore::gitignore::Gitignore;
use notify::{RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::time::{Instant, sleep_until};

use crate::jj::Jj;

const SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(300);
const CHANGE_DEBOUNCE: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A jj operation landed: the repo's commits may have changed.
    RepoChanged,
    /// The working copy became stale (with jj's message) or stopped being stale.
    Stale(Option<String>),
}

/// Keeps watching until dropped.
pub struct Watch {
    _watcher: notify::RecommendedWatcher,
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
    if rel.starts_with(".jj") || rel.starts_with(".git") {
        return None;
    }
    if rel.file_name().is_some_and(|n| n == ".gitignore") {
        ignores.by_dir.clear();
        return Some(Ping::Edit);
    }
    (!ignores.ignored(path)).then_some(Ping::Edit)
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
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.kind.is_access() {
            return;
        }
        for path in &event.paths {
            if let Some(ping) = classify(&watch_root, &mut ignores, path) {
                tx.send(ping).ok();
            }
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    tauri::async_runtime::spawn(run(rx, jj, stale, emit));
    Ok(Watch { _watcher: watcher })
}

async fn run(
    mut rx: mpsc::UnboundedReceiver<Ping>,
    jj: Arc<Jj>,
    stale: Arc<Mutex<Option<String>>>,
    emit: impl Fn(Event),
) {
    let mut snapshot_at: Option<Instant> = None;
    let mut changed_at: Option<Instant> = None;
    loop {
        let next = snapshot_at.into_iter().chain(changed_at).min();
        tokio::select! {
            ping = rx.recv() => match ping {
                None => return,
                // An operation can make the working copy stale, so recheck after one too.
                Some(Ping::Op) => {
                    changed_at.get_or_insert(Instant::now() + CHANGE_DEBOUNCE);
                    snapshot_at.get_or_insert(Instant::now() + SNAPSHOT_DEBOUNCE);
                }
                Some(Ping::Edit) => snapshot_at = Some(Instant::now() + SNAPSHOT_DEBOUNCE),
            },
            _ = sleep_until(next.unwrap_or_else(Instant::now)), if next.is_some() => {
                let now = Instant::now();
                if changed_at.is_some_and(|t| t <= now) {
                    changed_at = None;
                    emit(Event::RepoChanged);
                }
                if snapshot_at.is_some_and(|t| t <= now) {
                    snapshot_at = None;
                    let now_stale = match jj.snapshot_or_stale().await {
                        Ok(now_stale) => now_stale,
                        Err(e) => {
                            eprintln!("lgtm: snapshot: {e:#}");
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
        assert_eq!(c("src/main.rs"), Some(Ping::Edit));
        assert_eq!(c("target/debug/x"), None);
        assert_eq!(c("a/b.log"), None);
        assert_eq!(c("web/dist/app.js"), None);
        assert_eq!(c("web/src/app.ts"), Some(Ping::Edit));
        assert_eq!(c(".gitignore"), Some(Ping::Edit));
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
}
