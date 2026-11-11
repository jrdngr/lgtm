use super::{CmdResult, err, repo};
use crate::canvas::{Canvas, CanvasFocus};
use crate::drawings::DrawingDto;
use crate::frame::Frame;

/// Every drawing of the repo, the last changed first.
#[tauri::command]
#[specta::specta]
pub(crate) fn drawings(window: Frame) -> CmdResult<Vec<DrawingDto>> {
    repo(&window)?.drawings().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn drawing(window: Frame, id: String) -> CmdResult<DrawingDto> {
    repo(&window)?.drawing(&id).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn create_drawing(window: Frame, name: String) -> CmdResult<DrawingDto> {
    repo(&window)?.create_drawing(&name).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn rename_drawing(window: Frame, id: String, name: String) -> CmdResult<DrawingDto> {
    repo(&window)?.rename_drawing(&id, &name).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn delete_drawing(window: Frame, id: String) -> CmdResult<()> {
    repo(&window)?.delete_drawing(&id).map_err(err)
}

/// Stores the user's drawing.
#[tauri::command]
#[specta::specta]
pub(crate) fn save_drawing_canvas(
    window: Frame,
    id: String,
    canvas: Canvas,
) -> CmdResult<DrawingDto> {
    repo(&window)?.save_drawing_canvas(&id, canvas).map_err(err)
}

/// Undoes the last agent pass over a drawing.
#[tauri::command]
#[specta::specta]
pub(crate) fn undo_drawing_agent(window: Frame, id: String) -> CmdResult<DrawingDto> {
    repo(&window)?.undo_drawing_agent(&id).map_err(err)
}

/// Records the canvas a workspace shows and what's selected on it, for agents.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_canvas_focus(
    window: Frame,
    workspace: Option<String>,
    focus: Option<CanvasFocus>,
) -> CmdResult<()> {
    repo(&window)?.set_canvas_focus(workspace.as_deref(), focus);
    Ok(())
}

/// Writes an exported drawing to `path`, which the user picked in a save dialog.
#[tauri::command]
#[specta::specta]
pub(crate) fn write_export(path: String, data: Vec<u8>) -> CmdResult<()> {
    std::fs::write(&path, data).map_err(|e| format!("{path}: {e}"))
}
