use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::ipc::Channel;
use tauri::{State, WebviewWindow};

use super::{CmdResult, err, repo};
use crate::agent::Runs;
use crate::design::agent::ArtifactKind;
use crate::design::queue::Answer;
use crate::design::{CommentKind, CommentState, DesignDto, DocComment, edit_queue, inside};

/// The workspace's designs.
#[tauri::command]
#[specta::specta]
pub(crate) async fn list_designs(
    window: WebviewWindow,
    workspace: Option<String>,
) -> CmdResult<Vec<DesignDto>> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.designs(&ws).map_err(err)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn create_design(
    window: WebviewWindow,
    workspace: Option<String>,
    name: String,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.create_design(&ws, &name).map_err(err)
}

/// Saves the brief and the paths the agent reads before drafting.
#[tauri::command]
#[specta::specta]
pub(crate) async fn save_design_brief(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    brief: String,
    attach: Vec<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .save_brief(&ws, &slug, &brief, attach)
        .map_err(err)
}

/// A file in the design's folder, and what it said when the user last looked.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignText {
    pub text: String,
    pub seen: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn design_file(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    path: String,
) -> CmdResult<DesignText> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let ctx = repo(&window)?;
    Ok(DesignText {
        text: ctx.design_file(&ws, &slug, &path).map_err(err)?,
        seen: ctx.design_seen(&ws, &slug, &path).map_err(err)?,
    })
}

/// Records `text` as what the user has seen of `path`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn mark_design_seen(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    path: String,
    text: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .mark_design_seen(&ws, &slug, &path, text)
        .map_err(err)
}

/// An answer to a question: an option from 0, or none of them with `other`.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnswerInput {
    pub option: Option<u32>,
    pub note: Option<String>,
    pub other: Option<String>,
}

/// What can happen to a question without the agent.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum QuestionAct {
    Decide {
        id: String,
        answer: AnswerInput,
    },
    /// Reopens the last decision.
    Undo,
    /// Moves it to the end of the queue.
    Later {
        id: String,
    },
    /// Opens a decided question to answer it again.
    Reopen {
        id: String,
    },
    /// The user saw what a rewrite changed.
    Seen {
        id: String,
    },
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn act_on_question(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    act: QuestionAct,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let ctx = repo(&window)?;
    let (_, dir) = ctx.design_dir(&ws, &slug).map_err(err)?;
    edit_queue(&dir, |q| match act {
        QuestionAct::Decide { id, answer } => q.decide(
            &id,
            Answer {
                option: answer.option,
                note: answer.note.filter(|n| !n.trim().is_empty()),
                other: answer.other.filter(|o| !o.trim().is_empty()),
                at: crate::review::now_ms() as f64,
            },
        ),
        QuestionAct::Undo => {
            q.undo();
            Ok(())
        }
        QuestionAct::Later { id } => q.later(&id),
        QuestionAct::Reopen { id } => q.reopen(&id),
        QuestionAct::Seen { id } => q.seen(&id),
    })
    .map_err(err)?;
    ctx.design(&ws, &slug).map_err(err)
}

/// Has the agent write the design from its brief, with its open questions.
#[tauri::command]
#[specta::specta]
pub(crate) async fn draft_design(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    on_tool: Channel<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .draft_design(&ws, &slug, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Applies the answer to question `id` to the rest of the queue.
#[tauri::command]
#[specta::specta]
pub(crate) async fn apply_answer(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    id: String,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .apply_answer(&ws, &slug, &id, &runs)
        .await
        .map_err(err)
}

/// A discussion turn on a question, streaming the agent's reply through `on_text`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn discuss_question(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    id: String,
    message: String,
    on_text: Channel<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .discuss_question(&ws, &slug, &id, &message, &runs, |t| {
            on_text.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Rewrites a question after its discussion.
#[tauri::command]
#[specta::specta]
pub(crate) async fn reask_question(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    id: String,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .reask_question(&ws, &slug, &id, &runs)
        .await
        .map_err(err)
}

/// Folds the new decisions into the design.
#[tauri::command]
#[specta::specta]
pub(crate) async fn revise_design(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    on_tool: Channel<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .revise_design(&ws, &slug, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

/// A comment on a passage of one of the design's markdown files.
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommentInput {
    pub path: String,
    pub quote: String,
    pub start: u32,
    pub end: u32,
    pub body: String,
    pub kind: CommentKind,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn add_design_comment(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    comment: CommentInput,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let c = DocComment {
        id: 0,
        path: comment.path,
        quote: comment.quote,
        start: comment.start,
        end: comment.end,
        body: comment.body,
        kind: comment.kind,
        state: CommentState::Queued,
        reply: None,
    };
    repo(&window)?
        .add_design_comment(&ws, &slug, c)
        .map_err(err)
}

/// Changes a comment's text, or deletes it when `body` is null.
#[tauri::command]
#[specta::specta]
pub(crate) async fn edit_design_comment(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    id: u32,
    body: Option<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .edit_design_comment(&ws, &slug, id, body.as_deref())
        .map_err(err)
}

/// Has the agent apply the queued comments in one turn.
#[tauri::command]
#[specta::specta]
pub(crate) async fn apply_design_comments(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    on_tool: Channel<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .apply_design_comments(&ws, &slug, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)
}

/// Answers an ask comment, streaming the reply through `on_text`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn answer_design_comment(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    id: u32,
    on_text: Channel<String>,
) -> CmdResult<DesignDto> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?
        .answer_design_comment(&ws, &slug, id, &runs, |t| {
            on_text.send(t).ok();
        })
        .await
        .map_err(err)
}

/// A design after the agent made a file for it, and the file's path in the design.
#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Made {
    pub design: DesignDto,
    pub path: String,
}

/// Makes a doc, diagram or HTML page for the design.
#[tauri::command]
#[specta::specta]
pub(crate) async fn make_design_artifact(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    kind: ArtifactKind,
    about: String,
    on_tool: Channel<String>,
) -> CmdResult<Made> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let (design, path) = repo(&window)?
        .make_design_artifact(&ws, &slug, kind, &about, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)?;
    Ok(Made { design, path })
}

/// Makes an HTML page from one of the design's markdown or Mermaid files.
#[tauri::command]
#[specta::specta]
pub(crate) async fn make_design_html(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
    source: String,
    on_tool: Channel<String>,
) -> CmdResult<Made> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let (design, path) = repo(&window)?
        .make_design_html(&ws, &slug, &source, &runs, |t| {
            on_tool.send(t).ok();
        })
        .await
        .map_err(err)?;
    Ok(Made { design, path })
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn stop_design(
    window: WebviewWindow,
    runs: State<'_, Runs>,
    workspace: Option<String>,
    slug: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    repo(&window)?.stop_design(&ws, &slug, &runs);
    Ok(())
}

/// Opens one of the design's files in the default browser.
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_design_file(
    window: WebviewWindow,
    workspace: Option<String>,
    slug: String,
    path: String,
) -> CmdResult<()> {
    let ws = super::workspace(&window, workspace.as_deref()).await?;
    let (_, dir) = repo(&window)?.design_dir(&ws, &slug).map_err(err)?;
    let file = inside(&dir, &path).map_err(err)?;
    std::process::Command::new("open")
        .arg(&file)
        .spawn()
        .map_err(|e| format!("can't open {path}: {e}"))?;
    Ok(())
}
