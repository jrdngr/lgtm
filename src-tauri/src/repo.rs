use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};

use tauri_specta::Event as _;

use crate::ctx::{Emit, RepoCtx};
use crate::events::{
    JjBlocked, JjPaused, RepoChanged, ReviewOutdated, WorkingCopyMoved, WorkingCopyStale,
};
use crate::jj::{Jj, Notice};
use crate::paths::{data_dir, find_root, key, repo_dir};
use crate::store::Store;
use crate::watch;

/// Each open repo's state by its label, and the label of the window showing it.
#[derive(Default)]
pub struct Repos {
    open: Mutex<HashMap<String, (Arc<RepoCtx>, String)>>,
}

impl Repos {
    pub fn get(&self, label: &str) -> Option<Arc<RepoCtx>> {
        self.open
            .lock()
            .unwrap()
            .get(label)
            .map(|(ctx, _)| ctx.clone())
    }

    /// Forgets the repo and stops its language servers.
    pub fn remove(&self, label: &str) {
        let Some((ctx, _)) = self.open.lock().unwrap().remove(label) else {
            return;
        };
        tauri::async_runtime::spawn(async move { ctx.stop_servers().await });
    }

    /// The label and state of the repo whose main workspace is at `root`.
    pub fn find(&self, root: &Path) -> Option<(String, Arc<RepoCtx>)> {
        let map = self.open.lock().unwrap();
        map.iter()
            .find(|(_, (ctx, _))| ctx.root == root)
            .map(|(label, (ctx, _))| (label.clone(), ctx.clone()))
    }

    /// The window showing the repo with `label`.
    pub fn window_of(&self, label: &str) -> Option<String> {
        self.open.lock().unwrap().get(label).map(|(_, w)| w.clone())
    }

    /// The labels of the repos window `window` shows.
    pub fn in_window(&self, window: &str) -> Vec<String> {
        let map = self.open.lock().unwrap();
        map.iter()
            .filter(|(_, (_, w))| w == window)
            .map(|(label, _)| label.clone())
            .collect()
    }

    fn insert(&self, label: String, window: &str, ctx: Arc<RepoCtx>) {
        self.open
            .lock()
            .unwrap()
            .insert(label, (ctx, window.to_string()));
    }
}

#[derive(Serialize, Type)]
pub struct RepoInfo {
    pub root: String,
    pub name: String,
}

impl RepoInfo {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.display().to_string(),
            name: name(root),
        }
    }
}

pub(crate) fn name(root: &Path) -> String {
    root.file_name().map_or_else(
        || root.display().to_string(),
        |n| n.to_string_lossy().into(),
    )
}

/// Sends a workspace's watch events to the repo's window.
fn emitter(app: &AppHandle, label: &str) -> Emit {
    let (app, label) = (app.clone(), label.to_string());
    Arc::new(move |workspace: Option<String>, e: watch::Event| {
        let sent = match e {
            watch::Event::RepoChanged { snapshot } => {
                RepoChanged { snapshot }.emit_to(&app, &label)
            }
            watch::Event::Stale(message) => {
                WorkingCopyStale { workspace, message }.emit_to(&app, &label)
            }
            watch::Event::WorkingCopyMoved => WorkingCopyMoved { workspace }.emit_to(&app, &label),
            watch::Event::Jj(Notice::Blocked(holders)) => {
                JjBlocked { workspace, holders }.emit_to(&app, &label)
            }
            watch::Event::Jj(Notice::Paused(paused)) => {
                JjPaused { workspace, paused }.emit_to(&app, &label)
            }
            watch::Event::ReviewOutdated { review } => {
                ReviewOutdated { review }.emit_to(&app, &label)
            }
        };
        sent.ok();
    })
}

/// The label of the repo at `root`, which keys its state, terminals and events.
pub fn label_of(root: &Path) -> String {
    format!("repo-{}", key(root))
}

/// The root of the jj repo holding `path`.
pub fn root_of(path: &Path) -> Result<PathBuf, String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    find_root(&path).ok_or_else(|| format!("not a jj repo: {}", path.display()))
}

/// Starts keeping the state of the repo at `root` for window `window`, unless it's open
/// already. Returns its label.
pub fn start(app: &AppHandle, root: &Path, window: &str) -> Result<String, String> {
    let label = label_of(root);
    let repos = app.state::<Repos>();
    if repos.get(&label).is_some() {
        return Ok(label);
    }
    let data_dir = data_dir(app)?;
    let store = Store::open(&repo_dir(&data_dir, root).join("lgtm.db"))
        .map_err(|e| format!("open mark store: {e:#}"))?;
    let ctx = RepoCtx::new(root.to_path_buf(), Jj::new(root), store);
    *ctx.emit.lock().unwrap() = Some(emitter(app, &label));
    *ctx.ui.lock().unwrap() = Some(crate::ui::Ui {
        app: app.clone(),
        label: label.clone(),
    });
    ctx.hook(&ctx.main);
    // The main workspace's watcher also reports operations made in the other workspaces.
    ctx.watch(&ctx.main);
    let ctx = Arc::new(ctx);
    // Lists the other workspaces without holding up the window.
    tauri::async_runtime::spawn({
        let ctx = ctx.clone();
        async move {
            ctx.list_workspaces()
                .await
                .map(drop)
                .unwrap_or_else(|e| tracing::error!("{e:#}"))
        }
    });
    poll_store(&ctx);
    repos.insert(label.clone(), window, ctx);
    tracing::info!(repo = %root.display(), "opened repo");
    if let Err(e) = crate::recent::add(&crate::recent::path(&data_dir), root) {
        tracing::warn!("save recent repos: {e}");
    }
    Ok(label)
}

/// Rereads open reviews whenever another program, like the terminal app, writes to the
/// repo's store. Stops once the repo closes.
fn poll_store(ctx: &Arc<RepoCtx>) {
    let weak = Arc::downgrade(ctx);
    tauri::async_runtime::spawn(async move {
        let mut seen = None;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let Some(ctx) = weak.upgrade() else { return };
            let version = ctx.store.lock().unwrap().data_version().ok();
            if seen.is_some() && version != seen {
                ctx.reread_store();
                ctx.reread_turns();
            }
            seen = version;
        }
    });
}
