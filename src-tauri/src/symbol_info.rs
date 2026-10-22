//! Symbol Information: an agent's notes on a symbol for its callers, kept per definition and
//! shown under its line.

use std::path::Path;

use anyhow::{Context, bail};
use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::search;
use crate::store::Store;
use crate::syntax::SymbolKind;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Caller {
    /// The calling function or other symbol.
    pub name: String,
    /// Workspace-relative path.
    pub path: String,
    /// 1-based line of the call.
    pub line: u32,
}

/// The agent's notes on a symbol, as set_symbol_info takes them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct InfoReply {
    /// The path of the symbol's definition, as the prompt named it.
    pub path: String,
    /// The symbol's name, as the prompt named it.
    pub name: String,
    /// What it does, in a few sentences.
    pub description: String,
    /// Every place that calls or uses it.
    pub callers: Vec<Caller>,
    /// What a caller must know: assumptions, invariants, preconditions, locking, ordering.
    pub assumptions: Vec<String>,
    /// How it fails and what it changes besides its return value.
    #[serde(default)]
    pub errors: Vec<String>,
}

/// What's stored per definition: notes once the agent has written them, and the hash of the
/// code they describe.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Stored {
    pub info: Option<StoredInfo>,
    /// The code's hash when it was asked about or described.
    pub hash: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct StoredInfo {
    pub description: String,
    pub callers: Vec<Caller>,
    pub assumptions: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfoDto {
    pub path: String,
    pub name: String,
    /// The definition's line now, 1-based.
    pub line: u32,
    pub kind: SymbolKind,
    /// Asked for, and the agent hasn't answered yet.
    pub working: bool,
    /// The code changed since the agent described it.
    pub changed: bool,
    pub description: String,
    pub callers: Vec<Caller>,
    pub assumptions: Vec<String>,
    pub errors: Vec<String>,
}

/// FNV-1a, so a stored hash means the same thing in every build.
fn hash(code: &str) -> u64 {
    code.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The definition of `name`: in `path` when it's defined there, else the first one in the
/// workspace. Returns its path, line, kind and code.
pub fn locate(
    root: &Path,
    name: &str,
    path: Option<&str>,
) -> anyhow::Result<(String, u32, SymbolKind, String)> {
    let at = |rel: &str| {
        let text = search::read(&crate::files::resolve(root, rel).ok()?)?;
        search::definition(rel, &text, name).map(|(l, k, c)| (rel.to_string(), l, k, c))
    };
    if let Some(found) = path.and_then(at) {
        return Ok(found);
    }
    let defs = search::references(root, name).hits;
    defs.iter()
        .filter(|h| h.def)
        .find_map(|h| at(&h.path))
        .with_context(|| format!("No definition of `{name}` found"))
}

/// Marks `name` at `path` as asked for and returns the prompt for the agent.
pub fn request(store: &Store, root: &Path, name: &str, path: Option<&str>) -> anyhow::Result<String> {
    let (path, line, _, code) = locate(root, name, path)?;
    let mut s = load(store, &path, name)?.unwrap_or_default();
    s.hash = hash(&code);
    save(store, &path, name, &s, true)?;
    Ok(prompt(&path, line, name))
}

fn prompt(path: &str, line: u32, name: &str) -> String {
    format!(
        "Write up `{name}`, defined at {path}:{line}, for someone about to call or change it. \
         Read its code and search the workspace for every caller. Then call lgtm's \
         set_symbol_info tool with path `{path}` and name `{name}`: a short description, every \
         caller with its path and line, the assumptions and invariants a caller must keep \
         (preconditions, locking, ordering, ownership, units, what it never does), and how it \
         fails or what side effects it has. Be concrete and brief; don't repeat the signature."
    )
}

/// Stores the agent's notes on `reply.name`, keeping the code hash from when it was asked.
pub fn set(store: &Store, root: &Path, reply: InfoReply) -> anyhow::Result<()> {
    let Ok((path, _, _, code)) = locate(root, &reply.name, Some(&reply.path)) else {
        bail!("`{}` isn't defined in {}", reply.name, reply.path);
    };
    if path != reply.path {
        bail!("`{}` isn't defined in {}; it's in {path}", reply.name, reply.path);
    }
    let s = Stored {
        info: Some(StoredInfo {
            description: reply.description,
            callers: reply.callers,
            assumptions: reply.assumptions,
            errors: reply.errors,
        }),
        hash: hash(&code),
    };
    save(store, &path, &reply.name, &s, false)
}

/// The notes on symbols defined in `paths`, placed on their definitions as they are now.
/// Notes whose symbol is gone are left out.
pub fn infos(store: &Store, root: &Path, paths: &[String]) -> anyhow::Result<Vec<SymbolInfoDto>> {
    let mut out = Vec::new();
    for path in paths {
        let rows = store.symbol_infos(path)?;
        if rows.is_empty() {
            continue;
        }
        let Ok(abs) = crate::files::resolve(root, path) else {
            continue;
        };
        let Some(text) = search::read(&abs) else {
            continue;
        };
        for (name, data, working) in rows {
            let Ok(s) = serde_json::from_str::<Stored>(&data) else {
                continue;
            };
            let Some((line, kind, code)) = search::definition(path, &text, &name) else {
                continue;
            };
            let info = s.info.clone();
            out.push(SymbolInfoDto {
                path: path.clone(),
                name,
                line,
                kind,
                working,
                changed: info.is_some() && !working && s.hash != hash(&code),
                description: info.as_ref().map(|i| i.description.clone()).unwrap_or_default(),
                callers: info.as_ref().map(|i| i.callers.clone()).unwrap_or_default(),
                assumptions: info.as_ref().map(|i| i.assumptions.clone()).unwrap_or_default(),
                errors: info.map(|i| i.errors).unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

fn load(store: &Store, path: &str, name: &str) -> anyhow::Result<Option<Stored>> {
    Ok(store
        .symbol_infos(path)?
        .into_iter()
        .find(|(n, ..)| n == name)
        .and_then(|(_, data, _)| serde_json::from_str(&data).ok()))
}

fn save(store: &Store, path: &str, name: &str, s: &Stored, working: bool) -> anyhow::Result<()> {
    store.save_symbol_info(path, name, &serde_json::to_string(s)?, working)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(path: &str, name: &str) -> InfoReply {
        InfoReply {
            path: path.into(),
            name: name.into(),
            description: "Adds.".into(),
            callers: vec![Caller { name: "main".into(), path: "b.rs".into(), line: 2 }],
            assumptions: vec!["Never overflows.".into()],
            errors: vec![],
        }
    }

    #[test]
    fn asked_then_answered_then_changed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.rs"), "/// Docs.\nfn add() {}\n").unwrap();
        std::fs::write(root.join("b.rs"), "fn main() {\n    add();\n}\n").unwrap();
        let store = Store::in_memory();
        let paths = ["a.rs".to_string()];

        let prompt = request(&store, root, "add", Some("b.rs")).unwrap();
        assert!(prompt.contains("a.rs:2"), "{prompt}");
        let got = infos(&store, root, &paths).unwrap();
        assert_eq!((got[0].line, got[0].working, got[0].changed), (2, true, false));

        set(&store, root, reply("a.rs", "add")).unwrap();
        let got = &infos(&store, root, &paths).unwrap()[0];
        assert_eq!((got.working, got.changed, got.description.as_str()), (false, false, "Adds."));

        std::fs::write(root.join("a.rs"), "\n/// Docs.\nfn add() { 1; }\n").unwrap();
        let got = &infos(&store, root, &paths).unwrap()[0];
        assert_eq!((got.line, got.changed), (3, true));
        assert!(set(&store, root, reply("b.rs", "add")).is_err());
    }
}
