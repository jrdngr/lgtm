//! Designs: C4 maps a terminal agent writes from spec files at a revision, laid over the
//! system map of the code. Each is a named object of the repo with its files and a prompt, and
//! can be generated again at will.

pub mod model;
pub mod prompt;
pub mod rev;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

pub use model::*;
pub use rev::ResolvedDto;

use lgtm_canvas::Canvas;
use lgtm_core::jj::Jj;
use lgtm_core::store::Store;

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
pub struct At {
    pub resolved: ResolvedDto,
    pub all: Vec<String>,
}

/// Where `revision` is. The caller snapshots the working copy first, so an edit to a spec
/// file there counts.
pub async fn at(jj: &Jj, revision: &str) -> Option<At> {
    let resolved = rev::resolve(jj, revision).await.ok()?;
    let all = jj.commit_files(&resolved.commit).await.ok()?.to_vec();
    Some(At { resolved, all })
}

/// A stored design. Designs stored before revisions name a `bookmark`, which becomes the
/// revision.
pub fn parse(data: &str) -> anyhow::Result<DesignDto> {
    let mut v: serde_json::Value = serde_json::from_str(data)?;
    if let Some(o) = v.as_object_mut()
        && !o.contains_key("revision")
        && let Some(b) = o.remove("bookmark")
    {
        o.insert("revision".into(), b);
    }
    Ok(serde_json::from_value(v)?)
}

pub fn load(store: &Store, id: &str) -> anyhow::Result<DesignDto> {
    let data = store
        .design(id)?
        .with_context(|| format!("no design `{id}`"))?;
    parse(&data)
}

pub fn save(store: &Store, d: &DesignDto) -> anyhow::Result<()> {
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

pub fn check_view(view: &str) -> anyhow::Result<()> {
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
