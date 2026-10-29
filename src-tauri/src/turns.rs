//! Agent turns and checkpoints. lgtm's Claude Code hooks snapshot a terminal's workspace when
//! a prompt is sent, around each tool that can write files, and when the agent stops. A
//! turn's diff runs from its first snapshot to its last over the files its tools changed, so
//! edits made in the working copy between tool calls stay out of it. Saving a file in lgtm's
//! editor snapshots around the save, and saves in a row make one edit of the user's. Each
//! change made in the Files sidebar is an edit of its own. A
//! checkpoint is a snapshot the user takes, diffed whole against `@` as it is now.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::{RepoCtx, now_ms};
use crate::diff::RowKind;
use crate::jj::{FileDiff, Jj, JjAction, Range};
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

/// Who made a turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TurnKind {
    Agent,
    /// The user, saving files in lgtm's editor or changing them in the Files sidebar.
    Edit,
    Checkpoint,
}

/// A turn, saved edit or checkpoint on the workspace's working-copy change.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TurnDto {
    pub id: u32,
    pub kind: TurnKind,
    /// The prompt, for an agent's turn, or what the user did in the Files sidebar, for an
    /// edit made there.
    pub prompt: Option<String>,
    /// The terminal panel the agent ran in, for an agent's turn.
    pub terminal: Option<String>,
    /// The files it changed; none for a checkpoint.
    pub paths: Vec<String>,
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

/// `@`'s commit, after a snapshot.
pub async fn working_commit(ws: &Workspace) -> anyhow::Result<String> {
    Ok(at(ws).await?.1)
}

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
pub(crate) fn counts<'a>(diffs: impl Iterator<Item = &'a FileDiff>) -> (u32, u32) {
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

/// The files a turn's diff covers: those it changed, or every file for a checkpoint.
fn paths_of(turn: &Turn) -> Option<&[String]> {
    (!turn.is_checkpoint()).then_some(turn.paths.as_slice())
}

fn kind_of(turn: &Turn) -> TurnKind {
    match (turn.human, turn.session.is_some()) {
        (true, _) => TurnKind::Edit,
        (false, true) => TurnKind::Agent,
        (false, false) => TurnKind::Checkpoint,
    }
}

impl RepoCtx {
    /// Records what a hook saw at `point` of `session`'s turn in workspace `workspace`, its
    /// agent running in terminal panel `terminal`. `tool` names the tool call around a tool.
    /// Returns whether the workspace's turns changed.
    pub async fn turn_checkpoint(
        &self,
        workspace: Option<&str>,
        terminal: Option<&str>,
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
                store.prune_fixes(now_ms() - KEEP_MS)?;
                let id = store.start_turn(
                    ws.key(),
                    Some(session),
                    prompt,
                    &change,
                    &commit,
                    now_ms(),
                )?;
                if let Some(terminal) = terminal {
                    store.set_turn_terminal(id, terminal)?;
                }
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
                Some(turn) => self
                    .end_turn(&ws, turn, &change, &commit)
                    .await
                    .map(|_| true),
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
        let turns = self
            .store
            .lock()
            .unwrap()
            .turns_on(ws.key(), &at.change_id)?;
        let mut out = Vec::new();
        for t in turns {
            let running = t.session.is_some() && t.end.is_none();
            if running && t.paths.is_empty() {
                continue;
            }
            let (added, removed) = if t.end.is_some() {
                (t.added, t.removed)
            } else {
                counts(
                    span_diff(&ws.jj, &t.start, &at.commit_id, paths_of(&t))
                        .await?
                        .iter(),
                )
            };
            out.push(TurnDto {
                id: t.id as u32,
                kind: kind_of(&t),
                prompt: t.prompt,
                terminal: t.terminal,
                paths: t.paths,
                change_id: t.change_id,
                running,
                added,
                removed,
                at: t.started_at as f64,
            });
        }
        Ok(out)
    }

    /// Records the user's save of `path` as an edit, `before` being `@`'s commit before it.
    /// A save right after another edit, with no turn or checkpoint between, joins it.
    /// Returns whether the workspace's turns changed.
    pub async fn record_save(
        &self,
        ws: &Workspace,
        path: &str,
        before: &str,
    ) -> anyhow::Result<bool> {
        self.record_edit(ws, &[path.to_string()], before, None)
            .await
    }

    /// Records a change the user made in the Files sidebar to `roots` (files or folders) as an
    /// edit of its own named `label`, `before` being `@`'s commit before it. Returns whether
    /// the workspace's turns changed.
    pub async fn record_files_change(
        &self,
        ws: &Workspace,
        roots: &[String],
        before: &str,
        label: &str,
    ) -> anyhow::Result<bool> {
        self.record_edit(ws, roots, before, Some(label)).await
    }

    /// Records the files under `roots` that changed since `before` as the user's edit: one of
    /// its own named `label`, else joining the last edit of saves when nothing came between.
    async fn record_edit(
        &self,
        ws: &Workspace,
        roots: &[String],
        before: &str,
        label: Option<&str>,
    ) -> anyhow::Result<bool> {
        let (change, commit) = at(ws).await?;
        if commit == before {
            return Ok(false);
        }
        let under = |p: &str| {
            roots.iter().any(|r| {
                p == r
                    || p.strip_prefix(r.as_str())
                        .is_some_and(|s| s.starts_with('/'))
            })
        };
        let changed: Vec<String> = span_diff(&ws.jj, before, &commit, None)
            .await?
            .into_iter()
            .flat_map(|d| [Some(d.path), d.old_path])
            .flatten()
            .filter(|p| under(p))
            .collect();
        if changed.is_empty() {
            return Ok(false);
        }
        let (id, start, mut paths) = {
            let store = self.store.lock().unwrap();
            let last = store.turns_on(ws.key(), &change)?.into_iter().next();
            match last.filter(|t| label.is_none() && t.human && t.prompt.is_none()) {
                Some(t) => (t.id, t.start, t.paths),
                None => (
                    store.start_edit(ws.key(), &change, before, label, now_ms())?,
                    before.to_string(),
                    vec![],
                ),
            }
        };
        for p in changed {
            if !paths.contains(&p) {
                paths.push(p);
            }
        }
        let lines = counts(
            span_diff(&ws.jj, &start, &commit, Some(&paths))
                .await?
                .iter(),
        );
        let store = self.store.lock().unwrap();
        store.add_turn_paths(id, &paths)?;
        store.end_turn(id, &change, &commit, lines, now_ms())?;
        Ok(true)
    }

    /// Runs `action` in the workspace and returns how many jj operations it took. A squash
    /// moves the squashed changes' turns onto the change they went into.
    pub async fn jj_act(&self, workspace: Option<&str>, action: &JjAction) -> anyhow::Result<u32> {
        let ws = self.workspace(workspace).await?;
        let JjAction::Squash { revs } = action else {
            return ws.jj.act(action).await;
        };
        let ids = |cs: Vec<crate::jj::Change>| cs.into_iter().map(|c| c.change_id).collect();
        let from: Vec<String> = ids(ws.jj.changes(revs).await?);
        let into: Vec<String> = ids(ws.jj.changes(&format!("roots({revs})-")).await?);
        let steps = ws.jj.act(action).await?;
        if let [into] = into.as_slice() {
            self.store
                .lock()
                .unwrap()
                .move_turns(ws.key(), &from, into)?;
        }
        Ok(steps)
    }

    /// Takes a checkpoint of the workspace's working copy.
    pub async fn checkpoint(&self, workspace: Option<&str>) -> anyhow::Result<()> {
        let ws = self.workspace(workspace).await?;
        let (change, commit) = at(&ws).await?;
        let store = self.store.lock().unwrap();
        store.start_turn(ws.key(), None, None, &change, &commit, now_ms())?;
        Ok(())
    }

    /// The commits and files the diff of turns `ids` covers, as of `@` being `at`: from the
    /// first's start to the last's end, over every file any of them changed, or every file
    /// when one is a checkpoint. Turns that are gone are left out.
    pub fn turn_span(
        &self,
        ids: &[u32],
        at: &str,
    ) -> anyhow::Result<Option<(Range, Option<Vec<String>>)>> {
        let mut turns = Vec::new();
        let store = self.store.lock().unwrap();
        for id in ids {
            turns.extend(store.turn((*id).into())?);
        }
        drop(store);
        turns.sort_by_key(|t| (t.started_at, t.id));
        let (Some(first), Some(last)) = (turns.first(), turns.last()) else {
            return Ok(None);
        };
        let range = Range {
            from: Some(first.start.clone()),
            to: end_of(last, at),
        };
        let paths = turns
            .iter()
            .map(paths_of)
            .collect::<Option<Vec<_>>>()
            .map(|ps| {
                let mut all: Vec<String> = Vec::new();
                for p in ps.into_iter().flatten() {
                    if !all.contains(p) {
                        all.push(p.clone());
                    }
                }
                all
            });
        Ok(Some((range, paths)))
    }

    /// Puts the files a finished turn or edit changed back as they were before it, unless they've
    /// changed since.
    pub async fn undo_turn(&self, workspace: Option<&str>, id: u32) -> anyhow::Result<()> {
        let ws = self.workspace(workspace).await?;
        let turn = self.store.lock().unwrap().turn(id.into())?;
        let Some(turn) = turn.filter(|t| !t.is_checkpoint()) else {
            bail!("no such turn");
        };
        let Some(end) = &turn.end else {
            bail!("the agent is still working on it");
        };
        let (_, commit) = at(&ws).await?;
        if !span_diff(&ws.jj, end, &commit, Some(&turn.paths))
            .await?
            .is_empty()
        {
            bail!("its files have changed since it ended");
        }
        let _busy = ws.claim()?;
        ws.jj.restore_paths(&turn.start, &turn.paths).await?;
        Ok(self.store.lock().unwrap().delete_turn(turn.id)?)
    }

    /// Splits `@` by time around turns `ids`: what came before them, what they changed, and
    /// the rest stay in that order, each turn going with its part.
    pub async fn split_at_turns(&self, workspace: Option<&str>, ids: &[u32]) -> anyhow::Result<()> {
        let ws = self.workspace(workspace).await?;
        let _busy = ws.claim()?;
        let (change, commit) = at(&ws).await?;
        let mut turns = self.store.lock().unwrap().turns_on(ws.key(), &change)?;
        turns.sort_by_key(|t| (t.started_at, t.id));
        let picked = |t: &Turn| ids.contains(&(t.id as u32));
        let Some(first) = turns.iter().position(picked) else {
            bail!("no such turn");
        };
        if turns
            .iter()
            .any(|t| picked(t) && t.session.is_some() && t.end.is_none())
        {
            bail!("the agent is still working on it");
        }
        let Some((range, paths)) = self.turn_span(ids, &commit)? else {
            bail!("no such turn");
        };
        let from = range.from.unwrap_or_default();
        if span_diff(&ws.jj, &from, &range.to, paths.as_deref())
            .await?
            .is_empty()
        {
            bail!("there's nothing in them to move");
        }
        let (before, middle) = ws
            .jj
            .slice_working_copy(&from, &range.to, paths.as_deref())
            .await?;
        let store = self.store.lock().unwrap();
        for t in &turns[..first] {
            if let Some(before) = &before {
                store.set_turn_change(t.id, before)?;
            }
        }
        for t in turns.iter().filter(|t| picked(t)) {
            store.set_turn_change(t.id, &middle)?;
        }
        Ok(())
    }

    pub fn remove_turn(&self, id: u32) -> anyhow::Result<()> {
        Ok(self.store.lock().unwrap().delete_turn(id.into())?)
    }
}

#[cfg(test)]
#[path = "turns_tests.rs"]
mod tests;
