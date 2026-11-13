//! Watches the workspace: a new jj operation or git ref change reloads what's on screen, and
//! while the review shows the working copy, edits are snapshotted (in git, reloaded) so they
//! show up.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use lgtm_core::repo::Repo;
use lgtm_core::watch::{Ignores, Ping, classify, shared_op_heads};
use notify::{RecursiveMode, Watcher as _};
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::load::Msg;

/// Quiet time after an edit before snapshotting.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// The longest a run of edits holds off a snapshot.
const MAX_WAIT: Duration = Duration::from_secs(2);
/// Quiet time after an operation before reloading, so a burst reloads once.
const OP_DEBOUNCE: Duration = Duration::from_millis(50);
/// How often the files written are passed on, for the trees' heat.
const WRITTEN_EVERY: Duration = Duration::from_millis(250);

/// Must be called inside a tokio runtime.
pub fn start(
    root: &Path,
    repo: Arc<Repo>,
    tx: Sender<Msg>,
    snapshots: Arc<AtomicBool>,
) -> anyhow::Result<notify::RecommendedWatcher> {
    let (pings, mut rx) = mpsc::unbounded_channel();
    let mut ignores = Ignores::new(root);
    let watch_root = root.to_path_buf();
    let shared = shared_op_heads(root);
    let shared_dir = shared.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else { return };
        if event.kind.is_access() {
            return;
        }
        for path in &event.paths {
            let ping = if shared_dir.as_ref().is_some_and(|d| path.starts_with(d)) {
                Some(Ping::Op)
            } else {
                classify(&watch_root, &mut ignores, path)
            };
            if let Some(ping) = ping {
                pings.send(ping).ok();
            }
        }
    })?;
    watcher.watch(root, RecursiveMode::Recursive)?;
    if let Some(dir) = &shared {
        watcher.watch(dir, RecursiveMode::NonRecursive)?;
    }
    tokio::spawn(async move {
        // When the current run of edits started, and when it last grew.
        let mut edits: Option<(Instant, Instant)> = None;
        let mut op: Option<Instant> = None;
        // Files written since they were last passed on, and when that was.
        let mut written = std::collections::BTreeSet::new();
        let mut written_at = Instant::now();
        loop {
            let wait = if edits.is_some() || op.is_some() || !written.is_empty() {
                OP_DEBOUNCE
            } else {
                Duration::from_secs(3600)
            };
            match tokio::time::timeout(wait, rx.recv()).await {
                Ok(None) => return,
                Ok(Some(Ping::Op)) => op = Some(Instant::now()),
                Ok(Some(Ping::Edit(path))) => {
                    written.insert(path);
                    if snapshots.load(Ordering::Relaxed) {
                        let now = Instant::now();
                        edits = Some((edits.map_or(now, |(first, _)| first), now));
                    }
                }
                Err(_) => {}
            }
            let now = Instant::now();
            if !written.is_empty() && now - written_at >= WRITTEN_EVERY {
                written_at = now;
                let paths = std::mem::take(&mut written).into_iter().collect();
                if tx.send(Msg::Written(paths)).is_err() {
                    return;
                }
            }
            if let Some((first, last)) = edits
                && (now - last >= DEBOUNCE || now - first >= MAX_WAIT)
            {
                edits = None;
                // A jj snapshot that changed something lands as an operation. git's working
                // copy is read fresh, so its edits reload what's on screen themselves.
                repo.background_snapshot().await;
                if repo.is_git() {
                    op = Some(Instant::now());
                }
            }
            if op.is_some_and(|at| now - at >= OP_DEBOUNCE) {
                op = None;
                if tx.send(Msg::Changed).is_err() {
                    return;
                }
            }
        }
    });
    Ok(watcher)
}
