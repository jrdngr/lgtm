//! The canvas the user last looked at in each workspace, with what they selected, so an agent
//! told "this" or "tag 3" knows where to look.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::RepoCtx;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum CanvasKind {
    Drawing,
    Design,
}

/// A design part as the view draws it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusPart {
    pub id: String,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasFocus {
    pub kind: CanvasKind,
    /// The drawing's or design's id.
    pub id: String,
    pub view: String,
    /// Selected element ids.
    pub selection: Vec<String>,
    /// A design view's parts; none for a drawing.
    pub parts: Vec<FocusPart>,
}

impl RepoCtx {
    /// Records the canvas workspace `workspace` shows, or with none, that it shows none.
    pub fn set_canvas_focus(&self, workspace: Option<&str>, focus: Option<CanvasFocus>) {
        let mut all = self.canvas_focus.lock().unwrap();
        let key = workspace.unwrap_or_default().to_string();
        match focus {
            Some(f) => all.insert(key, f),
            None => all.remove(&key),
        };
    }

    pub fn canvas_focus(&self, workspace: Option<&str>) -> Option<CanvasFocus> {
        let all = self.canvas_focus.lock().unwrap();
        all.get(workspace.unwrap_or_default()).cloned()
    }
}
