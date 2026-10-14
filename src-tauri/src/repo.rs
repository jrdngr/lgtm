use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, Wry};

use tauri_specta::Event as _;

use crate::agent::Runs;
use crate::events::{RepoChanged, WorkingCopyMoved, WorkingCopyStale};
use crate::jj::Jj;
use crate::review::{Emit, RepoCtx};
use crate::store::Store;
use crate::watch;

/// Repo state by window label. The start window has no entry.
#[derive(Default)]
pub struct RepoWindows {
    open: Mutex<HashMap<String, Arc<RepoCtx>>>,
    /// Closed repos whose agent runs were stopped, until their cleanup lets go of them.
    closing: Mutex<Vec<Weak<RepoCtx>>>,
}

impl RepoWindows {
    pub fn get(&self, label: &str) -> Option<Arc<RepoCtx>> {
        self.open.lock().unwrap().get(label).cloned()
    }

    /// Forgets the window's repo and stops its agent runs.
    pub fn remove(&self, label: &str, runs: &Runs) {
        let Some(ctx) = self.open.lock().unwrap().remove(label) else {
            return;
        };
        if runs.stop_all(&ctx.agent_key("")) {
            self.closing.lock().unwrap().push(Arc::downgrade(&ctx));
        }
    }

    /// Whether every stopped run has cleaned up after itself, like `jj` putting `@` back.
    pub fn settled(&self) -> bool {
        let mut closing = self.closing.lock().unwrap();
        closing.retain(|ctx| ctx.strong_count() > 0);
        closing.is_empty()
    }

    /// Waits until `settled`, for a few seconds at most.
    pub async fn settle(&self) {
        for _ in 0..100 {
            if self.settled() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
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

/// The main workspace root of the repo containing `path`: the nearest ancestor (inclusive)
/// holding a `.jj` directory or, when that's a secondary workspace, the main workspace its
/// `.jj/repo` file points to.
pub fn find_root(path: &Path) -> Option<PathBuf> {
    let root = path.ancestors().find(|p| p.join(".jj").is_dir())?;
    Some(main_root(root).unwrap_or_else(|| root.to_path_buf()))
}

/// The main workspace root a secondary workspace at `root` belongs to, from its `.jj/repo` file.
/// `None` for a main workspace, or when that file's path doesn't resolve.
pub fn main_root(root: &Path) -> Option<PathBuf> {
    let link = std::fs::read_to_string(root.join(".jj/repo")).ok()?;
    let store = root.join(".jj").join(link.trim()).canonicalize().ok()?;
    Some(store.parent()?.parent()?.to_path_buf())
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
            watch::Event::WorkingCopyMoved => WorkingCopyMoved { workspace }.emit_to(&app, &label),
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
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(name(&root))
        .inner_size(1400.0, 900.0)
        // Tauri's file-drop handler swallows HTML5 drags, which dockview uses to move panes.
        .disable_drag_drop_handler();
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

pub fn open_start(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(START_LABEL) {
        return window.set_focus();
    }
    let builder = WebviewWindowBuilder::new(app, START_LABEL, WebviewUrl::App("index.html".into()))
        .title("lgtm")
        .inner_size(480.0, 420.0);
    crate::placement::place(app, START_LABEL, builder)
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
    fn find_root_resolves_a_secondary_workspace_to_its_main_repo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (main, ws) = (root.join("main"), root.join("ws"));
        std::fs::create_dir_all(main.join(".jj/repo")).unwrap();
        std::fs::create_dir_all(ws.join(".jj")).unwrap();
        std::fs::create_dir_all(ws.join("src")).unwrap();
        std::fs::write(ws.join(".jj/repo"), "../../main/.jj/repo").unwrap();
        assert_eq!(find_root(&ws.join("src")), Some(main));
    }

    #[test]
    fn find_root_ignores_jj_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".jj"), "").unwrap();
        assert_eq!(find_root(dir.path()), None);
    }

    #[tokio::test]
    async fn closing_a_window_stops_only_its_runs_and_waits_for_their_cleanup() {
        let (dir, ctx) = crate::review::tests::repo();
        let (_other_dir, other) = crate::review::tests::repo();
        let (ctx, other) = (Arc::new(ctx), Arc::new(other));
        let windows = RepoWindows::default();
        windows.insert("a".into(), ctx.clone());
        windows.insert("b".into(), other.clone());
        let runs = Runs::default();
        let run = runs.register(&ctx.agent_key("fix")).unwrap();
        let _other_run = runs.register(&other.agent_key("fix")).unwrap();

        windows.remove("a", &runs);
        let task = crate::agent::Task {
            cwd: dir.path(),
            prompt: "",
            resume: None,
            edit: false,
            schema: None,
            purpose: crate::agent::Purpose::Ask,
        };
        let reply = crate::agent::run_registered(run, task, |_| {}).await;
        assert!(reply.unwrap().stopped);
        assert!(
            runs.register(&other.agent_key("fix")).is_err(),
            "b's run goes on"
        );
        // The run's command still holds the repo while it cleans up.
        assert!(!windows.settled());
        drop(ctx);
        assert!(windows.settled());
    }
}
