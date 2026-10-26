use std::path::{Path, PathBuf};

use tauri::WebviewWindow;

use super::{CmdResult, err};
use crate::lsp::{self, At, Place};
use crate::workspace::Workspace;

/// A file to ask a language server about: in the workspace, or a library file a definition
/// led to.
fn file(ws: &Workspace, path: &str) -> CmdResult<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() {
        return if lsp::shown_external(p) {
            Ok(p.to_path_buf())
        } else {
            Err(format!("{path} isn't in the workspace"))
        };
    }
    let abs = crate::files::resolve(&ws.root, path).map_err(err)?;
    crate::files::contain(&ws.root, &abs).map_err(err)?;
    Ok(abs)
}

/// Where the symbol at 0-based `line` and UTF-16 `character` of `path` is defined, by the
/// language server for the file. `text` is the file as the editor has it. `None` when no
/// server on PATH takes the file.
#[tauri::command]
#[specta::specta]
pub(crate) async fn lsp_definition(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
    text: Option<String>,
    line: u32,
    character: u32,
) -> CmdResult<Option<Vec<Place>>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let file = file(&ws, &path)?;
    let at = At {
        root: &ws.root,
        file: &file,
        text,
        line,
        character,
    };
    lsp::definition(at).await.map_err(err)
}

/// The language server's markdown about the symbol at `line` and `character` of `path`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn lsp_hover(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
    text: Option<String>,
    line: u32,
    character: u32,
) -> CmdResult<Option<String>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let file = file(&ws, &path)?;
    let at = At {
        root: &ws.root,
        file: &file,
        text,
        line,
        character,
    };
    lsp::hover(at).await.map_err(err)
}
