//! The app side of `lgtm_arch::design`.

pub use lgtm_arch::design::*;

pub mod prompt;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use crate::canvas::{Canvas, CanvasOp};
use crate::ctx::{RepoCtx, now_ms};
use crate::events::DesignChanged;
use anyhow::{Context, bail};
use std::collections::{BTreeSet, HashMap};

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
        let d = self.save_design(workspace, None, input).await?;
        self.design_changed(workspace, &d.id);
        Ok(d)
    }

    /// Has `workspace`'s window show design `id`, which an agent changed.
    fn design_changed(&self, workspace: Option<&str>, id: &str) {
        self.notify(DesignChanged {
            id: id.to_string(),
            workspace: workspace.map(String::from),
        });
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
        workspace: Option<&str>,
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
        drop(store);
        self.design_changed(workspace, id);
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
        self.design_changed(workspace, id);
        Ok(d)
    }
}
