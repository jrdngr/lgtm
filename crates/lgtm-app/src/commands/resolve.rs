//! Resolving a working-copy file's conflicts: one session per open file, kept here while its
//! tab is open, so the panel only draws what the session says.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use lgtm_core::resolve::{self, FileChoice, ResolveView, Saved, Session};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{CmdResult, err};
use crate::frame::Frame;
use crate::turns;

/// The open sessions, by workspace root and path.
static SESSIONS: LazyLock<Mutex<HashMap<String, Session>>> = LazyLock::new(Default::default);

fn key(root: &std::path::Path, path: &str) -> String {
    format!("{}\n{path}", root.display())
}

/// A change to a session.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ResolveAct {
    /// Keeps side `key` of `conflict` after the others it keeps, or drops it.
    Toggle {
        conflict: u32,
        key: u8,
    },
    /// Moves the kept side at `from` to `to`.
    Place {
        conflict: u32,
        from: u32,
        to: u32,
    },
    Edit {
        conflict: u32,
        pick: u32,
        text: String,
    },
    /// Switches an edited side between its edit and its text from before.
    Flip {
        conflict: u32,
        pick: u32,
    },
    Choose {
        choice: FileChoice,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct ResolveSaved {
    pub view: ResolveView,
    pub saved: Saved,
}

fn with<T>(root: &std::path::Path, path: &str, f: impl FnOnce(&mut Session) -> T) -> CmdResult<T> {
    let mut sessions = SESSIONS.lock().map_err(|e| e.to_string())?;
    let session = sessions
        .get_mut(&key(root, path))
        .ok_or_else(|| format!("{path} isn't being resolved"))?;
    Ok(f(session))
}

/// The session on `path`, started from the file when there's none or `fresh` asks.
#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_open(
    window: Frame,
    workspace: Option<String>,
    path: String,
    fresh: bool,
) -> CmdResult<ResolveView> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    super::files::contained(&ws, &path)?;
    let k = key(&ws.root, &path);
    if !fresh && let Some(s) = SESSIONS.lock().map_err(|e| e.to_string())?.get(&k) {
        return Ok(s.view());
    }
    let session = resolve::load(&ws.root, &path, Some(&ws.jj))
        .await
        .map_err(|e| format!("couldn't read {path}: {e}"))?;
    let view = session.view();
    SESSIONS
        .lock()
        .map_err(|e| e.to_string())?
        .insert(k, session);
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_act(
    window: Frame,
    workspace: Option<String>,
    path: String,
    act: ResolveAct,
) -> CmdResult<ResolveView> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    with(&ws.root, &path, |s| {
        match act {
            ResolveAct::Toggle { conflict, key } => s.toggle(conflict as usize, key),
            ResolveAct::Place { conflict, from, to } => {
                s.place(conflict as usize, from as usize, to as usize)
            }
            ResolveAct::Edit {
                conflict,
                pick,
                text,
            } => s.edit(conflict as usize, pick as usize, &text),
            ResolveAct::Flip { conflict, pick } => {
                s.flip_original(conflict as usize, pick as usize)
            }
            ResolveAct::Choose { choice } => s.choose(choice),
            ResolveAct::Undo => s.undo(),
            ResolveAct::Redo => s.redo(),
        };
        s.view()
    })
}

/// Writes what's resolved, between two snapshots of the working copy as a save is.
#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_save(
    window: Frame,
    workspace: Option<String>,
    path: String,
) -> CmdResult<ResolveSaved> {
    let repo = super::repo(&window)?;
    let ws = repo.workspace(workspace.as_deref()).await.map_err(err)?;
    let before = turns::working_commit(&ws).await;
    let (view, saved) = with(&ws.root, &path, |s| {
        s.save(&ws.root).map(|saved| (s.view(), saved))
    })?
    .map_err(|e| format!("couldn't save {path}: {e}"))?;
    match before {
        Ok(before) if !saved.removed => {
            repo.record_save(&ws, &path, &before).await.map_err(err)?;
        }
        _ => ws.snapshot().await.map_err(err)?,
    }
    Ok(ResolveSaved { view, saved })
}

/// Ends the session on `path`, as its tab closes.
#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_close(
    window: Frame,
    workspace: Option<String>,
    path: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    SESSIONS
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&key(&ws.root, &path));
    Ok(())
}
