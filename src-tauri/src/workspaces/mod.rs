//! Workspaces. New ones live under `~/.local/share/lgtm/ws/<repo>/<workspace>`.

pub mod gc;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, bail};
use serde::Serialize;
use specta::Type;

use crate::jj::{Jj, WorkspaceInfo};
use crate::review::RepoCtx;
use crate::watch;
use crate::workspace::Workspace;

/// The repo-relative script a new workspace runs in its first terminal.
pub const SETUP: &str = ".lgtm/setup";

/// The command a new workspace's first terminal runs: its setup script, then a login shell.
pub fn setup_command() -> Vec<String> {
    let shell = crate::pty::default_shell();
    let quoted = format!("'{}'", shell.replace('\'', r"'\''"));
    let script = format!(
        "if [ -x {s} ]; then ./{s}; else sh {s}; fi || echo \"lgtm: {s} exited with $?\"; exec {quoted} -l",
        s = SETUP
    );
    vec![shell, "-l".into(), "-c".into(), script]
}

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDto {
    /// The workspace name, or `null` for the repo's main workspace.
    pub name: Option<String>,
    /// What to show: the workspace name, main included.
    pub title: String,
    pub root: String,
    pub change_id: String,
    /// Length of the shortest prefix of `change_id` that's unique in the repo.
    pub unique: u32,
    pub description: String,
}

/// Where lgtm puts new workspaces: `$XDG_DATA_HOME/lgtm/ws`, else `~/.local/share/lgtm/ws`.
pub fn ws_base() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))?;
    Some(data.join("lgtm/ws"))
}

/// A path component without spaces or shell metacharacters.
fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Workspace names become directory names, so they're kept to a safe set.
pub fn validate_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || name.starts_with(['.', '-']) || slug(name) != name {
        bail!(
            "workspace names use letters, digits, `.`, `_` and `-`, and can't start with `.` or `-`"
        );
    }
    Ok(())
}

fn canon(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

pub(super) fn same_path(a: &Path, b: &Path) -> bool {
    canon(a) == canon(b)
}

/// Whether `path` is inside `base`, comparing canonical paths.
pub fn under(path: &Path, base: &Path) -> bool {
    canon(path).starts_with(canon(base))
}

impl RepoCtx {
    /// The directory new workspaces of this repo go in.
    fn workspace_dir(&self) -> anyhow::Result<PathBuf> {
        let name = self.root.file_name().context("repo root has no name")?;
        Ok(self
            .workspace_base
            .as_ref()
            .context("no home directory")?
            .join(slug(&name.to_string_lossy())))
    }

    /// The workspace named `name`, `None` meaning the main one.
    pub async fn workspace(&self, workspace: Option<&str>) -> anyhow::Result<Arc<Workspace>> {
        let Some(name) = workspace else {
            return Ok(self.main.clone());
        };
        if let Some(ws) = self.workspaces.lock().unwrap().get(name) {
            return Ok(ws.clone());
        }
        self.list_workspaces().await?;
        self.workspaces
            .lock()
            .unwrap()
            .get(name)
            .cloned()
            .with_context(|| format!("no workspace named `{name}`"))
    }

    /// Every workspace, main first, and starts watching any not seen before.
    pub async fn list_workspaces(&self) -> anyhow::Result<Vec<WorkspaceDto>> {
        let listed = self.main.jj.workspaces().await?;
        let (main, others): (Vec<WorkspaceInfo>, Vec<WorkspaceInfo>) = listed
            .into_iter()
            .partition(|w| same_path(&w.root, &self.root));
        let mut map = self.workspaces.lock().unwrap();
        map.retain(|name, _| others.iter().any(|w| &w.name == name));
        for w in &others {
            if !map.contains_key(&w.name) {
                let ws = Arc::new(Workspace::new(
                    Some(w.name.clone()),
                    w.root.clone(),
                    Jj::new(&w.root),
                ));
                self.watch(&ws);
                map.insert(w.name.clone(), ws);
            }
        }
        let dto = |w: WorkspaceInfo, main: bool| WorkspaceDto {
            name: (!main).then(|| w.name.clone()),
            title: w.name,
            root: w.root.display().to_string(),
            change_id: w.change_id,
            unique: w.unique,
            description: w.description,
        };
        Ok(main
            .into_iter()
            .map(|w| dto(w, true))
            .chain(others.into_iter().map(|w| dto(w, false)))
            .collect())
    }

    /// Starts reporting the workspace's snapshots and staleness, once the window is wired up.
    pub fn watch(&self, ws: &Arc<Workspace>) {
        let Some(emit) = self.emit.lock().unwrap().clone() else {
            return;
        };
        let name = ws.name.clone();
        let started = watch::start(&ws.root, ws.jj.clone(), ws.stale.clone(), move |e| {
            emit(name.clone(), e)
        });
        match started {
            Ok(w) => *ws.watch.lock().unwrap() = Some(w),
            Err(e) => eprintln!("lgtm: watch {}: {e:#}", ws.root.display()),
        }
    }

    /// Adds a workspace on `trunk()`. Returns whether it has a setup script.
    pub async fn create_workspace(&self, name: &str) -> anyhow::Result<bool> {
        self.add_workspace(name, "trunk()").await
    }

    /// Adds a workspace with a new commit on `rev`. Returns whether it has a setup script.
    pub(crate) async fn add_workspace(&self, name: &str, rev: &str) -> anyhow::Result<bool> {
        validate_name(name)?;
        let path = self.workspace_dir()?.join(name);
        if path.exists() {
            bail!("{} already exists", path.display());
        }
        std::fs::create_dir_all(path.parent().unwrap())?;
        self.main.jj.workspace_add(name, &path, rev).await?;
        self.list_workspaces().await?;
        Ok(path.join(SETUP).is_file())
    }

    /// Forgets the workspace and, when lgtm created it, deletes its directory. The
    /// workspace's commits stay in the repo.
    pub async fn delete_workspace(&self, name: &str) -> anyhow::Result<()> {
        let ws = self.workspace(Some(name)).await?;
        self.main.jj.workspace_forget(name).await?;
        self.workspaces.lock().unwrap().remove(name);
        let ours = self
            .workspace_base
            .as_ref()
            .is_some_and(|b| under(&ws.root, b));
        // The workspace is forgotten already, so its PR goes even if its directory stays.
        for problem in self.forget_pr(name).await {
            eprintln!("lgtm: cleaning up after {name}: {problem}");
        }
        if ours && ws.root.is_dir() {
            std::fs::remove_dir_all(&ws.root)
                .with_context(|| format!("remove {}", ws.root.display()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
