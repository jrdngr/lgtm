//! Syntax nodes for the editor's modal keys. Ranges are UTF-8 bytes of `text`, the end past
//! the last.

use serde::Deserialize;
use specta::Type;

use crate::syntax::nodes;

/// What the modal keys ask the syntax tree for.
#[derive(Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SyntaxOp {
    Grow,
    Shrink,
    InFunction,
    AroundFunction,
}

/// The node `op` finds for bytes `start..end` of `text`, a file at `path`.
#[tauri::command]
#[specta::specta]
pub(crate) async fn syntax_node(
    path: String,
    text: String,
    start: u32,
    end: u32,
    op: SyntaxOp,
) -> Option<(u32, u32)> {
    let (s, e) = (start as usize, end as usize);
    let found = match op {
        SyntaxOp::Grow => nodes::grow(&path, &text, s, e),
        SyntaxOp::Shrink => nodes::shrink(&path, &text, s, e),
        SyntaxOp::InFunction => nodes::function(&path, &text, s, e, false),
        SyntaxOp::AroundFunction => nodes::function(&path, &text, s, e, true),
    };
    found.map(|(s, e)| (s as u32, e as u32))
}

/// The lines folds can cover, as start and end lines from 0, or none without a grammar.
#[tauri::command]
#[specta::specta]
pub(crate) async fn fold_spans(path: String, text: String) -> Option<Vec<(u32, u32)>> {
    nodes::fold_spans(&path, &text).map(|spans| {
        spans
            .into_iter()
            .map(|(s, e)| (s as u32, e as u32))
            .collect()
    })
}
