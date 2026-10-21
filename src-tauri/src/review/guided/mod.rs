//! The guided review: an agent picks the review's code a person should read, groups it into
//! chunks by concern across files, rates each, explains it and walks through it. The person
//! gives each chunk a verdict.

mod prompt;
pub mod provenance;
pub mod reply;
#[cfg(test)]
mod tests;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::{RepoCtx, now_ms};
use crate::jj::Range;
use crate::review::arch::assess::Commits;
use crate::store::Store;
use reply::GuidedReply;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChunkRisk {
    High,
    Medium,
    Low,
}

/// New-side lines `start` to `end` of `path`, 1-based and inclusive.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GuidedRange {
    pub path: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StepDto {
    pub text: String,
    pub ranges: Vec<GuidedRange>,
}

/// What the agent found running the tests that cover a chunk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestsDto {
    pub passed: u32,
    pub failed: u32,
    /// Changed lines no test exercises.
    pub uncovered: Vec<GuidedRange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChunkDto {
    /// Kept across updates of the guided review, so verdicts follow the chunk.
    pub id: String,
    pub title: String,
    pub risk: ChunkRisk,
    pub tags: Vec<String>,
    /// Markdown.
    pub writeup: String,
    /// The changed lines it's made of, in the review's files.
    pub ranges: Vec<GuidedRange>,
    /// Unchanged lines read alongside it, in any file.
    pub related: Vec<GuidedRange>,
    pub steps: Vec<StepDto>,
    /// Paths of the context docs it bears on.
    pub docs: Vec<String>,
    pub tests: Option<TestsDto>,
    /// The person made it from excluded code and picked its risk.
    pub promoted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExcludeReason {
    Docs,
    /// Simple and obviously right.
    Simple,
    Boilerplate,
    Generated,
    Tests,
    Other,
    /// Changed code the agent neither put in a chunk nor excluded.
    Unlisted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExcludedDto {
    pub path: String,
    pub ranges: Vec<GuidedRange>,
    pub reason: ExcludeReason,
    /// The agent's one line on why.
    pub why: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DocKind {
    Adr,
    Devlog,
    Other,
}

/// An ADR, dev log or other doc the change relates to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextDocDto {
    pub path: String,
    pub kind: DocKind,
    pub title: String,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChunkVerdict {
    Approved,
    /// Sent back to the agent with a note.
    Change,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VerdictDto {
    pub verdict: ChunkVerdict,
    /// The chunk's code when the verdict was given, to show what changed since.
    pub text: String,
    /// What the person asked for, on a Change.
    pub note: String,
    pub at: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GuidedDto {
    pub chunks: Vec<ChunkDto>,
    pub excluded: Vec<ExcludedDto>,
    pub context: Vec<ContextDocDto>,
    /// By chunk id.
    pub verdicts: Vec<(String, VerdictDto)>,
    pub created_at: f64,
    /// It was made for other commits than the review shows now.
    pub stale: bool,
}

/// A guided review as stored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Guided {
    pub chunks: Vec<ChunkDto>,
    pub excluded: Vec<ExcludedDto>,
    pub context: Vec<ContextDocDto>,
    pub created_at: f64,
    /// The commits the revset resolved to when it was made.
    pub commits: Commits,
}

/// A verdict to give a chunk, or `None` to take it back.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VerdictSet {
    pub chunk: String,
    pub verdict: Option<ChunkVerdict>,
    pub text: String,
    pub note: String,
}

fn load(store: &Store, ws: &str, revset: &str) -> anyhow::Result<Option<Guided>> {
    let Some(data) = store.guided(ws, revset)? else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_str(&data)?))
}

fn save(store: &Store, ws: &str, revset: &str, g: &Guided) -> anyhow::Result<()> {
    Ok(store.save_guided(ws, revset, &serde_json::to_string(g)?)?)
}

fn dto(store: &Store, ws: &str, revset: &str, g: Guided, now: &Range) -> anyhow::Result<GuidedDto> {
    let verdicts = store
        .guided_verdicts(ws, revset)?
        .into_iter()
        .filter_map(|(chunk, data)| Some((chunk, serde_json::from_str(&data).ok()?)))
        .collect();
    Ok(GuidedDto {
        stale: g.commits != Commits::from(now),
        chunks: g.chunks,
        excluded: g.excluded,
        context: g.context,
        verdicts,
        created_at: g.created_at,
    })
}

/// A range's base as the prompt names it.
fn base(r: &Range) -> &str {
    r.from.as_deref().unwrap_or("root()")
}

impl RepoCtx {
    /// The review's workspace key, revset, whole range and changed files with their rows.
    fn guided_review(&self, review: u32) -> anyhow::Result<(String, String, Range, Vec<reply::FileRows>)> {
        let r = self.review(review)?;
        let r = r.lock().unwrap();
        let files = r
            .files
            .iter()
            .map(|f| reply::FileRows::of(&f.path, &f.rows))
            .collect();
        Ok((r.ws.key().to_string(), r.revset.clone(), r.whole.clone(), files))
    }

    /// The review's last guided review, if it has one.
    pub fn guided(&self, review: u32) -> anyhow::Result<Option<GuidedDto>> {
        let (ws, revset, whole, _) = self.guided_review(review)?;
        let store = self.store.lock().unwrap();
        load(&store, &ws, &revset)?
            .map(|g| dto(&store, &ws, &revset, g, &whole))
            .transpose()
    }

    /// The prompt asking a terminal agent for the review's guided review, or to update the
    /// last one.
    pub fn guided_prompt(&self, review: u32) -> anyhow::Result<String> {
        let (ws, revset, whole, _) = self.guided_review(review)?;
        let last = load(&self.store.lock().unwrap(), &ws, &revset)?;
        let text = crate::prompts::load_for(crate::prompts::Prompt::Guided, &self.root)?;
        Ok(prompt::text(&revset, base(&whole), &whole.to, last.as_ref(), &text))
    }

    /// Stores `reply` as review `review`'s guided review, replacing the last one. It must be
    /// about the revset and commits the review shows, as the prompt named them. Returns how
    /// many chunks and excluded parts it kept.
    pub fn save_guided(
        &self,
        review: u32,
        revset: &str,
        (from, to): (&str, &str),
        reply: GuidedReply,
    ) -> anyhow::Result<(usize, usize)> {
        let (ws, shown, whole, files) = self.guided_review(review)?;
        ensure!(
            shown == revset,
            "lgtm's review now shows `{shown}`, not `{revset}`, so the guided review wasn't stored"
        );
        ensure!(
            (from, to) == (base(&whole), whole.to.as_str()),
            "lgtm's review of `{revset}` moved to other commits since the prompt, so the guided \
             review wasn't stored; ask for it again"
        );
        let (chunks, excluded, context) = reply::validate(reply, &files);
        let counts = (chunks.len(), excluded.len());
        let g = Guided {
            chunks,
            excluded,
            context,
            created_at: now_ms() as f64,
            commits: (&whole).into(),
        };
        let mut store = self.store.lock().unwrap();
        save(&store, &ws, revset, &g)?;
        let ids: Vec<&str> = g.chunks.iter().map(|c| c.id.as_str()).collect();
        store.keep_guided_verdicts(&ws, revset, &ids)?;
        Ok(counts)
    }

    /// Gives or takes back verdicts on chunks of the review's guided review.
    pub fn set_guided_verdicts(&self, review: u32, sets: Vec<VerdictSet>) -> anyhow::Result<GuidedDto> {
        let (ws, revset, whole, _) = self.guided_review(review)?;
        let mut store = self.store.lock().unwrap();
        let g = load(&store, &ws, &revset)?.ok_or_else(|| anyhow::anyhow!("no guided review"))?;
        let at = now_ms() as f64;
        let rows = sets
            .into_iter()
            .filter(|s| g.chunks.iter().any(|c| c.id == s.chunk))
            .map(|s| {
                let data = s.verdict.map(|verdict| {
                    let v = VerdictDto {
                        verdict,
                        text: s.text,
                        note: s.note,
                        at,
                    };
                    serde_json::to_string(&v).unwrap_or_default()
                });
                (s.chunk, data)
            })
            .collect::<Vec<_>>();
        store.set_guided_verdicts(&ws, &revset, &rows)?;
        dto(&store, &ws, &revset, g, &whole)
    }

    /// Makes excluded part `index` a chunk at the risk the person picked.
    pub fn promote_excluded(
        &self,
        review: u32,
        index: u32,
        risk: ChunkRisk,
    ) -> anyhow::Result<GuidedDto> {
        let (ws, revset, whole, _) = self.guided_review(review)?;
        let store = self.store.lock().unwrap();
        let mut g =
            load(&store, &ws, &revset)?.ok_or_else(|| anyhow::anyhow!("no guided review"))?;
        let index = index as usize;
        ensure!(index < g.excluded.len(), "that excluded code is gone");
        let e = g.excluded.remove(index);
        g.chunks.push(reply::promoted(e, risk, &g.chunks));
        save(&store, &ws, &revset, &g)?;
        dto(&store, &ws, &revset, g, &whole)
    }
}
