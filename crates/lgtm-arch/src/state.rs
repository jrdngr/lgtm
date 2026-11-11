//! The stored Look checks.

use lgtm_core::store::Store;

pub const LOOK: &str = "look";

/// The Look keys checked on `revset` in workspace `ws`.
pub fn checked(store: &Store, ws: &str, revset: &str) -> anyhow::Result<Vec<String>> {
    Ok(match store.arch_review(ws, revset, LOOK)? {
        Some(data) => serde_json::from_str(&data)?,
        None => vec![],
    })
}

pub fn save_checked(store: &Store, ws: &str, revset: &str, keys: &[String]) -> anyhow::Result<()> {
    Ok(store.save_arch_review(ws, revset, LOOK, &serde_json::to_string(keys)?)?)
}

/// Unchecks `revset`'s agent and security items, whose keys a new assessment reuses.
pub fn uncheck_assessed(store: &Store, ws: &str, revset: &str) -> anyhow::Result<()> {
    let mut keys = checked(store, ws, revset)?;
    keys.retain(|k| k.starts_with("rule:"));
    save_checked(store, ws, revset, &keys)
}
