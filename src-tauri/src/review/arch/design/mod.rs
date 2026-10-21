//! Designs: C4 maps a terminal agent writes from spec files at a revision, laid over the
//! system map of the code. Each is a named object of the repo with its files and a prompt, and
//! can be generated again at will.

mod canvas;
mod model;
mod prompt;
mod rev;

use std::collections::{BTreeSet, HashMap};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

pub use canvas::*;
pub use model::*;
pub use rev::ResolvedDto;

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
    pub canvas: DesignCanvas,
    /// The canvas before the last agent pass, while that pass can be undone.
    #[serde(default)]
    pub canvas_undo: Option<DesignCanvas>,
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

/// Snapshots the working copy first, so an edit to a spec file there counts.
async fn at(jj: &Jj, revision: &str) -> Option<At> {
    jj.snapshot_or_stale().await.ok()?;
    let resolved = rev::resolve(jj, revision).await.ok()?;
    let all = jj.files(&resolved.commit).await.ok()?;
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
        // Designs often share a revision: resolve and hash each one once.
        let mut by_rev: HashMap<String, Vec<DesignDto>> = HashMap::new();
        let mut order = Vec::new();
        for d in all {
            order.push(d.id.clone());
            by_rev.entry(d.revision.clone()).or_default().push(d);
        }
        let mut items = HashMap::new();
        for (revision, designs) in by_rev {
            let at = at(&ws.jj, &revision).await;
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
                    canvas: DesignCanvas::default(),
                    canvas_undo: None,
                }
            }
        };
        save(&store, &d)?;
        Ok(d)
    }

    /// Saves a new design an agent asked for, after checking its revision and that each entry
    /// names a file or folder there. Folders get a trailing `/`, as the design form writes them.
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
        if input.files.is_empty() {
            bail!("a design needs files");
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
        ws.jj.files(&r.commit).await
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
    pub fn save_design_canvas(&self, id: &str, canvas: DesignCanvas) -> anyhow::Result<DesignDto> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::{jj, repo, write};

    fn states(items: &[DesignItemDto]) -> Vec<(&str, DesignState)> {
        items
            .iter()
            .map(|i| (i.design.name.as_str(), i.state))
            .collect()
    }

    #[tokio::test]
    async fn a_design_goes_stale_only_when_its_own_files_change_at_its_revision() {
        let (dir, ctx) = repo();
        let d = dir.path();
        std::fs::create_dir_all(d.join("docs")).unwrap();
        write(d, "docs/phase1.md", "one\n");
        write(d, "docs/phase2.md", "two\n");
        write(d, "notes.txt", "x\n");
        let design = |name: &str, file: &str| DesignInput {
            name: name.into(),
            revision: "@".into(),
            files: vec![file.into()],
            prompt: String::new(),
        };
        let empty = serde_json::json!({"summary": "", "people": [], "externals": [],
            "containers": [], "components": [], "relations": [], "differences": []});
        let one = ctx
            .create_design(None, design("One", "docs/phase1.md"))
            .await
            .unwrap();
        let two = ctx
            .create_design(None, design("Two", "docs/phase2.md"))
            .await
            .unwrap();
        assert_ne!(one.revision, "@", "stores the change id, not @");
        assert_eq!(
            states(&ctx.designs(None).await.unwrap()),
            [("One", DesignState::Missing), ("Two", DesignState::Missing)]
        );
        for id in [&one.id, &two.id] {
            ctx.save_design_map(None, id, empty.clone()).await.unwrap();
        }
        let fresh = [("One", DesignState::Fresh), ("Two", DesignState::Fresh)];
        assert_eq!(states(&ctx.designs(None).await.unwrap()), fresh);

        // An unrelated file amended into the revision, a new description, and @ moving on.
        write(d, "notes.txt", "y\n");
        jj(d, &["describe", "-m", "specs"]);
        jj(d, &["new"]);
        assert_eq!(states(&ctx.designs(None).await.unwrap()), fresh);

        jj(d, &["edit", "@-"]);
        write(d, "docs/phase1.md", "one, changed\n");
        assert_eq!(
            states(&ctx.designs(None).await.unwrap()),
            [("One", DesignState::Stale), ("Two", DesignState::Fresh)]
        );
        let listed = ctx.designs(None).await.unwrap();
        assert_eq!(listed[0].resolved.as_ref().unwrap().description, "specs");
    }

    #[tokio::test]
    async fn create_design_marks_folders_and_refuses_paths_not_at_the_bookmark() {
        let (dir, ctx) = repo();
        let d = dir.path();
        std::fs::create_dir_all(d.join("docs/notify")).unwrap();
        write(d, "docs/notify/a.md", "# A\n");
        write(d, "docs/plan.md", "# Plan\n");
        jj(d, &["bookmark", "create", "spec", "-r", "@"]);
        let input = |files: &[&str]| DesignInput {
            name: "Notify".into(),
            revision: "spec".into(),
            files: files.iter().map(|f| f.to_string()).collect(),
            prompt: String::new(),
        };

        let made = ctx
            .create_design(None, input(&["./docs/notify", "docs/plan.md"]))
            .await
            .unwrap();
        assert_eq!(made.files, ["docs/notify/", "docs/plan.md"]);
        assert_eq!(ctx.design(&made.id).unwrap().name, "Notify");

        let err = ctx
            .create_design(None, input(&["docs/notify/", "docs/gone.md"]))
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), "not at `spec`: docs/gone.md");
        assert!(
            ctx.create_design(
                None,
                DesignInput {
                    revision: "nope".into(),
                    ..input(&["docs/plan.md"])
                }
            )
            .await
            .is_err()
        );
    }

    #[test]
    fn a_design_stored_with_a_bookmark_loads_with_it_as_its_revision() {
        let d = parse(
            r#"{"id":"d1","name":"N","bookmark":"spec","files":[],"prompt":"","createdAt":0,"map":null}"#,
        )
        .unwrap();
        assert_eq!(d.revision, "spec");
    }

    #[test]
    fn covered_takes_files_and_everything_under_folders() {
        let all: Vec<String> = [
            "docs/design/a.md",
            "docs/design/b/c.md",
            "docs/designs.md",
            "x.md",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            covered(&all, &["docs/design/".into(), "x.md".into()]),
            ["docs/design/a.md", "docs/design/b/c.md", "x.md"]
        );
    }
}
