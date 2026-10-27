//! Fixes an agent links to PR threads. Each is the span of the agent's running turn since its
//! last fix (or the prompt), over the files the turn's tools changed, so a run of fixes made
//! one after another in one turn splits into one diff each.

use anyhow::Context;
use serde::Serialize;
use specta::Type;

use super::StateDto;
use crate::ctx::{RepoCtx, now_ms};
use crate::jj::Range;
use crate::store::{Fix, Stage};
use crate::turns::{counts, span_diff, working_commit};

/// A fix linked to a thread, for its chip and the Fix view.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FixDto {
    pub id: u32,
    pub added: u32,
    pub removed: u32,
    /// When it was linked (Unix ms).
    pub at: f64,
}

impl From<&Fix> for FixDto {
    fn from(f: &Fix) -> Self {
        Self {
            id: f.id as u32,
            added: f.added,
            removed: f.removed,
            at: f.made_at as f64,
        }
    }
}

impl RepoCtx {
    /// Links what the agent changed since its last fix in its running turn to `threads` on PR
    /// `pr`, and moves them to Review. `session` is the agent's Claude Code session, else the
    /// workspace's latest running turn is taken.
    pub async fn agent_fixed(
        &self,
        workspace: Option<&str>,
        session: Option<&str>,
        pr: u32,
        threads: &[u32],
    ) -> anyhow::Result<(u32, StateDto, FixDto)> {
        anyhow::ensure!(!threads.is_empty(), "name at least one thread");
        let ws = self.workspace(workspace).await?;
        let end = working_commit(&ws).await?;
        let (turn, start) = {
            let store = self.store.lock().unwrap();
            let turn = match session {
                Some(s) => store.running_turn(s)?,
                None => store.running_agent_turn(ws.key())?,
            }
            .context(
                "lgtm has no running agent turn for this terminal, so it can't tell what you \
                 changed. lgtm's Claude Code hooks record turns.",
            )?;
            let start = store
                .last_fix_end(turn.id)?
                .unwrap_or_else(|| turn.start.clone());
            (turn, start)
        };
        let diffs = span_diff(&ws.jj, &start, &end, Some(&turn.paths)).await?;
        anyhow::ensure!(
            !diffs.is_empty(),
            "you changed no files since your last fix in this turn"
        );
        let (added, removed) = counts(diffs.iter());
        let mut paths: Vec<String> = Vec::new();
        for d in &diffs {
            for p in [Some(&d.path), d.old_path.as_ref()].into_iter().flatten() {
                if !paths.contains(p) {
                    paths.push(p.clone());
                }
            }
        }
        let mut fix = Fix {
            id: 0,
            workspace: ws.key().to_string(),
            thread: 0,
            turn: turn.id,
            start,
            end,
            paths,
            added,
            removed,
            made_at: now_ms(),
        };
        let mut last = None;
        for &thread in threads {
            fix.thread = thread.into();
            let f = &mut fix;
            last = Some(self.thread_edit(workspace, thread, pr, |store| {
                f.id = store.add_fix(f)?;
                super::threads::put_stage(store, thread, Stage::Review)
            })?);
        }
        let (id, state) = last.expect("threads isn't empty");
        Ok((id, state, FixDto::from(&fix)))
    }

    /// The commits and files fix `id`'s diff covers, unless it's gone.
    pub fn fix_span(&self, id: u32) -> anyhow::Result<Option<(Range, Option<Vec<String>>)>> {
        let fix = self.store.lock().unwrap().fix(id.into())?;
        Ok(fix.map(|f| {
            let range = Range {
                from: Some(f.start),
                to: f.end,
            };
            (range, Some(f.paths))
        }))
    }
}
