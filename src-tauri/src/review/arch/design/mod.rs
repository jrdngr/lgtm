//! Designs: C4 maps a terminal agent writes from spec files at a bookmark, laid over the
//! system map of the code. Each is a named object of the repo with its files and a prompt, and
//! can be generated again at will.

mod model;
mod prompt;

use std::collections::{BTreeSet, HashSet};

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

pub use model::*;

use crate::ctx::{RepoCtx, now_ms};
use crate::jj::Range;
use crate::store::Store;

/// A design as stored: what to generate it from, and the map once generated.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignDto {
    pub id: String,
    pub name: String,
    pub bookmark: String,
    /// Repo-relative files and folders; a folder takes every file under it.
    pub files: Vec<String>,
    pub prompt: String,
    pub created_at: f64,
    pub map: Option<DesignMapDto>,
}

/// What the create and edit form sends.
#[derive(Clone, Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignInput {
    pub name: String,
    pub bookmark: String,
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
    /// The files its entries cover at the bookmark.
    pub file_count: u32,
}

fn load(store: &Store, id: &str) -> anyhow::Result<DesignDto> {
    let data = store
        .design(id)?
        .with_context(|| format!("no design `{id}`"))?;
    Ok(serde_json::from_str(&data)?)
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
                .filter_map(|d| serde_json::from_str(d).ok())
                .collect()
        };
        let ws = self.workspace(workspace).await?;
        let mut out = Vec::new();
        for design in all {
            let files = match ws.jj.files(&exact(&design.bookmark)).await {
                Ok(paths) => covered(&paths, &design.files),
                Err(_) => vec![],
            };
            let state = match &design.map {
                None => DesignState::Missing,
                Some(m) => {
                    let head = ws.jj.commit_ids(&exact(&design.bookmark)).await;
                    let moved = match head.ok().and_then(|h| h.into_iter().next()) {
                        Some(head) if head != m.spec_commit => {
                            let range = Range {
                                from: Some(m.spec_commit.clone()),
                                to: head,
                            };
                            let set: HashSet<&String> = files.iter().collect();
                            ws.jj
                                .diff(&range)
                                .await
                                .map_or(true, |d| d.iter().any(|f| set.contains(&f.path)))
                        }
                        _ => false,
                    };
                    if moved {
                        DesignState::Stale
                    } else {
                        DesignState::Fresh
                    }
                }
            };
            out.push(DesignItemDto {
                design,
                state,
                file_count: files.len() as u32,
            });
        }
        Ok(out)
    }

    pub fn design(&self, id: &str) -> anyhow::Result<DesignDto> {
        load(&self.store.lock().unwrap(), id)
    }

    /// Creates a design, or with `id` changes one, keeping its map.
    pub fn save_design(&self, id: Option<&str>, input: DesignInput) -> anyhow::Result<DesignDto> {
        if input.name.trim().is_empty() {
            bail!("a design needs a name");
        }
        let store = self.store.lock().unwrap();
        let d = match id {
            Some(id) => DesignDto {
                name: input.name,
                bookmark: input.bookmark,
                files: input.files,
                prompt: input.prompt,
                ..load(&store, id)?
            },
            None => {
                let created_at = now_ms() as f64;
                DesignDto {
                    id: format!("d{}", created_at as u64),
                    name: input.name,
                    bookmark: input.bookmark,
                    files: input.files,
                    prompt: input.prompt,
                    created_at,
                    map: None,
                }
            }
        };
        save(&store, &d)?;
        Ok(d)
    }

    /// Saves a new design an agent asked for, after checking its bookmark and that each entry
    /// names a file or folder there. Folders get a trailing `/`, as the design form writes them.
    pub async fn create_design(
        &self,
        workspace: Option<&str>,
        mut input: DesignInput,
    ) -> anyhow::Result<DesignDto> {
        let all = self
            .bookmark_files(workspace, &input.bookmark)
            .await
            .with_context(|| format!("no bookmark `{}`", input.bookmark))?;
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
            bail!("not at `{}`: {}", input.bookmark, missing.join(", "));
        }
        if input.files.is_empty() {
            bail!("a design needs files");
        }
        self.save_design(None, input)
    }

    pub fn delete_design(&self, id: &str) -> anyhow::Result<()> {
        Ok(self.store.lock().unwrap().delete_design(id)?)
    }

    /// Every file at `bookmark`, for picking a design's files.
    pub async fn bookmark_files(
        &self,
        workspace: Option<&str>,
        bookmark: &str,
    ) -> anyhow::Result<Vec<String>> {
        let ws = self.workspace(workspace).await?;
        ws.jj.files(&exact(bookmark)).await
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
        let spec_commit = ws
            .jj
            .commit_ids(&exact(&design.bookmark))
            .await?
            .into_iter()
            .next()
            .with_context(|| format!("no bookmark `{}`", design.bookmark))?;
        let code_commit = ws.jj.commit_ids("@").await?.into_iter().next();
        let map = self.system_map(workspace).await?;
        let mut d = design;
        d.map = Some(model::validate(raw, &map, spec_commit, code_commit));
        save(&self.store.lock().unwrap(), &d)?;
        Ok(d)
    }
}

/// A revset naming exactly local bookmark `name`.
fn exact(name: &str) -> String {
    format!("bookmarks(exact:{})", serde_json::to_string(name).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::{jj, repo, write};

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
            bookmark: "spec".into(),
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
                    bookmark: "nope".into(),
                    ..input(&["docs/plan.md"])
                }
            )
            .await
            .is_err()
        );
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
