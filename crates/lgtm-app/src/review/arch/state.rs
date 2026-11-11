//! The app side of `lgtm_arch::state`.

use crate::ctx::RepoCtx;
pub use lgtm_arch::state::*;

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
