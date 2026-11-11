use tauri::Manager;

use super::{CmdResult, err, repo};
use crate::frame::Frame;
use crate::notes::{GlobalStore, NoteBook, NoteDto, NoteLevel, NotesDto};

/// Runs `f` on the notes `workspace` sees.
fn book<T>(
    window: &Frame,
    workspace: Option<String>,
    f: impl FnOnce(&NoteBook) -> anyhow::Result<T>,
) -> CmdResult<T> {
    let ctx = repo(window)?;
    let global = window.state::<GlobalStore>();
    let book = NoteBook {
        global: &global.0,
        repo: &ctx.store,
        workspace: workspace.unwrap_or_default(),
        notify: Some(&ctx),
    };
    f(&book).map_err(err)
}

/// Every note and folder the workspace shows: global, the repo's and its own.
#[tauri::command]
#[specta::specta]
pub(crate) fn notes(window: Frame, workspace: Option<String>) -> CmdResult<NotesDto> {
    book(&window, workspace, |b| b.list())
}

#[tauri::command]
#[specta::specta]
pub(crate) fn note(window: Frame, workspace: Option<String>, id: String) -> CmdResult<NoteDto> {
    book(&window, workspace, |b| b.get(&id))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn create_note(
    window: Frame,
    workspace: Option<String>,
    level: NoteLevel,
    folder: String,
    name: String,
    body: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, |b| {
        b.create(level, &folder, &name, &body)
    })
}

#[tauri::command]
#[specta::specta]
pub(crate) fn save_note(
    window: Frame,
    workspace: Option<String>,
    id: String,
    body: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, |b| b.save(&id, &body))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_note(
    window: Frame,
    workspace: Option<String>,
    id: String,
    name: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, |b| b.rename(&id, &name))
}

/// Moves a note into a folder of any level.
#[tauri::command]
#[specta::specta]
pub(crate) fn move_note(
    window: Frame,
    workspace: Option<String>,
    id: String,
    level: NoteLevel,
    folder: String,
) -> CmdResult<NoteDto> {
    book(&window, workspace, |b| b.move_to(&id, level, &folder))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_note(window: Frame, workspace: Option<String>, id: String) -> CmdResult<()> {
    book(&window, workspace, |b| b.delete(&id))
}

/// Makes folder `name` in `parent`. Returns its path.
#[tauri::command]
#[specta::specta]
pub(crate) fn create_note_folder(
    window: Frame,
    workspace: Option<String>,
    level: NoteLevel,
    parent: String,
    name: String,
) -> CmdResult<String> {
    book(&window, workspace, |b| b.add_folder(level, &parent, &name))
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_note_folder(
    window: Frame,
    workspace: Option<String>,
    level: NoteLevel,
    path: String,
    name: String,
) -> CmdResult<()> {
    book(&window, workspace, |b| b.rename_folder(level, &path, &name))
}

/// Deletes a folder with every note and folder in it.
#[tauri::command]
#[specta::specta]
pub(crate) fn delete_note_folder(
    window: Frame,
    workspace: Option<String>,
    level: NoteLevel,
    path: String,
) -> CmdResult<()> {
    book(&window, workspace, |b| b.delete_folder(level, &path))
}
