//! The architecture review: components and the imports between them at the review's base and
//! head, the user's lens and intent edits over them, rule and agent items to look at, and the
//! Assess pass that scores the change.

pub mod assess;
mod compute;
#[cfg(test)]
mod diff_tests;
mod extract;
mod fix;
mod group;
mod inputs;
mod intents;
mod layers;
mod load;
mod look;
mod model;
mod prompt;
mod reply;
mod resolve;
mod roots;
mod scan;
mod state;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use specta::Type;

/// A source root, like a crate's `src/` or a package root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContainerDto {
    /// The root's repo-relative path, "" for the repo root.
    pub id: String,
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CompStatus {
    /// Only at head.
    Added,
    /// Only at base.
    Removed,
    /// Has a changed file.
    Changed,
    /// Unchanged, but a changed component uses it.
    Upstream,
    /// Unchanged, but uses a changed or removed symbol of a changed component.
    Impacted,
    Unchanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LookSeverity {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileRefDto {
    pub path: String,
    /// The review file, absent for a file the review doesn't change.
    pub file: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComponentDto {
    /// Stable across lens renames: the folder (or root, for loose files) it was grouped from,
    /// or the id of the component it was merged into.
    pub id: String,
    /// The lens name, or the folder's name.
    pub name: String,
    pub container: String,
    /// Files at head, plus removed files.
    pub files: Vec<FileRefDto>,
    pub status: CompStatus,
    /// The first layer that covers it, 1-based.
    pub layer: Option<u32>,
    /// Tagged by an intent.
    pub risk: bool,
    pub trust: bool,
    pub note: Option<String>,
    /// Paths (or `path#symbol`) in it that Move intents say belong elsewhere.
    pub misplaced: Vec<String>,
    /// The worst open Look item on it.
    pub severity: Option<LookSeverity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum EdgeStatus {
    Added,
    Removed,
    Same,
}

/// One import line behind an edge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportDto {
    pub from_path: String,
    pub to_path: String,
    /// 1-based, at head, or at base for a removed import.
    pub line: u32,
}

/// `from` imports from `to`.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EdgeDto {
    pub from: String,
    pub to: String,
    pub status: EdgeStatus,
    /// `from` is impacted through this edge.
    pub impacted: bool,
    pub imports: Vec<ImportDto>,
    /// The Wrong intent on it.
    pub wrong: Option<u32>,
}

/// A new-side line range, or old-side for a removed file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RangeDto {
    pub path: String,
    pub file: Option<u32>,
    pub start: u32,
    pub end: u32,
}

/// One step of the change stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LayerDto {
    /// 1-based.
    pub n: u32,
    pub title: String,
    pub summary: String,
    pub components: Vec<String>,
    pub ranges: Vec<RangeDto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LookSource {
    Rule,
    Agent,
    Security,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LookDto {
    /// Stable across loads of the same review, so checks stick.
    pub key: String,
    pub source: LookSource,
    pub severity: LookSeverity,
    pub title: String,
    pub detail: Option<String>,
    pub component: Option<String>,
    /// An edge, as its `from` and `to` components.
    pub edge: Option<(String, String)>,
    pub path: Option<String>,
    pub line: Option<u32>,
    pub checked: bool,
}

/// What an intent says the code should be.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum IntentKind {
    /// `from` must not import from `to`.
    Wrong { from: String, to: String, reason: Option<String> },
    /// The file, or the symbol in it, belongs in component `to`.
    Move { path: String, symbol: Option<String>, to: String },
    Risk { component: String },
    Trust { component: String },
    Note { component: String, text: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum IntentState {
    /// The review introduces what the intent forbids.
    Broken,
    /// The code doesn't match yet, but the review didn't cause it.
    Open,
    /// The code matches.
    Met,
    /// Risk, Trust and Note, which aren't checked.
    Tag,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IntentDto {
    pub id: u32,
    #[serde(flatten)]
    pub kind: IntentKind,
    pub state: IntentState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RiskLevel {
    Minimal,
    Low,
    Moderate,
    High,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BoundaryDto {
    pub name: String,
    pub components: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentDto {
    /// 1 (risky) to 5 (safe to merge).
    pub score: u8,
    pub risk: RiskLevel,
    pub reason: String,
    pub boundaries: Vec<BoundaryDto>,
    pub created_at: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CountsDto {
    pub changed: u32,
    pub upstream: u32,
    pub impacted: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchDto {
    pub containers: Vec<ContainerDto>,
    pub components: Vec<ComponentDto>,
    pub edges: Vec<EdgeDto>,
    /// Assess's layers once it has run, else one per changed component in dependency order.
    pub layers: Vec<LayerDto>,
    pub look: Vec<LookDto>,
    pub intents: Vec<IntentDto>,
    pub assessment: Option<AssessmentDto>,
    pub counts: CountsDto,
}

/// Two commits to compare instead of the review's base and head.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchRange {
    /// Absent for the root.
    pub from: Option<String>,
    pub to: String,
}

/// A commit in the review that changed the architecture.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryDto {
    pub change_id: String,
    pub commit_id: String,
    /// The commit's parent, for its delta.
    pub parent: Option<String>,
    pub description: String,
    pub components_added: u32,
    pub components_removed: u32,
    pub edges_added: u32,
    pub edges_removed: u32,
}

/// A grouping change that flags nothing.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LensEdit {
    Rename { component: String, name: String },
    Merge { component: String, into: String },
    /// Undoes merges into it and groups its subfolders as their own components.
    Split { component: String },
}

/// The repo's lens, stored whole.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lens {
    pub names: BTreeMap<String, String>,
    /// Component id to the id it was merged into.
    pub merged: BTreeMap<String, String>,
    /// Component ids whose subfolders are components too.
    pub split: Vec<String>,
}
