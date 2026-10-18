//! The stored lens, intents and Look checks.

use super::{IntentKind, Lens, LensEdit, group};
use crate::review::RepoCtx;
use crate::store::Store;

const LENS: &str = "lens";
const LOOK: &str = "look";

pub fn lens(store: &Store) -> anyhow::Result<Lens> {
    Ok(match store.arch_state(LENS)? {
        Some(data) => serde_json::from_str(&data)?,
        None => Lens::default(),
    })
}

/// Every intent, oldest first. Rows that no longer parse are skipped.
pub fn intents(store: &Store) -> anyhow::Result<Vec<(u32, IntentKind)>> {
    Ok(store
        .arch_intents()?
        .into_iter()
        .filter_map(|(id, data)| Some((id, serde_json::from_str(&data).ok()?)))
        .collect())
}

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
    pub fn arch_lens(&self, edit: LensEdit) -> anyhow::Result<()> {
        let store = self.store.lock().unwrap();
        let mut l = lens(&store)?;
        group::edit(&mut l, edit);
        Ok(store.save_arch_state(LENS, &serde_json::to_string(&l)?)?)
    }

    /// Adds an intent, returning its id.
    pub fn add_intent(&self, kind: IntentKind) -> anyhow::Result<u32> {
        let data = serde_json::to_string(&kind)?;
        Ok(self.store.lock().unwrap().add_arch_intent(&data)?)
    }

    pub fn remove_intent(&self, id: u32) -> anyhow::Result<()> {
        Ok(self.store.lock().unwrap().delete_arch_intent(id)?)
    }

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
