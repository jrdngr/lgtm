use super::{CmdResult, err, repo};
use crate::frame::Frame;
use crate::review::guided::provenance::PromptDto;
use crate::review::guided::{ChunkRisk, GuidedDto, VerdictSet};

/// The review's last guided review, if it has one.
#[tauri::command]
#[specta::specta]
pub(crate) fn guided(window: Frame, review: u32) -> CmdResult<Option<GuidedDto>> {
    repo(&window)?.guided(review).map_err(err)
}

/// The prompt asking a terminal agent for the review's guided review, or to update it.
#[tauri::command]
#[specta::specta]
pub(crate) fn guided_prompt(window: Frame, review: u32) -> CmdResult<String> {
    repo(&window)?.guided_prompt(review).map_err(err)
}

/// Gives or takes back verdicts on chunks of the review's guided review.
#[tauri::command]
#[specta::specta]
pub(crate) fn set_guided_verdicts(
    window: Frame,
    review: u32,
    sets: Vec<VerdictSet>,
) -> CmdResult<GuidedDto> {
    repo(&window)?
        .set_guided_verdicts(review, sets)
        .map_err(err)
}

/// Makes excluded part `index` a chunk at `risk`.
#[tauri::command]
#[specta::specta]
pub(crate) fn promote_excluded(
    window: Frame,
    review: u32,
    index: u32,
    risk: ChunkRisk,
) -> CmdResult<GuidedDto> {
    repo(&window)?
        .promote_excluded(review, index, risk)
        .map_err(err)
}

/// The terminal agent prompts whose edits wrote `lines`, a chunk's added lines in `paths`.
#[tauri::command]
#[specta::specta]
pub(crate) fn chunk_prompts(
    window: Frame,
    paths: Vec<String>,
    lines: Vec<String>,
) -> CmdResult<Vec<PromptDto>> {
    repo(&window)?.chunk_prompts(&paths, &lines).map_err(err)
}
