//! The guided review as set_guided_review takes it, and turning it into one the review can
//! trust: chunks only on the review's files, unique ids, and every changed line accounted for.

use std::collections::HashSet;

use rmcp::schemars::{self, JsonSchema};
use serde::Deserialize;

use super::summary::ChunkSummary;
use super::{
    ChunkDto, ChunkRisk, ContextDocDto, DocKind, ExcludeReason, ExcludedDto, GuidedRange, StepDto,
    TestsDto,
};
use crate::diff::RowKind;
use crate::review::build::RowPos;

#[derive(Deserialize, JsonSchema)]
pub struct GuidedReply {
    /// The revset the prompt named.
    pub revset: String,
    /// The base commit the prompt named.
    pub from: String,
    /// The head commit the prompt named.
    pub to: String,
    /// The code a person should read, most important first.
    pub chunks: Vec<ChunkReply>,
    /// Changed code left out, with why.
    pub excluded: Vec<ExcludedReply>,
    /// ADRs and dev logs the change relates to.
    #[serde(default)]
    pub context: Vec<DocReply>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RiskReply {
    High,
    Medium,
    Low,
}

#[derive(Deserialize, JsonSchema)]
pub struct RangeReply {
    /// Repository-relative path.
    pub path: String,
    /// First line on the new side, 1-based.
    pub start: i64,
    /// Last line, inclusive.
    pub end: i64,
}

#[derive(Deserialize, JsonSchema)]
pub struct StepReply {
    /// A sentence or two.
    pub text: String,
    /// The lines it's about.
    pub ranges: Vec<RangeReply>,
}

#[derive(Deserialize, JsonSchema)]
pub struct TestsReply {
    pub passed: i64,
    pub failed: i64,
    /// Changed lines no test exercises.
    #[serde(default)]
    pub uncovered: Vec<RangeReply>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ChunkReply {
    /// A short slug. On an update, the id of the last guided review's chunk about the same code.
    pub id: String,
    /// A few words naming what the code does.
    pub title: String,
    pub risk: RiskReply,
    /// A word or two each, like concurrency or public API.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Fill only the sections that apply.
    #[serde(default)]
    pub summary: ChunkSummary,
    /// The changed lines it's made of, from any of the review's files.
    pub ranges: Vec<RangeReply>,
    /// Unchanged lines in any file to read alongside it.
    #[serde(default)]
    pub related: Vec<RangeReply>,
    /// A walk through it in reading order; empty for a short chunk.
    #[serde(default)]
    pub steps: Vec<StepReply>,
    /// Paths of `context` docs it bears on.
    #[serde(default)]
    pub docs: Vec<String>,
    pub tests: Option<TestsReply>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReasonReply {
    Docs,
    /// Simple and obviously right.
    Simple,
    Boilerplate,
    Generated,
    Tests,
    Other,
}

#[derive(Deserialize, JsonSchema)]
pub struct ExcludedReply {
    /// Repository-relative path.
    pub path: String,
    /// The lines left out; empty for the whole file's changes.
    #[serde(default)]
    pub ranges: Vec<RangeReply>,
    pub reason: ReasonReply,
    /// One short line.
    pub why: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DocKindReply {
    Adr,
    Devlog,
    Other,
}

#[derive(Deserialize, JsonSchema)]
pub struct DocReply {
    /// Repository-relative path.
    pub path: String,
    pub kind: DocKindReply,
    pub title: String,
    /// A sentence or two on what it decides or records that bears on the change.
    pub summary: String,
}

/// A review file's path and its changed lines, as new-side spans.
pub struct FileRows {
    pub path: String,
    pub changed: Vec<(u32, u32)>,
}

impl FileRows {
    /// Runs of changed rows, each from its first to its last new-side line. A removed row sits
    /// at the new line after the last one before it.
    pub fn of(path: &str, rows: &[RowPos]) -> Self {
        let mut changed: Vec<(u32, u32)> = Vec::new();
        let mut last_new = 0;
        let mut run: Option<(u32, u32)> = None;
        for r in rows {
            let at = r.new_line.unwrap_or(last_new + 1);
            if let Some(n) = r.new_line {
                last_new = n;
            }
            if r.kind == RowKind::Context {
                changed.extend(run.take());
                continue;
            }
            run = Some(match run {
                Some((start, end)) => (start, end.max(at)),
                None => (at, at),
            });
        }
        changed.extend(run);
        Self {
            path: path.to_string(),
            changed,
        }
    }
}

fn range(r: RangeReply) -> GuidedRange {
    let start = r.start.clamp(1, u32::MAX as i64) as u32;
    let end = r.end.clamp(start as i64, u32::MAX as i64) as u32;
    GuidedRange {
        path: r.path.trim().to_string(),
        start,
        end,
    }
}

fn overlaps(r: &GuidedRange, path: &str, (start, end): (u32, u32)) -> bool {
    r.path == path && r.start <= end && start <= r.end
}

/// `id` as a slug, made unique among `taken`.
fn unique_id(id: &str, taken: &mut HashSet<String>) -> String {
    let slug: String = id
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let slug = if slug.trim_matches('-').is_empty() { "chunk".to_string() } else { slug };
    let mut out = slug.clone();
    let mut n = 2;
    while !taken.insert(out.clone()) {
        out = format!("{slug}-{n}");
        n += 1;
    }
    out
}

fn risk(r: RiskReply) -> ChunkRisk {
    match r {
        RiskReply::High => ChunkRisk::High,
        RiskReply::Medium => ChunkRisk::Medium,
        RiskReply::Low => ChunkRisk::Low,
    }
}

/// The reply as stored: chunks and excluded parts only on the review's `files`, chunks with
/// no lines left dropped, docs only from `context`, and changed lines nothing names listed as
/// excluded.
pub fn validate(
    reply: GuidedReply,
    files: &[FileRows],
) -> (Vec<ChunkDto>, Vec<ExcludedDto>, Vec<ContextDocDto>) {
    let in_review = |path: &str| files.iter().any(|f| f.path == path);
    let context: Vec<ContextDocDto> = reply
        .context
        .into_iter()
        .map(|d| ContextDocDto {
            path: d.path.trim().to_string(),
            kind: match d.kind {
                DocKindReply::Adr => DocKind::Adr,
                DocKindReply::Devlog => DocKind::Devlog,
                DocKindReply::Other => DocKind::Other,
            },
            title: d.title.trim().to_string(),
            summary: d.summary.trim().to_string(),
        })
        .filter(|d| !d.path.is_empty())
        .collect();
    let mut taken = HashSet::new();
    let chunks: Vec<ChunkDto> = reply
        .chunks
        .into_iter()
        .filter_map(|c| {
            let ranges: Vec<GuidedRange> = c
                .ranges
                .into_iter()
                .map(range)
                .filter(|r| in_review(&r.path))
                .collect();
            if ranges.is_empty() {
                return None;
            }
            Some(ChunkDto {
                id: unique_id(&c.id, &mut taken),
                title: c.title.trim().to_string(),
                risk: risk(c.risk),
                tags: c
                    .tags
                    .into_iter()
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .take(5)
                    .collect(),
                summary: c.summary.cleaned(),
                ranges,
                related: c.related.into_iter().map(range).collect(),
                steps: c
                    .steps
                    .into_iter()
                    .map(|s| StepDto {
                        text: s.text.trim().to_string(),
                        ranges: s.ranges.into_iter().map(range).collect(),
                    })
                    .filter(|s| !s.text.is_empty())
                    .collect(),
                docs: c
                    .docs
                    .into_iter()
                    .map(|d| d.trim().to_string())
                    .filter(|d| context.iter().any(|c| c.path == *d))
                    .collect(),
                tests: c.tests.map(|t| TestsDto {
                    passed: t.passed.clamp(0, u32::MAX as i64) as u32,
                    failed: t.failed.clamp(0, u32::MAX as i64) as u32,
                    uncovered: t.uncovered.into_iter().map(range).collect(),
                }),
                promoted: false,
            })
        })
        .collect();
    let mut excluded: Vec<ExcludedDto> = reply
        .excluded
        .into_iter()
        .filter_map(|e| {
            let path = e.path.trim().to_string();
            let file = files.iter().find(|f| f.path == path)?;
            let mut ranges: Vec<GuidedRange> = e.ranges.into_iter().map(range).collect();
            ranges.retain(|r| r.path == path);
            if ranges.is_empty() {
                ranges = spans(&path, &file.changed);
            }
            Some(ExcludedDto {
                path,
                ranges,
                reason: match e.reason {
                    ReasonReply::Docs => ExcludeReason::Docs,
                    ReasonReply::Simple => ExcludeReason::Simple,
                    ReasonReply::Boilerplate => ExcludeReason::Boilerplate,
                    ReasonReply::Generated => ExcludeReason::Generated,
                    ReasonReply::Tests => ExcludeReason::Tests,
                    ReasonReply::Other => ExcludeReason::Other,
                },
                why: e.why.trim().to_string(),
            })
        })
        .collect();
    for f in files {
        let named = |span: (u32, u32)| {
            chunks.iter().flat_map(|c| &c.ranges).any(|r| overlaps(r, &f.path, span))
                || excluded.iter().flat_map(|e| &e.ranges).any(|r| overlaps(r, &f.path, span))
        };
        let left: Vec<(u32, u32)> = f.changed.iter().copied().filter(|s| !named(*s)).collect();
        if !left.is_empty() {
            excluded.push(ExcludedDto {
                path: f.path.clone(),
                ranges: spans(&f.path, &left),
                reason: ExcludeReason::Unlisted,
                why: String::new(),
            });
        }
    }
    (chunks, excluded, context)
}

fn spans(path: &str, spans: &[(u32, u32)]) -> Vec<GuidedRange> {
    spans
        .iter()
        .map(|&(start, end)| GuidedRange {
            path: path.to_string(),
            start,
            end,
        })
        .collect()
}

/// Excluded code `e` as a chunk at the person's `risk`, with an id none of `chunks` has.
pub fn promoted(e: ExcludedDto, risk: ChunkRisk, chunks: &[ChunkDto]) -> ChunkDto {
    let mut taken: HashSet<String> = chunks.iter().map(|c| c.id.clone()).collect();
    let base = e.path.rsplit('/').next().unwrap_or(&e.path).to_string();
    ChunkDto {
        id: unique_id(&format!("promoted-{base}"), &mut taken),
        title: e.path.clone(),
        risk,
        tags: Vec::new(),
        summary: ChunkSummary::default(),
        ranges: e.ranges,
        related: Vec::new(),
        steps: Vec::new(),
        docs: Vec::new(),
        tests: None,
        promoted: true,
    }
}
