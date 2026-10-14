use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::github::Verdict;
use crate::pr::{DiscussionDto, Fixed, HeadDto, NearbyPrsDto, PrDto, PrPreviewDto};
use crate::resolve::ResolveDto;
use crate::review::StateDto;

/// Checks out the PR `input` names (a number or a URL) into its own workspace, or finds the
/// one that has it. Returns the workspace.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_pr(window: WebviewWindow, input: String) -> CmdResult<String> {
    repo(&window)?.open_pr(&input).await.map_err(err)
}

/// Open PRs of the bookmarks pushed from the workspace's stack, nearest `@` first. GitHub is
/// asked again only with `refresh` or when the bookmarks change.
#[tauri::command]
#[specta::specta]
pub(crate) async fn nearby_prs(
    window: WebviewWindow,
    workspace: Option<String>,
    refresh: bool,
) -> CmdResult<NearbyPrsDto> {
    repo(&window)?
        .nearby_prs(workspace.as_deref(), refresh)
        .await
        .map_err(err)
}

/// Brings the workspace's stack PR in line with `@`, and returns the workspace's PR.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_stack_pr(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Option<PrDto>> {
    repo(&window)?
        .sync_stack_pr(workspace.as_deref())
        .await
        .map_err(err)
}

/// Picks stack PR `number` as the workspace's PR. With `edit`, edits its change first.
#[tauri::command]
#[specta::specta]
pub(crate) async fn select_stack_pr(
    window: WebviewWindow,
    workspace: Option<String>,
    number: u32,
    edit: bool,
) -> CmdResult<PrDto> {
    let repo = repo(&window)?;
    let ws = workspace.as_deref();
    let pr = if edit {
        repo.edit_stack_pr(ws, number).await
    } else {
        repo.select_stack_pr(ws, number).await
    };
    pr.map_err(err)
}

/// Deselects the workspace's stack PR.
#[tauri::command]
#[specta::specta]
pub(crate) fn clear_stack_pr(window: WebviewWindow, workspace: Option<String>) -> CmdResult<()> {
    repo(&window)?
        .clear_stack_pr(workspace.as_deref())
        .map_err(err)
}

/// The PR `input` names (a number or a URL), read from GitHub without fetching it: its
/// conversation and its diff with review threads.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pr_preview(window: WebviewWindow, input: String) -> CmdResult<PrPreviewDto> {
    repo(&window)?.pr_preview(&input).await.map_err(err)
}

/// The workspace's PR, checked out or picked from its stack, if any.
#[tauri::command]
#[specta::specta]
pub(crate) fn pr_info(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Option<PrDto>> {
    let pr = repo(&window)?
        .pr_record(workspace.as_deref())
        .map_err(err)?;
    Ok(pr.as_ref().map(PrDto::from))
}

/// Opens the workspace's PR on GitHub in the browser.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_pr_page(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<()> {
    repo(&window)?
        .open_pr_page(workspace.as_deref())
        .await
        .map_err(err)
}

/// How the PR's head on GitHub compares with the fetched one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pr_head(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<HeadDto> {
    repo(&window)?
        .pr_head(workspace.as_deref())
        .await
        .map_err(err)
}

/// Fetches the PR again and moves the workspace onto its new head. `review`'s session ends
/// first, so it can show what changed.
#[tauri::command]
#[specta::specta]
pub(crate) async fn refetch_pr(
    window: WebviewWindow,
    workspace: Option<String>,
    review: Option<u32>,
) -> CmdResult<()> {
    repo(&window)?
        .refetch_pr(workspace.as_deref(), review)
        .await
        .map_err(err)
}

/// Pulls the PR's review threads into the review as comments.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pull_pr_comments(window: WebviewWindow, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.pull_pr_comments(review).await.map_err(err)
}

/// Sends the PR's ready threads to GitHub and submits the review with `verdict` and
/// `summary`. `fixed` says what happens to ready threads with no reply.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_threads(
    window: WebviewWindow,
    review: u32,
    fixed: Fixed,
    verdict: Verdict,
    summary: String,
) -> CmdResult<StateDto> {
    repo(&window)?
        .send_threads(review, fixed, verdict, &summary)
        .await
        .map_err(err)
}

/// Sends one ready thread to GitHub.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_thread(
    window: WebviewWindow,
    review: u32,
    comment: u32,
) -> CmdResult<StateDto> {
    repo(&window)?
        .send_thread(review, comment)
        .await
        .map_err(err)
}

/// The PR's conversation: comments and review summaries.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pr_discussion(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<DiscussionDto> {
    repo(&window)?
        .pr_discussion(workspace.as_deref())
        .await
        .map_err(err)
}

/// Posts to the PR's conversation right away.
#[tauri::command]
#[specta::specta]
pub(crate) async fn reply_pr(
    window: WebviewWindow,
    workspace: Option<String>,
    body: String,
) -> CmdResult<()> {
    repo(&window)?
        .reply_pr(workspace.as_deref(), &body)
        .await
        .map_err(err)
}

/// Has an agent resolve a conflicted change, streaming its reply through `on_text`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn resolve_conflict(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    change: String,
    on_text: Channel<String>,
) -> CmdResult<ResolveDto> {
    repo(&window)?
        .resolve_conflict(workspace.as_deref(), &change, &runs, |t| {
            on_text.send(t).ok();
        })
        .await
        .map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) fn stop_resolve(
    window: WebviewWindow,
    runs: State<Runs>,
    workspace: Option<String>,
) -> CmdResult<()> {
    repo(&window)?.stop_resolve(workspace.as_deref(), &runs);
    Ok(())
}

/// Opens a web page in the default browser.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_url(url: String) -> CmdResult<()> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err(format!("{url} isn't a web address"));
    }
    let status = tokio::process::Command::new("open")
        .arg(&url)
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("can't open {url}"));
    }
    Ok(())
}
