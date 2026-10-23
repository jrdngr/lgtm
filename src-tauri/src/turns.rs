//! Agent turns and checkpoints. lgtm's Claude Code hooks snapshot a terminal's workspace when
//! a prompt is sent, around each tool that can write files, and when the agent stops. A
//! turn's diff runs from its first snapshot to its last over the files its tools changed, so
//! edits made in the working copy between tool calls stay out of it. A checkpoint is a
//! snapshot the user takes, diffed whole against `@` as it is now.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::{RepoCtx, now_ms};
use crate::diff::RowKind;
use crate::jj::{FileDiff, Jj, Range};
use crate::store::Turn;
use crate::workspace::Workspace;

/// Turns older than this are dropped.
const KEEP_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// Where in an agent's turn a hook snapshots the working copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Checkpoint {
    /// A prompt was sent.
    Start,
    /// A tool that can write files is about to run.
    Before,
    /// That tool finished.
    After,
    /// The agent stopped.
    End,
}

/// A turn or checkpoint on the workspace's working-copy change.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnDto {
    pub id: u32,
    /// The prompt, `None` for a checkpoint.
    pub prompt: Option<String>,
    pub change_id: String,
    /// The agent is still working on it.
    pub running: bool,
    pub added: u32,
    pub removed: u32,
    /// When it started (Unix ms).
    pub at: f64,
}

/// Snapshots taken before tools run, by tool call, until the tools finish.
#[derive(Default)]
pub struct Befores(Mutex<HashMap<String, String>>);

/// `@`'s change and commit, after a snapshot.
async fn at(ws: &Workspace) -> anyhow::Result<(String, String)> {
    ws.snapshot().await?;
    let at = ws.jj.changes("@").await?.pop().context("no working copy")?;
    Ok((at.change_id, at.commit_id))
}

/// Whether a file's diff is of one of `paths`, under its new name or its old one.
fn touches(d: &FileDiff, paths: &[String]) -> bool {
    paths.contains(&d.path) || d.old_path.as_ref().is_some_and(|p| paths.contains(p))
}

/// Lines added and removed in `diffs`.
fn counts<'a>(diffs: impl Iterator<Item = &'a FileDiff>) -> (u32, u32) {
    let rows = diffs.flat_map(|d| &d.hunks).flat_map(|h| &h.rows);
    rows.fold((0, 0), |(a, r), row| match row.kind {
        RowKind::Added => (a + 1, r),
        RowKind::Removed => (a, r + 1),
        RowKind::Context => (a, r),
    })
}

/// The diff from `from` to `to`, only of `paths` unless they're `None`.
pub async fn span_diff(
    jj: &Jj,
    from: &str,
    to: &str,
    paths: Option<&[String]>,
) -> anyhow::Result<Vec<FileDiff>> {
    let range = Range {
        from: Some(from.to_string()),
        to: to.to_string(),
    };
    let diffs = jj.diff(&range).await?;
    Ok(match paths {
        Some(paths) => diffs.into_iter().filter(|d| touches(d, paths)).collect(),
        None => diffs,
    })
}

/// Where a turn's diff ends: where it ended, else `@` now.
fn end_of(turn: &Turn, at: &str) -> String {
    turn.end.clone().unwrap_or_else(|| at.to_string())
}

/// The files a turn's diff covers: those its tools changed, or every file for a checkpoint.
fn paths_of(turn: &Turn) -> Option<&[String]> {
    turn.session.is_some().then_some(turn.paths.as_slice())
}

impl RepoCtx {
    /// Records what a hook saw at `point` of `session`'s turn in workspace `workspace`.
    /// `tool` names the tool call around a tool. Returns whether the workspace's turns changed.
    pub async fn turn_checkpoint(
        &self,
        workspace: Option<&str>,
        session: &str,
        tool: Option<&str>,
        point: Checkpoint,
        prompt: Option<&str>,
    ) -> anyhow::Result<bool> {
        let ws = self.workspace(workspace).await?;
        let (change, commit) = at(&ws).await?;
        let running = self.store.lock().unwrap().running_turn(session)?;
        let key = tool.unwrap_or(session).to_string();
        match point {
            Checkpoint::Start => {
                if let Some(turn) = running {
                    self.end_turn(&ws, turn, &change, &commit).await?;
                }
                let store = self.store.lock().unwrap();
                store.prune_turns(now_ms() - KEEP_MS)?;
                store.start_turn(ws.key(), Some(session), prompt, &change, &commit, now_ms())?;
                Ok(true)
            }
            Checkpoint::Before => {
                self.befores.0.lock().unwrap().insert(key, commit);
                Ok(false)
            }
            Checkpoint::After => {
                let before = self.befores.0.lock().unwrap().remove(&key);
                let (Some(turn), Some(before)) = (running, before) else {
                    return Ok(false);
                };
                let range = Range {
                    from: Some(before),
                    to: commit,
                };
                let paths: Vec<String> = ws
                    .jj
                    .diff_paths(&range)
                    .await?
                    .into_iter()
                    .flat_map(|p| [Some(p.path), p.old_path].into_iter().flatten())
                    .collect();
                let fresh = paths.iter().any(|p| !turn.paths.contains(p));
                self.store.lock().unwrap().add_turn_paths(turn.id, &paths)?;
                Ok(fresh)
            }
            Checkpoint::End => match running {
                Some(turn) => self.end_turn(&ws, turn, &change, &commit).await.map(|_| true),
                None => Ok(false),
            },
        }
    }

    /// Ends `turn` on `commit`, or drops it when its tools changed no files.
    async fn end_turn(
        &self,
        ws: &Workspace,
        turn: Turn,
        change: &str,
        commit: &str,
    ) -> anyhow::Result<()> {
        if turn.paths.is_empty() {
            return Ok(self.store.lock().unwrap().delete_turn(turn.id)?);
        }
        let diffs = span_diff(&ws.jj, &turn.start, commit, Some(&turn.paths)).await?;
        let lines = counts(diffs.iter());
        let store = self.store.lock().unwrap();
        Ok(store.end_turn(turn.id, change, commit, lines, now_ms())?)
    }

    /// The turns and checkpoints on the workspace's working-copy change, newest first. A
    /// running turn shows once its tools have changed a file.
    pub async fn turns(&self, workspace: Option<&str>) -> anyhow::Result<Vec<TurnDto>> {
        let ws = self.workspace(workspace).await?;
        let Some(at) = ws.jj.changes("@").await?.pop() else {
            return Ok(vec![]);
        };
        let turns = self.store.lock().unwrap().turns_on(ws.key(), &at.change_id)?;
        let mut out = Vec::new();
        for t in turns {
            let running = t.session.is_some() && t.end.is_none();
            if running && t.paths.is_empty() {
                continue;
            }
            let (added, removed) = if t.end.is_some() {
                (t.added, t.removed)
            } else {
                counts(span_diff(&ws.jj, &t.start, &at.commit_id, paths_of(&t)).await?.iter())
            };
            out.push(TurnDto {
                id: t.id as u32,
                prompt: t.prompt,
                change_id: t.change_id,
                running,
                added,
                removed,
                at: t.started_at as f64,
            });
        }
        Ok(out)
    }

    /// Takes a checkpoint of the workspace's working copy.
    pub async fn checkpoint(&self, workspace: Option<&str>) -> anyhow::Result<()> {
        let ws = self.workspace(workspace).await?;
        let (change, commit) = at(&ws).await?;
        let store = self.store.lock().unwrap();
        store.start_turn(ws.key(), None, None, &change, &commit, now_ms())?;
        Ok(())
    }

    /// The commits and files turn `id`'s diff covers, as of `@` being `at`.
    pub fn turn_span(&self, id: u32, at: &str) -> anyhow::Result<Option<(Range, Option<Vec<String>>)>> {
        let Some(turn) = self.store.lock().unwrap().turn(id.into())? else {
            return Ok(None);
        };
        let range = Range {
            from: Some(turn.start.clone()),
            to: end_of(&turn, at),
        };
        Ok(Some((range, paths_of(&turn).map(<[String]>::to_vec))))
    }

    /// Puts the files a finished turn changed back as they were before it, unless they've
    /// changed since.
    pub async fn undo_turn(&self, workspace: Option<&str>, id: u32) -> anyhow::Result<()> {
        let ws = self.workspace(workspace).await?;
        let turn = self.store.lock().unwrap().turn(id.into())?;
        let Some(turn) = turn.filter(|t| t.session.is_some()) else {
            bail!("no such turn");
        };
        let Some(end) = &turn.end else {
            bail!("the agent is still working on it");
        };
        let (_, commit) = at(&ws).await?;
        if !span_diff(&ws.jj, end, &commit, Some(&turn.paths)).await?.is_empty() {
            bail!("its files have changed since it ended");
        }
        let _busy = ws.claim()?;
        ws.jj.restore_paths(&turn.start, &turn.paths).await?;
        Ok(self.store.lock().unwrap().delete_turn(turn.id)?)
    }

    pub fn remove_turn(&self, id: u32) -> anyhow::Result<()> {
        Ok(self.store.lock().unwrap().delete_turn(id.into())?)
    }
}

#[cfg(test)]
#[path = "turns_tests.rs"]
mod tests;
