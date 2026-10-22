use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use tauri_specta::Event as _;

use crate::ctx::{Emit, RepoCtx};
use crate::events::{JjBlocked, JjPaused, RepoChanged, WorkingCopyMoved, WorkingCopyStale};
use crate::jj::{Jj, Notice};
use crate::paths::{data_dir, find_root, key, repo_dir};
use crate::store::Store;
use crate::watch;

/// Repo state by window label. The start window has no entry.
#[derive(Default)]
pub struct RepoWindows {
    open: Mutex<HashMap<String, Arc<RepoCtx>>>,
}

impl RepoWindows {
    pub fn get(&self, label: &str) -> Option<Arc<RepoCtx>> {
        self.open.lock().unwrap().get(label).cloned()
    }

    /// Forgets the window's repo.
    pub fn remove(&self, label: &str) {
        self.open.lock().unwrap().remove(label);
    }

    /// The window and state of the repo whose main workspace is at `root`.
    pub fn find(&self, root: &Path) -> Option<(String, Arc<RepoCtx>)> {
        let map = self.open.lock().unwrap();
        map.iter()
            .find(|(_, ctx)| ctx.root == root)
            .map(|(label, ctx)| (label.clone(), ctx.clone()))
    }

    fn insert(&self, label: String, ctx: Arc<RepoCtx>) {
        self.open.lock().unwrap().insert(label, ctx);
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

fn name(root: &Path) -> String {
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
            watch::Event::RepoChanged => RepoChanged {}.emit_to(&app, &label),
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
        };
        sent.ok();
    })
}

/// Focuses the window for the repo containing `path`, creating it if needed.
pub fn open(app: &AppHandle, path: &Path) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let root = find_root(&path).ok_or_else(|| format!("not a jj repo: {}", path.display()))?;
    let label = format!("repo-{}", key(&root));

    if let Some(window) = app.get_webview_window(&label) {
        close_start(app);
        window.unminimize().ok();
        return window.set_focus().map_err(|e| e.to_string());
    }

    let data_dir = data_dir(app)?;
    let store = Store::open(&repo_dir(&data_dir, &root).join("lgtm.db"))
        .map_err(|e| format!("open mark store: {e:#}"))?;
    let ctx = RepoCtx::new(root.clone(), Jj::new(&root), store);
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
    app.state::<RepoWindows>().insert(label.clone(), ctx);
    if let Err(e) = crate::recent::add(&crate::recent::path(&data_dir), &root) {
        tracing::warn!("save recent repos: {e}");
    }
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(name(&root))
        .inner_size(1400.0, 900.0)
        // Tauri's file-drop handler swallows HTML5 drags, which dockview uses to move panes.
        .disable_drag_drop_handler()
        .on_navigation(stays_in_app);
    let window = crate::placement::place(app, &label, builder)
        .build()
        .map_err(|e| e.to_string())?;
    crate::zoom::apply(&window);
    close_start(app);
    Ok(())
}

fn close_start(app: &AppHandle) {
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
    crate::placement::place(app, START_LABEL, builder)
        .build()
        .map(|window| crate::zoom::apply(&window))
}

/// Opens the repo named on a command line. With none, a `launch` reopens the last repo, and
/// otherwise, or when that fails, the start window opens.
pub fn open_from_args(app: &AppHandle, args: &[String], cwd: &Path, launch: bool) {
    let Some(arg) = args.iter().find(|a| !a.starts_with('-')) else {
        if !launch || open_last(app).is_err() {
            open_start(app).ok();
        }
        return;
    };
    if let Err(e) = open(app, &cwd.join(arg)) {
        tracing::error!("{e}");
        open_start(app).ok();
    }
}

/// Opens the most recently opened repo that's still a jj repo.
fn open_last(app: &AppHandle) -> Result<(), String> {
    let last = crate::recent::load(&crate::recent::path(&data_dir(app)?))
        .into_iter()
        .next()
        .ok_or("no recent repo")?;
    open(app, &last).inspect_err(|e| tracing::error!("reopen {}: {e}", last.display()))
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
