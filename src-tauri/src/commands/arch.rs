use tauri::WebviewWindow;

use super::{CmdResult, err, repo};
use crate::review::arch::edge::EdgeSitesDto;
use crate::review::arch::explore::SysSymbolDto;
use crate::review::arch::symbols::SymbolsDto;
use crate::review::arch::system::SystemDto;
use crate::review::arch::view::ArchViewDto;
use crate::review::arch::{ArchDto, ArchRange, HistoryDto};

/// The review's architecture diff, or the diff between two of its commits.
#[tauri::command]
#[specta::specta]
pub(crate) async fn review_arch(
    window: WebviewWindow,
    review: u32,
    transitive: bool,
    range: Option<ArchRange>,
) -> CmdResult<ArchDto> {
    repo(&window)?
        .arch(review, transitive, range)
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn arch_history(window: WebviewWindow, review: u32) -> CmdResult<Vec<HistoryDto>> {
    repo(&window)?.arch_history(review).await.map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn check_look(
    window: WebviewWindow,
    review: u32,
    key: String,
    checked: bool,
) -> CmdResult<()> {
    repo(&window)?
        .check_look(review, &key, checked)
        .map_err(err)
}

/// The prompt asking a terminal agent to assess the review's architecture change.
#[tauri::command]
#[specta::specta]
pub(crate) async fn assess_prompt(window: WebviewWindow, review: u32) -> CmdResult<String> {
    repo(&window)?.assess_prompt(review).await.map_err(err)
}

/// The system map of a workspace's working copy.
#[tauri::command]
#[specta::specta]
pub(crate) async fn system_map(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<SystemDto> {
    repo(&window)?
        .system_map(workspace.as_deref())
        .await
        .map_err(err)
}

/// What the system map shows.
#[tauri::command]
#[specta::specta]
pub(crate) fn arch_view(window: WebviewWindow) -> CmdResult<ArchViewDto> {
    repo(&window)?.arch_view().map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn set_arch_view(window: WebviewWindow, view: ArchViewDto) -> CmdResult<()> {
    repo(&window)?.set_arch_view(&view).map_err(err)
}

/// The prompt asking a terminal agent to describe the system map.
#[tauri::command]
#[specta::specta]
pub(crate) async fn describe_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<String> {
    repo(&window)?
        .describe_prompt(workspace.as_deref())
        .await
        .map_err(err)
}

/// A component's symbols other components use most.
#[tauri::command]
#[specta::specta]
pub(crate) async fn system_most_used(
    window: WebviewWindow,
    workspace: Option<String>,
    component: String,
) -> CmdResult<Vec<SysSymbolDto>> {
    repo(&window)?
        .system_most_used(workspace.as_deref(), component)
        .await
        .map_err(err)
}

/// Symbols in a workspace's working copy whose names hold `query`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn system_find(
    window: WebviewWindow,
    workspace: Option<String>,
    query: String,
) -> CmdResult<Vec<SysSymbolDto>> {
    repo(&window)?
        .system_find(workspace.as_deref(), query)
        .await
        .map_err(err)
}

/// The prompt asking a terminal agent to write the tour of a container or component.
#[tauri::command]
#[specta::specta]
pub(crate) async fn tour_prompt(
    window: WebviewWindow,
    workspace: Option<String>,
    node: String,
) -> CmdResult<String> {
    repo(&window)?
        .tour_prompt(workspace.as_deref(), &node)
        .await
        .map_err(err)
}

/// A component's symbols, the symbols in other components linked to them, and their uses.
#[tauri::command]
#[specta::specta]
pub(crate) async fn system_symbols(
    window: WebviewWindow,
    workspace: Option<String>,
    component: String,
) -> CmdResult<SymbolsDto> {
    repo(&window)?
        .system_symbols(workspace.as_deref(), component)
        .await
        .map_err(err)
}

/// Where one component imports another, and the symbols it names there.
#[tauri::command]
#[specta::specta]
pub(crate) async fn system_edge(
    window: WebviewWindow,
    workspace: Option<String>,
    from: String,
    to: String,
) -> CmdResult<EdgeSitesDto> {
    repo(&window)?
        .system_edge(workspace.as_deref(), from, to)
        .await
        .map_err(err)
}
