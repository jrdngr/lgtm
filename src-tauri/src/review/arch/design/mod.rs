//! Designs: C4 maps a terminal agent writes from spec files at a revision, laid over the
//! system map of the code. Each is a named object of the repo with its files and a prompt, and
//! can be generated again at will.

mod model;
mod prompt;
mod rev;

use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

pub use model::*;
pub use rev::ResolvedDto;

use crate::canvas::{Canvas, CanvasOp};
use crate::ctx::{RepoCtx, now_ms};
use crate::jj::Jj;
use crate::store::Store;

/// A design as stored: what to generate it from, and the map once generated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignDto {
    pub id: String,
    pub name: String,
    /// The revision holding the spec files: a bookmark's name, else a change id.
    pub revision: String,
    /// Repo-relative files and folders; a folder takes every file under it.
    pub files: Vec<String>,
    pub prompt: String,
    pub created_at: f64,
    pub map: Option<DesignMapDto>,
    #[serde(default)]
    pub canvas: Canvas,
    /// The canvas before the last agent pass, while that pass can be undone.
    #[serde(default)]
    pub canvas_undo: Option<Canvas>,
}

/// What the create and edit form sends.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignInput {
    pub name: String,
    /// Any revset of one commit, or a bookmark name.
    pub revision: String,
    pub files: Vec<String>,
    pub prompt: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DesignState {
    Fresh,
    Stale,
    Missing,
}

/// A design with its state, for lists.
#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignItemDto {
    pub design: DesignDto,
    pub state: DesignState,
    /// What its revision resolves to now; none when it doesn't resolve.
    pub resolved: Option<ResolvedDto>,
    /// The files its entries cover at that commit.
    pub file_count: u32,
}

/// A revision's commit and every file there.
struct At {
    resolved: ResolvedDto,
    all: Vec<String>,
}

/// Where `revision` is. The caller snapshots the working copy first, so an edit to a spec
/// file there counts.
async fn at(jj: &Jj, revision: &str) -> Option<At> {
    let resolved = rev::resolve(jj, revision).await.ok()?;
    let all = jj.commit_files(&resolved.commit).await.ok()?.to_vec();
    Some(At { resolved, all })
}

/// A stored design. Designs stored before revisions name a `bookmark`, which becomes the
/// revision.
fn parse(data: &str) -> anyhow::Result<DesignDto> {
    let mut v: serde_json::Value = serde_json::from_str(data)?;
    if let Some(o) = v.as_object_mut()
        && !o.contains_key("revision")
        && let Some(b) = o.remove("bookmark")
    {
        o.insert("revision".into(), b);
    }
    Ok(serde_json::from_value(v)?)
}

fn load(store: &Store, id: &str) -> anyhow::Result<DesignDto> {
    let data = store
        .design(id)?
        .with_context(|| format!("no design `{id}`"))?;
    parse(&data)
}

fn save(store: &Store, d: &DesignDto) -> anyhow::Result<()> {
    store.save_design(&d.id, &serde_json::to_string(d)?)?;
    Ok(())
}

/// The files at a revision that `entries` name: each entry itself, or every file under it.
pub fn covered(all: &[String], entries: &[String]) -> Vec<String> {
    all.iter()
        .filter(|p| {
            entries.iter().any(|e| {
                let e = e.trim_end_matches('/');
                e.is_empty() || *p == e || p.strip_prefix(e).is_some_and(|r| r.starts_with('/'))
            })
        })
        .cloned()
        .collect()
}

impl RepoCtx {
    pub async fn designs(&self, workspace: Option<&str>) -> anyhow::Result<Vec<DesignItemDto>> {
        let all: Vec<DesignDto> = {
            let store = self.store.lock().unwrap();
            store
                .designs()?
                .iter()
                .filter_map(|d| parse(d).ok())
                .collect()
        };
        let ws = self.workspace(workspace).await?;
        // A snapshot that fails leaves the last one's files to compare.
        ws.jj.snapshot_or_stale().await.ok();
        // Designs often share a revision: resolve and hash each one once.
        let mut by_rev: HashMap<String, Vec<DesignDto>> = HashMap::new();
        let mut order = Vec::new();
        for d in all {
            order.push(d.id.clone());
            by_rev.entry(d.revision.clone()).or_default().push(d);
        }
        let ats = futures::future::join_all(by_rev.keys().map(|r| at(&ws.jj, r))).await;
        let mut items = HashMap::new();
        for ((_, designs), at) in by_rev.into_iter().zip(ats) {
            let covers: Vec<Vec<String>> = designs
                .iter()
                .map(|d| at.as_ref().map_or(vec![], |a| covered(&a.all, &d.files)))
                .collect();
            let hashes = match &at {
                Some(a) => {
                    let mut union: Vec<String> = covers.iter().flatten().cloned().collect();
                    union.sort();
                    union.dedup();
                    ws.jj
                        .file_hashes(&a.resolved.commit, &union)
                        .await
                        .unwrap_or_default()
                }
                None => HashMap::new(),
            };
            for (design, files) in designs.into_iter().zip(covers) {
                let state = match &design.map {
                    _ if files.is_empty() => DesignState::Missing,
                    None => DesignState::Missing,
                    Some(m)
                        if m.files_hash.as_deref() == Some(&rev::files_hash(&files, &hashes)) =>
                    {
                        DesignState::Fresh
                    }
                    Some(_) => DesignState::Stale,
                };
                let item = DesignItemDto {
                    resolved: at.as_ref().map(|a| a.resolved.clone()),
                    file_count: files.len() as u32,
                    design,
                    state,
                };
                items.insert(item.design.id.clone(), item);
            }
        }
        let out = order.iter().filter_map(|id| items.remove(id)).collect();
        Ok(out)
    }

    pub fn design(&self, id: &str) -> anyhow::Result<DesignDto> {
        load(&self.store.lock().unwrap(), id)
    }

    /// Creates a design, or with `id` changes one, keeping its map.
    pub async fn save_design(
        &self,
        workspace: Option<&str>,
        id: Option<&str>,
        mut input: DesignInput,
    ) -> anyhow::Result<DesignDto> {
        if input.name.trim().is_empty() {
            bail!("a design needs a name");
        }
        let ws = self.workspace(workspace).await?;
        input.revision = rev::stored(&rev::resolve(&ws.jj, &input.revision).await?);
        let store = self.store.lock().unwrap();
        let d = match id {
            Some(id) => DesignDto {
                name: input.name,
                revision: input.revision,
                files: input.files,
                prompt: input.prompt,
                ..load(&store, id)?
            },
            None => {
                let created_at = now_ms() as f64;
                DesignDto {
                    id: format!("d{}", created_at as u64),
                    name: input.name,
                    revision: input.revision,
                    files: input.files,
                    prompt: input.prompt,
                    created_at,
                    map: None,
                    canvas: Canvas::default(),
                    canvas_undo: None,
                }
            }
        };
        save(&store, &d)?;
        Ok(d)
    }

    /// Saves a new design an agent asked for, after checking its revision and that each entry
    /// names a file or folder there. Folders get a trailing `/`, as the design form writes them.
    /// Without files it's an empty design to draw on.
    pub async fn create_design(
        &self,
        workspace: Option<&str>,
        mut input: DesignInput,
    ) -> anyhow::Result<DesignDto> {
        let all = self.revision_files(workspace, &input.revision).await?;
        let mut missing = Vec::new();
        for e in &mut input.files {
            let path = e.trim_start_matches("./").trim_end_matches('/').to_string();
            if all.contains(&path) {
                *e = path;
            } else if covered(&all, std::slice::from_ref(&path)).is_empty() {
                missing.push(path);
            } else {
                *e = format!("{path}/");
            }
        }
        if !missing.is_empty() {
            bail!("not at `{}`: {}", input.revision, missing.join(", "));
        }
        self.save_design(workspace, None, input).await
    }

    pub fn delete_design(&self, id: &str) -> anyhow::Result<()> {
        Ok(self.store.lock().unwrap().delete_design(id)?)
    }

    /// Every file at `revision`, for picking a design's files.
    pub async fn revision_files(
        &self,
        workspace: Option<&str>,
        revision: &str,
    ) -> anyhow::Result<Vec<String>> {
        let ws = self.workspace(workspace).await?;
        ws.jj.snapshot_or_stale().await?;
        let r = rev::resolve(&ws.jj, revision).await?;
        Ok(ws.jj.commit_files(&r.commit).await?.to_vec())
    }

    /// Local bookmarks, those on `@`'s stack first.
    pub async fn design_bookmarks(&self, workspace: Option<&str>) -> anyhow::Result<Vec<String>> {
        let ws = self.workspace(workspace).await?;
        let near: Vec<String> = ws
            .jj
            .bookmarks_near()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|b| b.remote.is_empty())
            .map(|b| b.name)
            .collect();
        let all = ws.jj.bookmarks("*").await?;
        let mut seen = BTreeSet::new();
        Ok(near
            .into_iter()
            .chain(all)
            .filter(|b| seen.insert(b.clone()))
            .collect())
    }

    /// Stores the user's canvas for design `id`. A canvas without an agent pass ends the last
    /// one's undo.
    pub fn save_design_canvas(&self, id: &str, canvas: Canvas) -> anyhow::Result<DesignDto> {
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        if canvas.agent.is_none() {
            d.canvas_undo = None;
        }
        d.canvas = canvas;
        save(&store, &d)?;
        Ok(d)
    }

    /// Applies an agent's `ops` to a view of design `id`'s canvas as one undoable pass.
    pub fn edit_design_canvas(
        &self,
        id: &str,
        view: &str,
        ops: Vec<CanvasOp>,
    ) -> anyhow::Result<DesignDto> {
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        check_view(view)?;
        let mut canvas = d.canvas.clone();
        canvas.apply(view, ops)?;
        let mut before = std::mem::replace(&mut d.canvas, canvas);
        before.agent = None;
        d.canvas_undo = Some(before);
        save(&store, &d)?;
        Ok(d)
    }

    /// Puts design `id`'s canvas back as it was before the last agent pass.
    pub fn undo_design_agent(&self, id: &str) -> anyhow::Result<DesignDto> {
        let store = self.store.lock().unwrap();
        let mut d = load(&store, id)?;
        d.canvas = d.canvas_undo.take().context("nothing to undo")?;
        save(&store, &d)?;
        Ok(d)
    }

    /// Stores `reply`, in set_design's format, as design `id`'s map.
    pub async fn save_design_map(
        &self,
        workspace: Option<&str>,
        id: &str,
        reply: serde_json::Value,
    ) -> anyhow::Result<DesignDto> {
        let raw: DesignReply =
            serde_json::from_value(reply).context("the design doesn't match the format")?;
        let design = self.design(id)?;
        let ws = self.workspace(workspace).await?;
        ws.jj.snapshot_or_stale().await.ok();
        let At { resolved, all } = at(&ws.jj, &design.revision)
            .await
            .with_context(|| format!("no revision `{}`", design.revision))?;
        let files = covered(&all, &design.files);
        let hashes = ws.jj.file_hashes(&resolved.commit, &files).await?;
        let code_commit = ws.jj.commit_ids("@").await?.into_iter().next();
        let map = self.system_map(workspace).await?;
        let mut d = design;
        let mut m = model::validate(raw, &map, resolved.commit, code_commit);
        m.files_hash = Some(rev::files_hash(&files, &hashes));
        d.map = Some(m);
        save(&self.store.lock().unwrap(), &d)?;
        Ok(d)
    }
}

fn check_view(view: &str) -> anyhow::Result<()> {
    let ok = matches!(view, "context" | "containers")
        || view
            .strip_prefix("components/")
            .or_else(|| view.strip_prefix("interfaces/"))
            .is_some_and(|rest| !rest.is_empty());
    if !ok {
        bail!(
            "no view `{view}`: use context, containers, components/<container id> or \
             interfaces/<component id>"
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
