use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use tauri_specta::Event as _;

use crate::events::{RepoChanged, WorkingCopyStale};
use crate::jj::Jj;
use crate::review::RepoCtx;
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

/// Focuses the window for the repo containing `path`, creating it if needed.
pub fn open(app: &AppHandle, path: &Path) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let root = find_root(&path).ok_or_else(|| format!("not a jj repo: {}", path.display()))?;
    let label = format!("repo-{}", key(&root));

    if let Some(window) = app.get_webview_window(&label) {
        window.unminimize().ok();
        return window.set_focus().map_err(|e| e.to_string());
    }

    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let store = Store::open(&data_dir.join("repos").join(key(&root)).join("lgtm.db"))
        .map_err(|e| format!("open mark store: {e:#}"))?;
    let ctx = RepoCtx::new(root.clone(), Jj::new(&root), store);
    let emit = {
        let (app, label) = (app.clone(), label.clone());
        move |e: watch::Event| {
            let sent = match e {
                watch::Event::RepoChanged => RepoChanged {}.emit_to(&app, &label),
                watch::Event::Stale(message) => WorkingCopyStale { message }.emit_to(&app, &label),
            };
            sent.ok();
        }
    };
    match watch::start(&root, ctx.jj.clone(), ctx.stale.clone(), emit) {
        Ok(w) => *ctx.watch.lock().unwrap() = Some(w),
        Err(e) => eprintln!("lgtm: watch {}: {e:#}", root.display()),
    }
    app.state::<RepoWindows>()
        .0
        .lock()
        .unwrap()
        .insert(label.clone(), Arc::new(ctx));
    if let Err(e) = crate::recent::add(&crate::recent::path(&data_dir), &root) {
        eprintln!("lgtm: save recent repos: {e}");
    }
    WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(name(&root))
        .inner_size(1400.0, 900.0)
        .build()
        .map_err(|e| e.to_string())?;

    if let Some(start) = app.get_webview_window(START_LABEL) {
        start.close().ok();
    }
    Ok(())
}

pub fn open_start(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(START_LABEL) {
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, START_LABEL, WebviewUrl::App("index.html".into()))
        .title("lgtm")
        .inner_size(480.0, 420.0)
        .build()
        .map(|_| ())
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
