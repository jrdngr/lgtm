//! What a canvas holds. Each view (a drawing's one page, or a design level) has its own
//! positions, shapes, groups, edge styles, notes, arrows and tags; a part's color follows it
//! into every view of its design.

use std::collections::BTreeMap;

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Swatch {
    Red,
    Orange,
    Yellow,
    Green,
    Cyan,
    Blue,
    Purple,
    Pink,
}

/// Who last made or changed an element.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CanvasAuthor {
    #[default]
    User,
    Agent,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
pub struct Pt {
    pub x: i32,
    pub y: i32,
}

/// A labeled frame around parts, shapes and other groups.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasGroup {
    pub id: String,
    pub label: String,
    pub color: Option<Swatch>,
    /// Part and shape ids, which have a `:` (or are `sys`), and group ids, which don't.
    pub members: Vec<String>,
    #[serde(default)]
    pub by: CanvasAuthor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LinePattern {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum LineWeight {
    #[default]
    Thin,
    Thick,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Route {
    #[default]
    Curved,
    Elbow,
}

/// How a styled edge draws. A styled edge shows at rest, with its label.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EdgeStyle {
    pub pattern: LinePattern,
    pub color: Option<Swatch>,
    pub weight: LineWeight,
    pub route: Route,
    /// Where its middle is dragged to, from halfway between its ends' centers.
    pub bend: Option<Pt>,
    #[serde(default)]
    pub by: CanvasAuthor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NoteSize {
    S,
    #[default]
    M,
    L,
}

/// Text on the canvas: plain, or on a sticky note.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasNote {
    pub id: String,
    pub text: String,
    /// Its top left.
    pub at: Pt,
    pub size: NoteSize,
    pub color: Option<Swatch>,
    #[serde(default)]
    pub sticky: bool,
    #[serde(default)]
    pub by: CanvasAuthor,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ShapeKind {
    #[default]
    Rect,
    Ellipse,
    Diamond,
}

/// A box the user or an agent drew, with its text inside.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasShape {
    /// Starts with `s:`.
    pub id: String,
    pub kind: ShapeKind,
    /// Its top left.
    pub at: Pt,
    pub w: i32,
    pub h: i32,
    pub text: String,
    pub color: Option<Swatch>,
    /// Tinted inside, else an outline only.
    pub fill: bool,
    pub pattern: LinePattern,
    #[serde(default)]
    pub by: CanvasAuthor,
}

/// An arrow's end: stuck to a part, shape, group or note, or free at `at`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ArrowEnd {
    /// A part, shape, group or note id; none when the end is free.
    pub target: Option<String>,
    /// Where a free end is, and where an attached one was last drawn.
    #[serde(default)]
    pub at: Pt,
}

const fn yes() -> bool {
    true
}

/// An arrow, or without a head, a line.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasArrow {
    pub id: String,
    pub from: ArrowEnd,
    pub to: ArrowEnd,
    pub color: Option<Swatch>,
    pub pattern: LinePattern,
    #[serde(default = "yes")]
    pub head: bool,
    #[serde(default)]
    pub by: CanvasAuthor,
}

/// One view's drawing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewCanvas {
    /// Set once the user moves a part: every part keeps a position, and a part without one is
    /// new since.
    pub frozen: bool,
    /// Parts' top lefts, by part id.
    pub positions: BTreeMap<String, Pt>,
    pub shapes: Vec<CanvasShape>,
    pub groups: Vec<CanvasGroup>,
    /// By `from>to` part ids.
    pub edges: BTreeMap<String, EdgeStyle>,
    pub notes: Vec<CanvasNote>,
    pub arrows: Vec<CanvasArrow>,
    /// The user's numbered pointers: the element each digit, `0` to `9`, sits on.
    pub tags: BTreeMap<String, String>,
}

/// The last agent pass: how many changes it made and what it touched.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentPass {
    pub changed: u32,
    /// Part, shape, group, note and arrow ids, and edges as `from>to`.
    pub touched: Vec<String>,
    pub view: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct Canvas {
    /// By view: a drawing's is `main`; a design's are `context`, `containers`,
    /// `components/<container id>` and `interfaces/<component id>`.
    pub views: BTreeMap<String, ViewCanvas>,
    /// Part colors, by part id, in every view.
    pub colors: BTreeMap<String, Swatch>,
    pub agent: Option<AgentPass>,
}

/// A part or shape id has a `:`, as `c:api`, `k:api/auth` or `s:3`, or is `sys`, the system.
pub fn is_part(id: &str) -> bool {
    id.contains(':') || id == "sys"
}
