use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry};

use tauri_specta::Event as _;

use crate::events::{RepoChanged, WorkingCopyStale};
use crate::jj::Jj;
use crate::review::{Emit, RepoCtx};
use crate::store::Store;
use crate::watch;

/// Repo state by window label. The start window has no entry.
#[derive(Default)]
pub struct RepoWindows(Mutex<HashMap<String, Arc<RepoCtx>>>);

impl RepoWindows {
    pub fn get(&self, label: &str) -> Option<Arc<RepoCtx>> {
        self.0.lock().unwrap().get(label).cloned()
    }

    pub fn remove(&self, label: &str) {
        self.0.lock().unwrap().remove(label);
    }

    /// The window and state of the repo whose main workspace is at `root`.
    pub fn find(&self, root: &Path) -> Option<(String, Arc<RepoCtx>)> {
        let map = self.0.lock().unwrap();
        map.iter()
            .find(|(_, ctx)| ctx.root == root)
            .map(|(label, ctx)| (label.clone(), ctx.clone()))
    }

    fn insert(&self, label: String, ctx: Arc<RepoCtx>) {
        self.0.lock().unwrap().insert(label, ctx);
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

/// Nearest ancestor of `path` (inclusive) holding a `.jj` directory.
pub fn find_root(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| p.join(".jj").is_dir())
        .map(Path::to_path_buf)
}

/// Stable per-repo key, used for window labels and storage directories.
pub fn key(root: &Path) -> String {
    blake3::hash(root.as_os_str().as_encoded_bytes()).to_hex()[..16].to_string()
}

pub fn data_dir(app: &impl Manager<Wry>) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// Where the repo's mark store and layouts live.
pub fn repo_dir(data_dir: &Path, root: &Path) -> PathBuf {
    data_dir.join("repos").join(key(root))
}

/// Sends a workspace's watch events to the repo's window.
fn emitter(app: &AppHandle, label: &str) -> Emit {
    let (app, label) = (app.clone(), label.to_string());
    Arc::new(move |workspace: Option<String>, e: watch::Event| {
        let sent = match e {
            watch::Event::RepoChanged => RepoChanged {}.emit_to(&app, &label),
            watch::Event::Stale(message) => {
                WorkingCopyStale { workspace, message }.emit_to(&app, &label)
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
    ctx.watch(&ctx.main);
    let ctx = Arc::new(ctx);
    // Watch the other workspaces too, without holding up the window.
    tauri::async_runtime::spawn({
        let ctx = ctx.clone();
        async move {
            ctx.list_workspaces()
                .await
                .map(drop)
                .unwrap_or_else(|e| eprintln!("lgtm: {e:#}"))
        }
    });
    app.state::<RepoWindows>().insert(label.clone(), ctx);
    if let Err(e) = crate::recent::add(&crate::recent::path(&data_dir), &root) {
        eprintln!("lgtm: save recent repos: {e}");
    }
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(name(&root))
        .inner_size(1400.0, 900.0)
        // Tauri's file-drop handler swallows HTML5 drags, which dockview uses to move panes.
        .disable_drag_drop_handler()
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

pub fn open_start(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(START_LABEL) {
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, START_LABEL, WebviewUrl::App("index.html".into()))
        .title("lgtm")
        .inner_size(480.0, 420.0)
        .build()
        .map(|window| crate::zoom::apply(&window))
}

/// Opens the repo named on a command line, or the start window when none is given.
pub fn open_from_args(app: &AppHandle, args: &[String], cwd: &Path) {
    let Some(arg) = args.iter().find(|a| !a.starts_with('-')) else {
        open_start(app).ok();
        return;
    };
    if let Err(e) = open(app, &cwd.join(arg)) {
        eprintln!("lgtm: {e}");
        open_start(app).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_root_walks_up_to_nearest_jj_dir() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path();
        let inner = outer.join("vendor/inner");
        std::fs::create_dir_all(outer.join(".jj")).unwrap();
        std::fs::create_dir_all(inner.join(".jj")).unwrap();
        std::fs::create_dir_all(inner.join("src/deep")).unwrap();

        assert_eq!(find_root(&inner.join("src/deep")), Some(inner.clone()));
        assert_eq!(find_root(&outer.join("vendor")), Some(outer.to_path_buf()));
    }

    #[test]
    fn find_root_ignores_jj_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".jj"), "").unwrap();
        assert_eq!(find_root(dir.path()), None);
    }
}
