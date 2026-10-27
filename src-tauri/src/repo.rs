use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

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
    fn remove(&self, label: &str) {
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

pub const START_LABEL: &str = "start";

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
    repos.insert(label.clone(), window, ctx);
    tracing::info!(repo = %root.display(), "opened repo");
    if let Err(e) = crate::recent::add(&crate::recent::path(&data_dir), root) {
        tracing::warn!("save recent repos: {e}");
    }
    Ok(label)
}

/// Ends the repo's terminals and HTML tabs, and forgets its state.
pub fn stop(app: &AppHandle, label: &str) {
    app.state::<crate::pty::Ptys>().kill_window(label);
    app.state::<crate::html::HtmlRoots>().close_window(label);
    app.state::<Repos>().remove(label);
}

/// Opens the repo holding `path`: focuses the window showing it, or else opens the project
/// that has it, or a new project of its own.
pub fn open(app: &AppHandle, path: &Path) -> Result<(), String> {
    crate::project::open_repo(app, &root_of(path)?)
}

/// Builds a window showing the app at `label`.
pub fn window(app: &AppHandle, label: &str, title: &str) -> Result<(), String> {
    let builder = WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(1400.0, 900.0)
        // Tauri's file-drop handler swallows HTML5 drags, which dockview uses to move panes.
        .disable_drag_drop_handler()
        .on_navigation(stays_in_app);
    crate::zoom::on_load(crate::placement::place(app, label, builder))
        .build()
        .map_err(|e| e.to_string())?;
    close_start(app);
    Ok(())
}

pub fn close_start(app: &AppHandle) {
    if let Some(start) = app.get_webview_window(START_LABEL) {
        start.close().ok();
    }
}

/// Whether the webview or a frame in it may go to `url`: only the app's own pages and HTML
/// tabs' files, so a dropped file or a link never replaces the app and an HTML tab's page
/// never leaves for the web.
fn stays_in_app(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" | crate::html::SCHEME => true,
        "http" | "https" => matches!(
            url.host_str(),
            Some("localhost" | "tauri.localhost" | "127.0.0.1")
        ),
        _ => false,
    }
}

pub fn open_start(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(START_LABEL) {
        return window.set_focus();
    }
    let builder = WebviewWindowBuilder::new(app, START_LABEL, WebviewUrl::App("index.html".into()))
        .title("lgtm")
        .inner_size(480.0, 420.0)
        .on_navigation(stays_in_app);
    crate::zoom::on_load(crate::placement::place(app, START_LABEL, builder))
        .build()
        .map(drop)
}

/// Opens the repo named on a command line. With none, a `launch` reopens the projects open at
/// the last quit, and otherwise, or when that fails, the start window opens.
pub fn open_from_args(app: &AppHandle, args: &[String], cwd: &Path, launch: bool) {
    let Some(arg) = args.iter().find(|a| !a.starts_with('-')) else {
        if !launch || crate::project::reopen(app).is_err() {
            open_start(app).ok();
        }
        return;
    };
    if let Err(e) = open(app, &cwd.join(arg)) {
        tracing::error!("{e}");
        open_start(app).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_webview_stays_on_the_apps_own_pages() {
        let ok = |u: &str| stays_in_app(&tauri::Url::parse(u).unwrap());
        assert!(ok("tauri://localhost/index.html"));
        assert!(ok("http://localhost:1420/"));
        assert!(ok("lgtmhtml://localhost/r0/docs/map.html"));
        assert!(!ok("file:///Users/j/diagram.png"));
        assert!(!ok("https://github.com/o/r"));
    }
}
