//! Symbol Information: an agent's notes on a symbol for its callers, kept per workspace and
//! definition and shown under its line. A file's definitions of one name are told apart by
//! their order in it.

use std::path::Path;

use anyhow::{Context, bail};
use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::search::{self, Definition};
use crate::store::{InfoKey, Store};
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
    /// The definition's line, as the prompt named it, for a file with several of the name.
    #[serde(default)]
    pub line: Option<u32>,
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
    /// Which of the file's definitions of `name` it is, from 0.
    pub nth: u32,
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

/// A definition asked about.
pub struct Located {
    pub path: String,
    /// Which of the file's definitions of the name it is, from 0.
    pub nth: u32,
    pub line: u32,
}

/// The definitions of `name` in `rel` under `root`.
fn defs_at(root: &Path, rel: &str, name: &str) -> Vec<Definition> {
    let Some(text) = crate::files::resolve(root, rel)
        .ok()
        .and_then(|a| search::read(&a))
    else {
        return vec![];
    };
    search::definitions(rel, &text, name)
}

/// The definition `line` is in, else the one named nearest it, else the first.
fn pick(defs: &[Definition], line: Option<u32>) -> Option<usize> {
    let Some(line) = line else {
        return (!defs.is_empty()).then_some(0);
    };
    defs.iter()
        .position(|d| (d.start..=d.end).contains(&line))
        .or_else(|| (0..defs.len()).min_by_key(|&i| defs[i].line.abs_diff(line)))
}

fn located(path: &str, defs: Vec<Definition>, i: usize) -> Located {
    let d = defs.into_iter().nth(i).expect("picked from these");
    Located {
        path: path.to_string(),
        nth: i as u32,
        line: d.line,
    }
}

/// The definition of `name`: in `path` when it's defined there, the one at `line` when there
/// are several, else the first one in the workspace.
pub fn locate(
    root: &Path,
    name: &str,
    path: Option<&str>,
    line: Option<u32>,
) -> anyhow::Result<Located> {
    if let Some(path) = path {
        let defs = defs_at(root, path, name);
        if let Some(i) = pick(&defs, line) {
            return Ok(located(path, defs, i));
        }
    }
    let hits = search::references(root, name).hits;
    hits.iter()
        .filter(|h| h.def)
        .find_map(|h| {
            let defs = defs_at(root, &h.path, name);
            (!defs.is_empty()).then(|| located(&h.path, defs, 0))
        })
        .with_context(|| format!("No definition of `{name}` found"))
}

/// The prompt asking the agent about `d`.
pub fn request(d: &Located, name: &str) -> String {
    prompt(&d.path, d.line, name)
}

/// Marks definition `nth` of `name` in `path` as asked about, once the prompt went out.
pub fn asked(
    store: &Store,
    workspace: &str,
    root: &Path,
    path: &str,
    name: &str,
    nth: u32,
) -> anyhow::Result<()> {
    let defs = defs_at(root, path, name);
    let d = defs
        .get(nth as usize)
        .with_context(|| format!("`{name}` isn't defined in {path} any more"))?;
    let key = InfoKey {
        workspace,
        path,
        name,
        nth,
    };
    let mut s = load(store, &key)?.unwrap_or_default();
    s.hash = hash(&d.code);
    save(store, &key, &s, true)
}

fn prompt(path: &str, line: u32, name: &str) -> String {
    format!(
        "Write up `{name}`, defined at {path}:{line}, for someone about to call or change it. \
         Read its code and search the workspace for every caller. Then call lgtm's \
         set_symbol_info tool with path `{path}`, name `{name}` and line {line}: a short \
         description, every \
         caller with its path and line, the assumptions and invariants a caller must keep \
         (preconditions, locking, ordering, ownership, units, what it never does), and how it \
         fails or what side effects it has. Be concrete and brief; don't repeat the signature."
    )
}

/// Stores the agent's notes on `reply.name`, keeping the code hash from when it was asked.
pub fn set(store: &Store, workspace: &str, root: &Path, reply: InfoReply) -> anyhow::Result<()> {
    let defs = defs_at(root, &reply.path, &reply.name);
    let Some(i) = pick(&defs, reply.line) else {
        if let Ok(d) = locate(root, &reply.name, None, None) {
            bail!(
                "`{}` isn't defined in {}; it's in {}",
                reply.name,
                reply.path,
                d.path
            );
        }
        bail!("`{}` isn't defined in {}", reply.name, reply.path);
    };
    let s = Stored {
        info: Some(StoredInfo {
            description: reply.description,
            callers: reply.callers,
            assumptions: reply.assumptions,
            errors: reply.errors,
        }),
        hash: hash(&defs[i].code),
    };
    let key = InfoKey {
        workspace,
        path: &reply.path,
        name: &reply.name,
        nth: i as u32,
    };
    save(store, &key, &s, false)
}

/// The notes on symbols defined in `paths`, placed on their definitions as they are now.
/// Notes whose symbol is gone are left out.
pub fn infos(
    store: &Store,
    workspace: &str,
    root: &Path,
    paths: &[String],
) -> anyhow::Result<Vec<SymbolInfoDto>> {
    let mut out = Vec::new();
    for path in paths {
        let rows = store.symbol_infos(workspace, path)?;
        if rows.is_empty() {
            continue;
        }
        let Ok(abs) = crate::files::resolve(root, path) else {
            continue;
        };
        let Some(text) = search::read(&abs) else {
            continue;
        };
        let mut defs: std::collections::HashMap<String, Vec<Definition>> = Default::default();
        for (name, nth, data, working) in rows {
            let Ok(s) = serde_json::from_str::<Stored>(&data) else {
                continue;
            };
            let named = defs
                .entry(name.clone())
                .or_insert_with(|| search::definitions(path, &text, &name));
            let Some(d) = named.get(nth as usize) else {
                continue;
            };
            let info = s.info.clone();
            out.push(SymbolInfoDto {
                path: path.clone(),
                name,
                nth,
                line: d.line,
                kind: d.kind,
                working,
                changed: info.is_some() && !working && s.hash != hash(&d.code),
                description: info
                    .as_ref()
                    .map(|i| i.description.clone())
                    .unwrap_or_default(),
                callers: info.as_ref().map(|i| i.callers.clone()).unwrap_or_default(),
                assumptions: info
                    .as_ref()
                    .map(|i| i.assumptions.clone())
                    .unwrap_or_default(),
                errors: info.map(|i| i.errors).unwrap_or_default(),
            });
        }
    }
    Ok(out)
}

fn load(store: &Store, k: &InfoKey) -> anyhow::Result<Option<Stored>> {
    Ok(store
        .symbol_infos(k.workspace, k.path)?
        .into_iter()
        .find(|(n, nth, ..)| n == k.name && *nth == k.nth)
        .and_then(|(_, _, data, _)| serde_json::from_str(&data).ok()))
}

fn save(store: &Store, k: &InfoKey, s: &Stored, working: bool) -> anyhow::Result<()> {
    store.save_symbol_info(k, &serde_json::to_string(s)?, working)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(path: &str, name: &str, line: Option<u32>) -> InfoReply {
        InfoReply {
            path: path.into(),
            name: name.into(),
            line,
            description: "Adds.".into(),
            callers: vec![Caller {
                name: "main".into(),
                path: "b.rs".into(),
                line: 2,
            }],
            assumptions: vec!["Never overflows.".into()],
            errors: vec![],
        }
    }

    fn ask(
        store: &Store,
        ws: &str,
        root: &Path,
        name: &str,
        path: &str,
        line: Option<u32>,
    ) -> String {
        let d = locate(root, name, Some(path), line).unwrap();
        asked(store, ws, root, &d.path, name, d.nth).unwrap();
        request(&d, name)
    }

    #[test]
    fn asked_then_answered_then_changed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("a.rs"), "/// Docs.\nfn add() {}\n").unwrap();
        std::fs::write(root.join("b.rs"), "fn main() {\n    add();\n}\n").unwrap();
        let store = Store::in_memory();
        let paths = ["a.rs".to_string()];

        let prompt = ask(&store, "", root, "add", "b.rs", None);
        assert!(prompt.contains("a.rs:2"), "{prompt}");
        let got = infos(&store, "", root, &paths).unwrap();
        assert_eq!(
            (got[0].line, got[0].working, got[0].changed),
            (2, true, false)
        );

        set(&store, "", root, reply("a.rs", "add", None)).unwrap();
        let got = &infos(&store, "", root, &paths).unwrap()[0];
        assert_eq!(
            (got.working, got.changed, got.description.as_str()),
            (false, false, "Adds.")
        );

        std::fs::write(root.join("a.rs"), "\n/// Docs.\nfn add() { 1; }\n").unwrap();
        let got = &infos(&store, "", root, &paths).unwrap()[0];
        assert_eq!((got.line, got.changed), (3, true));
        assert!(set(&store, "", root, reply("b.rs", "add", None)).is_err());
    }

    #[test]
    fn same_named_definitions_and_workspaces_are_kept_apart() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("a.rs"),
            "impl A {\n    fn new() {}\n}\nimpl B {\n    fn new() {}\n}\n",
        )
        .unwrap();
        let store = Store::in_memory();
        let paths = ["a.rs".to_string()];

        let prompt = ask(&store, "", root, "new", "a.rs", Some(5));
        assert!(prompt.contains("a.rs:5"), "{prompt}");
        set(&store, "", root, reply("a.rs", "new", Some(5))).unwrap();

        let got = infos(&store, "", root, &paths).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].nth, got[0].line, got[0].working), (1, 5, false));
        assert!(infos(&store, "other", root, &paths).unwrap().is_empty());
    }
}
