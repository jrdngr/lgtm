//! What a PR workspace leaves behind when it's deleted: its record, its `lgtm/pr-N` bookmark,
//! and the fork remote lgtm added for it, once no other PR workspace uses that.

use super::{PrRecord, fallback_bookmark};
use crate::ctx::RepoCtx;

impl RepoCtx {
    /// Cleans up after PR workspace `name`, if it is one. Failures are reported, not fatal: the
    /// workspace is already gone.
    pub async fn forget_pr(&self, name: &str) -> Vec<String> {
        let mut problems = Vec::new();
        let (pr, others) = {
            let store = self.store.lock().unwrap();
            let pr = store.pr(name).ok().flatten();
            store.delete_pr(name).ok();
            let mut others = store.prs().unwrap_or_default();
            // Another PR still fetching from the remote lgtm added takes it over, so it goes
            // with the last of them.
            if let Some(r) = pr.as_ref().and_then(|p| p.added_remote.as_ref())
                && let Some(o) = others
                    .iter_mut()
                    .find(|o| o.head_remote.as_ref() == Some(r) && o.added_remote.is_none())
            {
                o.added_remote = Some(r.clone());
                if let Err(e) = store.save_pr(o) {
                    problems.push(format!("{e:#}"));
                }
            }
            (pr, others)
        };
        let Some(pr) = pr else {
            return problems;
        };
        let jj = &self.main.jj;
        let bookmark = fallback_bookmark(pr.number);
        if jj.bookmarks(&bookmark).await.is_ok_and(|b| !b.is_empty())
            && let Err(e) = jj.bookmark_delete(&bookmark).await
        {
            problems.push(format!("{e:#}"));
        }
        let uses = |o: &PrRecord, r: &String| {
            o.head_remote.as_ref() == Some(r) || o.added_remote.as_ref() == Some(r)
        };
        if let Some(remote) = &pr.added_remote
            && !others.iter().any(|o| uses(o, remote))
            && let Err(e) = jj.remote_remove(remote).await
        {
            problems.push(format!("{e:#}"));
        }
        problems
    }
}
