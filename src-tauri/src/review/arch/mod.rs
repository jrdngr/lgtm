//! The architecture review: components and the imports between them at the review's base and
//! head, rule and agent items to look at, and the assessment that scores the change.

pub mod assess;
mod compute;
pub mod describe;
pub mod design;
#[cfg(test)]
mod diff_tests;
pub mod edge;
pub mod explore;
mod extract;
mod group;
pub(crate) mod history;
mod inputs;
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
pub mod symbols;
pub mod system;
#[cfg(test)]
mod tests;
pub mod tour;
mod tour_prompt;
pub mod view;

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
    /// The folder (or root, for loose files) it was grouped from.
    pub id: String,
    /// The folder's name.
    pub name: String,
    pub container: String,
    /// Files at head, plus removed files.
    pub files: Vec<FileRefDto>,
    pub status: CompStatus,
    /// The first layer that covers it, 1-based.
    pub layer: Option<u32>,
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
    /// Whether the revset has moved to other commits since.
    #[serde(default)]
    pub stale: bool,
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
