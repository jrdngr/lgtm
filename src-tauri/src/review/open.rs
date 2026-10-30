//! Opening, refreshing and closing reviews.

use super::*;

impl RepoCtx {
    pub(super) fn review(&self, id: u32) -> anyhow::Result<Arc<Mutex<Review>>> {
        self.reviews
            .open
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .context("review is closed")
    }

    pub fn close(&self, id: u32) {
        self.reviews.open.lock().unwrap().remove(&id);
        self.reviews.current.lock().unwrap().retain(|_, r| *r != id);
    }

    /// Records the review a workspace's active review pane shows, for agents to act on.
    pub fn focus(&self, workspace: Option<&str>, id: u32) {
        let key = workspace.unwrap_or("").to_string();
        self.reviews.current.lock().unwrap().insert(key, id);
    }

    pub fn current_review(&self, workspace: Option<&str>) -> anyhow::Result<u32> {
        self.reviews
            .current
            .lock()
            .unwrap()
            .get(workspace.unwrap_or(""))
            .copied()
            .context("no review is open in lgtm for this workspace")
    }

    /// Opens `revset` in the Combined view. Starts a review session, or with `resume`
    /// continues the revset's latest one, as when the same revset is loaded again.
    #[cfg(test)]
    pub async fn open(
        &self,
        workspace: Option<&str>,
        revset: &str,
        resume: bool,
    ) -> anyhow::Result<ReviewDto> {
        self.open_in(workspace, revset, resume, &View::Combined)
            .await
    }

    /// [`Self::open`], in `view`.
    pub async fn open_in(
        &self,
        workspace: Option<&str>,
        revset: &str,
        resume: bool,
        view: &View,
    ) -> anyhow::Result<ReviewDto> {
        let ws = self.workspace(workspace).await?;
        ws.snapshot().await?;
        let label = ws.label(revset);
        let (prev, resumed, seen) = {
            let store = self.store.lock().unwrap();
            match store.last_session(&label)? {
                Some(last) if resume => (
                    store.session_before(&label, last.id)?,
                    Some(last.id),
                    last.seen,
                ),
                prev => (prev, None, vec![]),
            }
        };
        let listed = Listed::of(&ws, revset).await?;
        let loaded = self.load(&ws, revset, listed, view, prev.as_ref()).await?;
        // Started only once the load succeeds, so a revset that fails leaves no session.
        let session = match resumed {
            Some(id) => id,
            None => self
                .store
                .lock()
                .unwrap()
                .start_session(&label, ws.key(), revset, now_ms())?,
        };
        let id = self.reviews.next_id.fetch_add(1, Ordering::Relaxed);
        let mut review = Review::new(ws, revset, label, loaded.cfg);
        review.view = loaded.dto.view.clone();
        review.baseline = prev
            .iter()
            .flat_map(|p| p.seen.iter().copied())
            .chain(seen)
            .collect();
        review.session = session;
        review.prev = prev;
        let dto = {
            let store = self.store.lock().unwrap();
            review.swap_in(&store, loaded, id)?.0
        };
        self.reviews
            .open
            .lock()
            .unwrap()
            .insert(id, Arc::new(Mutex::new(review)));
        Ok(dto)
    }

    /// Ends the review's session and starts another, so the next load compares against what
    /// it showed until now. For when commits are about to be replaced, as a PR refetch does.
    pub fn seal_session(&self, id: u32) -> anyhow::Result<()> {
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        let store = self.store.lock().unwrap();
        review.save_session(&store)?;
        review.prev = store.last_session(&review.label)?;
        review.session =
            store.start_session(&review.label, review.ws.key(), &review.revset, now_ms())?;
        review.shown = None;
        Ok(())
    }

    /// Whether the review shows `revset` whole, in the Combined view.
    pub fn shows_whole(&self, id: u32, revset: &str) -> anyhow::Result<bool> {
        let review = self.review(id)?;
        let review = review.lock().unwrap();
        Ok(review.revset == revset && review.view == View::Combined)
    }

    /// The workspace a review is in.
    pub fn review_workspace(&self, id: u32) -> anyhow::Result<Arc<Workspace>> {
        Ok(self.review(id)?.lock().unwrap().ws.clone())
    }

    /// Reloads a review after the repo changed, or in another view, keeping its marks' undo
    /// history. Fails with [`SUPERSEDED`] when a later refresh started before it finished.
    pub async fn refresh(&self, id: u32, view: Option<View>) -> anyhow::Result<RefreshDto> {
        let (ws, revset, view, prev, load_gen) = {
            let review = self.review(id)?;
            let mut review = review.lock().unwrap();
            // Set now, so a refresh that starts before this one ends loads this view too.
            if let Some(view) = view {
                review.view = view;
            }
            review.load_gen += 1;
            (
                review.ws.clone(),
                review.revset.clone(),
                review.view.clone(),
                review.prev.clone(),
                review.load_gen,
            )
        };
        let listed = Listed::of(&ws, &revset).await?;
        {
            let review = self.review(id)?;
            let mut review = review.lock().unwrap();
            anyhow::ensure!(review.load_gen == load_gen, SUPERSEDED);
            // Commit ids name their contents, so the same ones in the same view diff the same.
            // A turn's files and end change without new commits.
            if review.shown.as_ref() == Some(&view)
                && !matches!(view, View::Turn { .. })
                && review.whole == listed.whole
                && review.commits == listed.commits()
            {
                review.changes = views::relisted(listed.changes, &review.changes);
                let store = self.store.lock().unwrap();
                review.rematch(&store)?;
                review.save_session(&store)?;
                return Ok(RefreshDto::Same {
                    changes: review.changes.clone(),
                    state: review.state(),
                    stale: ws.stale.lock().unwrap().clone(),
                });
            }
        }
        let loaded = self
            .load(&ws, &revset, listed, &view, prev.as_ref())
            .await?;
        let review = self.review(id)?;
        let mut review = review.lock().unwrap();
        anyhow::ensure!(review.load_gen == load_gen, SUPERSEDED);
        review.view = loaded.dto.view.clone();
        let store = self.store.lock().unwrap();
        let (review, moves) = review.swap_in(&store, loaded, id)?;
        Ok(RefreshDto::Loaded {
            review: Box::new(review),
            moves,
        })
    }
}
