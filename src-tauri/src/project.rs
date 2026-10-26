//! Projects. A project window shows a named list of repos, each in a frame of its own, and a
//! repo is open in one window at a time. Projects are kept in the global store, so a window
//! reopens with its repos, and the projects open at quit reopen at launch.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event as _;

use crate::events::ShowRepo;
use crate::notes::GlobalStore;
use crate::repo::{self, Repos};
use crate::store::ProjectRow;

/// Project windows' labels start with this, then the project's id.
pub const PREFIX: &str = "project-";

/// Project windows by label: the project each shows, and the repo it has on screen.
#[derive(Default)]
pub struct Projects {
    open: Mutex<HashMap<String, Shown>>,
}

struct Shown {
    id: String,
    /// The label of the repo on screen.
    active: Option<String>,
}

impl Projects {
    /// The id of the project window `window` shows.
    pub fn id(&self, window: &str) -> Option<String> {
        self.open.lock().unwrap().get(window).map(|s| s.id.clone())
    }

    /// The label of the repo window `window` has on screen.
    pub fn active(&self, window: &str) -> Option<String> {
        self.open
            .lock()
            .unwrap()
            .get(window)
            .and_then(|s| s.active.clone())
    }

    pub fn set_active(&self, window: &str, label: Option<String>) {
        if let Some(s) = self.open.lock().unwrap().get_mut(window) {
            s.active = label;
        }
    }

    fn window_of(&self, id: &str) -> Option<String> {
        let open = self.open.lock().unwrap();
        open.iter()
            .find(|(_, s)| s.id == id)
            .map(|(window, _)| window.clone())
    }
}

/// One of a project's repos.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRepo {
    pub label: String,
    pub root: String,
    pub name: String,
    /// Put to sleep, or open in another window: nothing of it runs here.
    pub asleep: bool,
}

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDto {
    pub id: String,
    pub name: String,
    pub repos: Vec<ProjectRepo>,
    /// The window's own state for its repo list, as it saved it.
    pub ui: String,
    /// The label of the repo to show, when the window was opened for it.
    pub active: Option<String>,
}

/// A saved project, as the start window lists it.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    /// Its repos' names.
    pub repos: Vec<String>,
}

fn global(app: &AppHandle) -> std::sync::MutexGuard<'_, crate::store::Store> {
    app.state::<GlobalStore>().inner().0.lock().unwrap()
}

fn load(app: &AppHandle, id: &str) -> Result<ProjectRow, String> {
    global(app)
        .project(id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no project {id}"))
}

/// Changes the saved project `id` with `f`, and returns it as changed.
fn update(
    app: &AppHandle,
    id: &str,
    f: impl FnOnce(&mut ProjectRow),
) -> Result<ProjectRow, String> {
    let mut row = load(app, id)?;
    f(&mut row);
    global(app).save_project(&row).map_err(|e| e.to_string())?;
    Ok(row)
}

/// A new project id: the time plus random bits.
fn new_id() -> String {
    let mut bytes = [0u8; 4];
    getrandom::fill(&mut bytes).expect("OS randomness");
    let rand: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{:x}{rand}", crate::ctx::now_ms())
}

fn repo_dto(app: &AppHandle, window: &str, root: &Path, row: &ProjectRow) -> ProjectRepo {
    let label = repo::label_of(root);
    let here = app.state::<Repos>().window_of(&label).as_deref() == Some(window);
    ProjectRepo {
        asleep: !here || row.asleep.iter().any(|r| r == root),
        root: root.display().to_string(),
        name: repo::name(root),
        label,
    }
}

/// The project window `window` shows.
pub fn current(app: &AppHandle, window: &str) -> Option<ProjectDto> {
    let projects = app.state::<Projects>();
    let row = load(app, &projects.id(window)?).ok()?;
    Some(ProjectDto {
        repos: row
            .repos
            .iter()
            .map(|r| repo_dto(app, window, r, &row))
            .collect(),
        id: row.id,
        name: row.name,
        ui: row.ui,
        active: projects.active(window),
    })
}

/// Focuses window `window` and has it show the repo with `label`.
fn show(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let w = app
        .get_webview_window(window)
        .ok_or_else(|| format!("no window {window}"))?;
    repo::close_start(app);
    w.unminimize().ok();
    w.set_focus().map_err(|e| e.to_string())?;
    ShowRepo {
        label: label.to_string(),
    }
    .emit_to(app, window)
    .map_err(|e| e.to_string())
}

/// Shows the repo at `root`: in the window that has it, or in the project that has it, which
/// opens, or else in a new project of its own.
pub fn open_repo(app: &AppHandle, root: &Path) -> Result<(), String> {
    let label = repo::label_of(root);
    if let Some(window) = app.state::<Repos>().window_of(&label) {
        return show(app, &window, &label);
    }
    let saved = global(app).projects().map_err(|e| e.to_string())?;
    if let Some(p) = saved.iter().find(|p| p.repos.iter().any(|r| r == root)) {
        if let Some(window) = app.state::<Projects>().window_of(&p.id) {
            wake(app, &window, &label)?;
            return show(app, &window, &label);
        }
        return open_project(app, &p.id, Some(label));
    }
    let row = ProjectRow {
        id: new_id(),
        name: repo::name(root),
        repos: vec![root.to_path_buf()],
        ui: "{}".into(),
        ..Default::default()
    };
    global(app).save_project(&row).map_err(|e| e.to_string())?;
    open_project(app, &row.id, Some(label))
}

/// Opens project `id` in a window, or focuses the window it's in, showing the repo with
/// `label` if given.
pub fn open_project(app: &AppHandle, id: &str, label: Option<String>) -> Result<(), String> {
    let projects = app.state::<Projects>();
    if let Some(window) = projects.window_of(id) {
        return match label {
            Some(label) => show(app, &window, &label),
            None => app
                .get_webview_window(&window)
                .map_or(Ok(()), |w| w.set_focus().map_err(|e| e.to_string())),
        };
    }
    let row = update(app, id, |p| {
        p.open = true;
        p.used = crate::ctx::now_ms();
    })?;
    let window = format!("{PREFIX}{id}");
    let mut started = Vec::new();
    for root in row.repos.iter().filter(|r| !row.asleep.contains(r)) {
        if app
            .state::<Repos>()
            .window_of(&repo::label_of(root))
            .is_some()
        {
            continue;
        }
        match repo::start(app, root, &window) {
            Ok(label) => started.push(label),
            Err(e) => tracing::warn!(repo = %root.display(), "open project repo: {e}"),
        }
    }
    // Unset, the window shows the repo it showed last.
    let active = label.filter(|l| started.contains(l));
    projects.open.lock().unwrap().insert(
        window.clone(),
        Shown {
            id: id.to_string(),
            active,
        },
    );
    repo::window(app, &window, &row.name)
}

/// Opens the projects that were open at the last quit, else the last used project.
pub fn reopen(app: &AppHandle) -> Result<(), String> {
    let saved = global(app).projects().map_err(|e| e.to_string())?;
    let open: Vec<&ProjectRow> = saved.iter().filter(|p| p.open).collect();
    let pick = if open.is_empty() {
        saved.first().into_iter().collect()
    } else {
        open
    };
    if pick.is_empty() {
        // Repos opened before projects come back as projects of their own.
        let last = crate::recent::load(&crate::recent::path(&crate::paths::data_dir(app)?))
            .into_iter()
            .next()
            .ok_or("nothing to reopen")?;
        return open_repo(app, &last);
    }
    for p in pick {
        if let Err(e) = open_project(app, &p.id, None) {
            tracing::error!("reopen project {}: {e}", p.name);
        }
    }
    Ok(())
}

/// A window closed: its repos stop, and its project stays open for the next launch only
/// when the app is quitting.
pub fn closed(app: &AppHandle, window: &str, quitting: bool) {
    for label in app.state::<Repos>().in_window(window) {
        repo::stop(app, &label);
    }
    let Some(shown) = app.state::<Projects>().open.lock().unwrap().remove(window) else {
        return;
    };
    if !quitting && let Err(e) = update(app, &shown.id, |p| p.open = false) {
        tracing::warn!("close project: {e}");
    }
}

fn id_of(app: &AppHandle, window: &str) -> Result<String, String> {
    app.state::<Projects>()
        .id(window)
        .ok_or_else(|| "window has no project".into())
}

/// Adds the repo holding `path` to window `window`'s project. When another window has it,
/// that window shows it instead and this returns `None`.
pub fn add(app: &AppHandle, window: &str, path: &Path) -> Result<Option<ProjectRepo>, String> {
    let root = repo::root_of(path)?;
    let label = repo::label_of(&root);
    if let Some(other) = app.state::<Repos>().window_of(&label)
        && other != window
    {
        show(app, &other, &label)?;
        return Ok(None);
    }
    let id = id_of(app, window)?;
    repo::start(app, &root, window)?;
    let row = update(app, &id, |p| {
        if !p.repos.contains(&root) {
            p.repos.push(root.clone());
        }
        p.asleep.retain(|r| r != &root);
    })?;
    Ok(Some(repo_dto(app, window, &root, &row)))
}

fn root_in(row: &ProjectRow, label: &str) -> Result<PathBuf, String> {
    row.repos
        .iter()
        .find(|r| repo::label_of(r) == label)
        .cloned()
        .ok_or_else(|| format!("no repo {label} in this project"))
}

/// Ends everything of the repo with `label` in window `window`, and keeps it in the project
/// asleep.
pub fn sleep(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let id = id_of(app, window)?;
    let root = root_in(&load(app, &id)?, label)?;
    if app.state::<Repos>().window_of(label).as_deref() == Some(window) {
        repo::stop(app, label);
    }
    update(app, &id, |p| {
        if !p.asleep.contains(&root) {
            p.asleep.push(root);
        }
    })
    .map(drop)
}

/// Wakes the repo with `label` in window `window`'s project.
pub fn wake(app: &AppHandle, window: &str, label: &str) -> Result<ProjectRepo, String> {
    let id = id_of(app, window)?;
    let root = root_in(&load(app, &id)?, label)?;
    match app.state::<Repos>().window_of(label) {
        Some(other) if other != window => {
            return Err(format!("{} is open in another window", repo::name(&root)));
        }
        _ => {}
    }
    repo::start(app, &root, window)?;
    let row = update(app, &id, |p| p.asleep.retain(|r| r != &root))?;
    Ok(repo_dto(app, window, &root, &row))
}

/// Takes the repo with `label` out of window `window`'s project, ending everything of it.
pub fn remove(app: &AppHandle, window: &str, label: &str) -> Result<(), String> {
    let id = id_of(app, window)?;
    let root = root_in(&load(app, &id)?, label)?;
    if app.state::<Repos>().window_of(label).as_deref() == Some(window) {
        repo::stop(app, label);
    }
    update(app, &id, |p| {
        p.repos.retain(|r| r != &root);
        p.asleep.retain(|r| r != &root);
    })
    .map(drop)
}

pub fn rename(app: &AppHandle, window: &str, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("a project needs a name".into());
    }
    update(app, &id_of(app, window)?, |p| p.name = name.to_string())?;
    if let Some(w) = app.get_webview_window(window) {
        w.set_title(name).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn save_ui(app: &AppHandle, window: &str, ui: String) -> Result<(), String> {
    update(app, &id_of(app, window)?, |p| p.ui = ui).map(drop)
}

/// Saved projects not open in a window, most recently used first.
pub fn closed_projects(app: &AppHandle) -> Result<Vec<ProjectSummary>, String> {
    let saved = global(app).projects().map_err(|e| e.to_string())?;
    let projects = app.state::<Projects>();
    Ok(saved
        .into_iter()
        .filter(|p| projects.window_of(&p.id).is_none())
        .map(|p| ProjectSummary {
            repos: p.repos.iter().map(|r| repo::name(r)).collect(),
            id: p.id,
            name: p.name,
        })
        .collect())
}

/// Forgets saved project `id`, unless a window has it open.
pub fn delete(app: &AppHandle, id: &str) -> Result<(), String> {
    if app.state::<Projects>().window_of(id).is_some() {
        return Err("the project is open".into());
    }
    global(app).delete_project(id).map_err(|e| e.to_string())
}
