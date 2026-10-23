use tauri::WebviewWindow;
use tauri_specta::Event as _;

use super::{CmdResult, err};
use crate::events::SymbolInfoChanged;
use crate::search::{self, Hits, SymbolHit, TextQuery};
use crate::store::InfoKey;
use crate::symbol_info::{self, SymbolInfoDto};

/// Lines of the workspace's working copy matching `query`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn search_text(
    window: WebviewWindow,
    workspace: Option<String>,
    query: TextQuery,
) -> CmdResult<Hits> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    tokio::task::spawn_blocking(move || search::search(&ws.root, &query))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Replaces `query`'s matches with `replacement` in the working copy. Returns how many files
/// changed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn replace_text(
    window: WebviewWindow,
    workspace: Option<String>,
    query: TextQuery,
    replacement: String,
) -> CmdResult<u32> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    tokio::task::spawn_blocking(move || search::replace(&ws.root, &query, &replacement))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)
}

/// Where `name` appears as an identifier in the working copy, definitions first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn find_references(
    window: WebviewWindow,
    workspace: Option<String>,
    name: String,
) -> CmdResult<Hits> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    tokio::task::spawn_blocking(move || search::references(&ws.root, &name))
        .await
        .map_err(|e| e.to_string())
}

/// Symbols in the working copy matching `query`, best first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn workspace_symbols(
    window: WebviewWindow,
    workspace: Option<String>,
    query: String,
) -> CmdResult<Vec<SymbolHit>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    tokio::task::spawn_blocking(move || search::symbols(&ws.root, &query, 60))
        .await
        .map_err(|e| e.to_string())
}

/// Every file in the working copy, relative, skipping ignored ones.
#[tauri::command]
#[specta::specta]
pub(crate) async fn workspace_files(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<String>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    tokio::task::spawn_blocking(move || {
        search::files(&ws.root).into_iter().map(|(_, rel)| rel).collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// The prompt asking the agent about `name`. Its definition is the one in `path` at `line`
/// when there is one there, else the first in the workspace.
#[tauri::command]
#[specta::specta]
pub(crate) async fn request_symbol_info(
    window: WebviewWindow,
    workspace: Option<String>,
    name: String,
    path: Option<String>,
    line: Option<u32>,
) -> CmdResult<SymbolAsk> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let root = ws.root.clone();
    let n = name.clone();
    let def = tokio::task::spawn_blocking(move || {
        symbol_info::locate(&root, &n, path.as_deref(), line)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(err)?;
    Ok(SymbolAsk {
        prompt: symbol_info::request(&def, &name),
        path: def.path,
        nth: def.nth,
        line: def.line,
    })
}

/// Where a symbol asked about is defined, and the prompt to send.
#[derive(serde::Serialize, specta::Type)]
pub(crate) struct SymbolAsk {
    path: String,
    nth: u32,
    line: u32,
    prompt: String,
}

/// Marks a definition as asked about, once its prompt went out.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mark_symbol_asked(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
    name: String,
    nth: u32,
) -> CmdResult<()> {
    let ctx = super::repo(&window)?;
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    symbol_info::asked(&ctx.store.lock().unwrap(), ws.key(), &ws.root, &path, &name, nth)
        .map_err(err)?;
    SymbolInfoChanged.emit(&window).ok();
    Ok(())
}

/// Symbol Information on symbols defined in `paths`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn symbol_infos(
    window: WebviewWindow,
    workspace: Option<String>,
    paths: Vec<String>,
) -> CmdResult<Vec<SymbolInfoDto>> {
    let ctx = super::repo(&window)?;
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    symbol_info::infos(&ctx.store.lock().unwrap(), ws.key(), &ws.root, &paths).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn dismiss_symbol_info(
    window: WebviewWindow,
    workspace: Option<String>,
    path: String,
    name: String,
    nth: u32,
) -> CmdResult<()> {
    let ctx = super::repo(&window)?;
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let key = InfoKey { workspace: ws.key(), path: &path, name: &name, nth };
    ctx.store.lock().unwrap().delete_symbol_info(&key).map_err(|e| e.to_string())?;
    SymbolInfoChanged.emit(&window).ok();
    Ok(())
}
