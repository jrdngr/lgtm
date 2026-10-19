//! The stored Look checks.

use crate::ctx::RepoCtx;
use crate::store::Store;

const LOOK: &str = "look";

/// The Look keys checked on `revset` in workspace `ws`.
pub fn checked(store: &Store, ws: &str, revset: &str) -> anyhow::Result<Vec<String>> {
    Ok(match store.arch_review(ws, revset, LOOK)? {
        Some(data) => serde_json::from_str(&data)?,
        None => vec![],
    })
}

fn save_checked(store: &Store, ws: &str, revset: &str, keys: &[String]) -> anyhow::Result<()> {
    Ok(store.save_arch_review(ws, revset, LOOK, &serde_json::to_string(keys)?)?)
}

/// Unchecks `revset`'s agent and security items, whose keys a new assessment reuses.
pub fn uncheck_assessed(store: &Store, ws: &str, revset: &str) -> anyhow::Result<()> {
    let mut keys = checked(store, ws, revset)?;
    keys.retain(|k| k.starts_with("rule:"));
    save_checked(store, ws, revset, &keys)
}

impl RepoCtx {
    /// Checks or unchecks a Look item for the review's revset.
    pub fn check_look(&self, review: u32, key: &str, checked: bool) -> anyhow::Result<()> {
        let (ws, revset) = {
            let r = self.review(review)?;
            let r = r.lock().unwrap();
            (r.ws.clone(), r.revset.clone())
        };
        let store = self.store.lock().unwrap();
        let mut keys = self::checked(&store, ws.key(), &revset)?;
        keys.retain(|k| k != key);
        if checked {
            keys.push(key.to_string());
        }
        save_checked(&store, ws.key(), &revset, &keys)
    }
}
