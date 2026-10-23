use tauri::{Manager, WebviewWindow};
use tauri_specta::Event as _;

use super::{CmdResult, err, repo};
use crate::events::NotesChanged;
use crate::notes::{GlobalStore, NoteBook, NoteDto, NoteLevel, NotesDto};

/// Runs `f` on the notes `workspace` sees. With `changed`, tells every window, since global
/// notes show in each.
fn book<T>(
    window: &WebviewWindow,
    workspace: Option<String>,
    changed: Option<&str>,
    f: impl FnOnce(&NoteBook) -> anyhow::Result<T>,
) -> CmdResult<T> {
    let ctx = repo(window)?;
    let global = window.state::<GlobalStore>();
    let book = NoteBook {
        global: &global.0,
        repo: &ctx.store,
        workspace: workspace.unwrap_or_default(),
    };
    let out = f(&book).map_err(err)?;
    if let Some(id) = changed {
        NotesChanged {
            id: (!id.is_empty()).then(|| id.to_string()),
        }
        .emit(window.app_handle())
        .ok();
    }
    Ok(out)
}

/// Every note and folder the workspace shows: global, the repo's and its own.
#[tauri::command]
#[specta::specta]
pub(crate) fn notes(window: WebviewWindow, workspace: Option<String>) -> CmdResult<NotesDto> {
    book(&window, workspace, None, |b| b.list())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn note(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, None, |b| b.get(&id))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn create_note(
    window: WebviewWindow,
    workspace: Option<String>,
    level: NoteLevel,
    folder: String,
    name: String,
    body: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, Some(""), |b| {
        b.create(level, &folder, &name, &body)
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn save_note(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
    body: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, Some(&id), |b| b.save(&id, &body))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_note(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
    name: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, Some(&id), |b| b.rename(&id, &name))
}

/// Moves a note into a folder of any level.
#[tauri::command]
#[specta::specta]
pub(crate) fn move_note(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
    level: NoteLevel,
    folder: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, Some(&id), |b| {
        b.move_to(&id, level, &folder)
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_note(
    window: WebviewWindow,
    workspace: Option<String>,
    id: String,
) -> CmdResult<()> {
    book(&window, workspace, Some(&id), |b| b.delete(&id))
}

/// Makes folder `name` in `parent`. Returns its path.
#[tauri::command]
#[specta::specta]
pub(crate) fn create_note_folder(
    window: WebviewWindow,
    workspace: Option<String>,
    level: NoteLevel,
    parent: String,
    name: String,
) -> CmdResult<String> {
    book(&window, workspace, Some(""), |b| {
        b.add_folder(level, &parent, &name)
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_note_folder(
    window: WebviewWindow,
    workspace: Option<String>,
    level: NoteLevel,
    path: String,
    name: String,
) -> CmdResult<()> {
    book(&window, workspace, Some(""), |b| {
        b.rename_folder(level, &path, &name)
    })
}

/// Deletes a folder with every note and folder in it.
#[tauri::command]
#[specta::specta]
pub(crate) fn delete_note_folder(
    window: WebviewWindow,
    workspace: Option<String>,
    level: NoteLevel,
    path: String,
) -> CmdResult<()> {
    book(&window, workspace, Some(""), |b| {
        b.delete_folder(level, &path)
    })
}
