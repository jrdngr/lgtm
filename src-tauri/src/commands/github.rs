use super::{CmdResult, err, repo};
use crate::frame::Frame;
use crate::github::Verdict;
use crate::pr::{DiscussionDto, Fixed, HeadDto, NearbyPrsDto, PrDto, PrPreviewDto};
use crate::review::StateDto;

/// Checks out the PR `input` names (a number or a URL) into its own workspace, or finds the
/// one that has it. Returns the workspace.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_pr(window: Frame, input: String) -> CmdResult<String> {
    repo(&window)?.open_pr(&input).await.map_err(err)
}

/// Open PRs of the bookmarks pushed from the workspace's stack, nearest `@` first. GitHub is
/// asked again only with `refresh` or when the bookmarks change.
#[tauri::command]
#[specta::specta]
pub(crate) async fn nearby_prs(
    window: Frame,
    workspace: Option<String>,
    refresh: bool,
) -> CmdResult<NearbyPrsDto> {
    repo(&window)?
        .nearby_prs(workspace.as_deref(), refresh)
        .await
        .map_err(err)
}

/// Keeps the workspace's stack PR's head current, and returns the workspace's PR.
#[tauri::command]
#[specta::specta]
pub(crate) async fn sync_stack_pr(
    window: Frame,
    workspace: Option<String>,
) -> CmdResult<Option<PrDto>> {
    repo(&window)?
        .sync_stack_pr(workspace.as_deref())
        .await
        .map_err(err)
}

/// Makes stack PR `number` the workspace's PR, to review its commits.
#[tauri::command]
#[specta::specta]
pub(crate) async fn review_stack_pr(
    window: Frame,
    workspace: Option<String>,
    number: u32,
) -> CmdResult<PrDto> {
    repo(&window)?
        .review_stack_pr(workspace.as_deref(), number)
        .await
        .map_err(err)
}

/// Pins stack PR `number`, making it the workspace's PR, or unpins it.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pin_stack_pr(
    window: Frame,
    workspace: Option<String>,
    number: u32,
    pinned: bool,
) -> CmdResult<PrDto> {
    repo(&window)?
        .pin_stack_pr(workspace.as_deref(), number, pinned)
        .await
        .map_err(err)
}

/// Drops the workspace's stack PR unless it's pinned or the review takes in any of its
/// commits. Whether the workspace still has a PR.
#[tauri::command]
#[specta::specta]
pub(crate) async fn follow_review(
    window: Frame,
    workspace: Option<String>,
    revset: String,
) -> CmdResult<bool> {
    repo(&window)?
        .follow_review(workspace.as_deref(), &revset)
        .await
        .map_err(err)
}

/// The PR `input` names (a number or a URL), read from GitHub without fetching it: its
/// conversation and its diff with review threads.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pr_preview(window: Frame, input: String) -> CmdResult<PrPreviewDto> {
    repo(&window)?.pr_preview(&input).await.map_err(err)
}

/// The workspace's PR, checked out or picked from its stack, if any.
#[tauri::command]
#[specta::specta]
pub(crate) fn pr_info(window: Frame, workspace: Option<String>) -> CmdResult<Option<PrDto>> {
    let pr = repo(&window)?
        .pr_record(workspace.as_deref())
        .map_err(err)?;
    Ok(pr.as_ref().map(PrDto::from))
}

/// Opens the workspace's PR on GitHub in the browser.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_pr_page(window: Frame, workspace: Option<String>) -> CmdResult<()> {
    repo(&window)?
        .open_pr_page(workspace.as_deref())
        .await
        .map_err(err)
}

/// How the PR's head on GitHub compares with the fetched one.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pr_head(window: Frame, workspace: Option<String>) -> CmdResult<HeadDto> {
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
    window: Frame,
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
pub(crate) async fn pull_pr_comments(window: Frame, review: u32) -> CmdResult<StateDto> {
    repo(&window)?.pull_pr_comments(review).await.map_err(err)
}

/// Sends the PR's ready threads to GitHub and submits the review with `verdict` and
/// `summary`. `fixed` says what happens to ready threads with no reply.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_threads(
    window: Frame,
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
pub(crate) async fn send_thread(window: Frame, review: u32, comment: u32) -> CmdResult<StateDto> {
    repo(&window)?
        .send_thread(review, comment)
        .await
        .map_err(err)
}

/// The PR's conversation: comments and review summaries.
#[tauri::command]
#[specta::specta]
pub(crate) async fn pr_discussion(
    window: Frame,
    workspace: Option<String>,
) -> CmdResult<DiscussionDto> {
    repo(&window)?
        .pr_discussion(workspace.as_deref())
        .await
        .map_err(err)
}

/// Posts `body` to the PR's conversation as the reply of quote threads `comments`, which
/// then are done.
#[tauri::command]
#[specta::specta]
pub(crate) async fn send_combined(
    window: Frame,
    review: u32,
    comments: Vec<u32>,
    body: String,
) -> CmdResult<StateDto> {
    repo(&window)?
        .send_combined(review, &comments, &body)
        .await
        .map_err(err)
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
