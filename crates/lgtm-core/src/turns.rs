//! Recording agent turns and named steps.
//! Claude Code's hooks snapshot a workspace's working copy when a prompt is sent, around each
//! tool that can write files, and when the agent stops. A turn's diff runs from its first
//! snapshot to its last over the files its tools changed. Naming a step ends the agent's
//! running turn and starts one named for the step.

use std::path::Path;
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::diff::RowKind;
use crate::jj::{FileDiff, Jj, Range};
use crate::store::{Store, Turn};

/// Turns older than this are dropped.
pub const KEEP_MS: i64 = 14 * 24 * 60 * 60 * 1000;

/// What agents read about naming steps, as the tool's description.
pub const START_SNAPSHOT: &str = "Start a new lgtm snapshot named `label`. lgtm records the \
files you change for each prompt as one snapshot under the working copy in the human's \
revisions log, which they can review, undo or move to its own commit on its own. Your edits \
after this call go into the new snapshot, until you call it again or your turn ends. When the \
human asks for work in separate snapshots, call it before each part, naming the part.";

/// `start_snapshot`'s argument.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct StartSnapshot {
    /// What the snapshot holds, in a few words, like `Fix the off-by-one in paging`.
    pub label: String,
}

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

/// Claude Code's tools that can change files.
const WRITERS: [&str; 5] = ["Edit", "Write", "MultiEdit", "NotebookEdit", "Bash"];

/// Where in a turn a hook event is: a prompt, either side of a tool that can change files, or
/// the agent stopping.
pub fn checkpoint(input: &serde_json::Value) -> Option<Checkpoint> {
    let writes = || WRITERS.contains(&input["tool_name"].as_str().unwrap_or(""));
    match input["hook_event_name"].as_str()? {
        "UserPromptSubmit" => Some(Checkpoint::Start),
        "PreToolUse" if writes() => Some(Checkpoint::Before),
        "PostToolUse" if writes() => Some(Checkpoint::After),
        "Stop" | "StopFailure" => Some(Checkpoint::End),
        _ => None,
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// Whether a file's diff is of one of `paths`, under its new name or its old one.
fn touches(d: &FileDiff, paths: &[String]) -> bool {
    paths.contains(&d.path) || d.old_path.as_ref().is_some_and(|p| paths.contains(p))
}

/// Lines added and removed in `diffs`.
pub fn counts<'a>(diffs: impl Iterator<Item = &'a FileDiff>) -> (u32, u32) {
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

/// `@`'s change and commit, after a snapshot.
pub async fn at(jj: &Jj) -> anyhow::Result<(String, String)> {
    jj.snapshot_or_stale().await?;
    let at = jj.changes("@").await?.pop().context("no working copy")?;
    Ok((at.change_id, at.commit_id))
}

/// How the store knows the jj workspace at `root`: empty for the repo's main one, else its
/// name.
pub async fn workspace_key(jj: &Jj, root: &Path) -> anyhow::Result<String> {
    if crate::paths::main_root(root).is_none() {
        return Ok(String::new());
    }
    let root = root.canonicalize()?;
    let found = jj
        .workspaces()
        .await?
        .into_iter()
        .find(|w| w.root.canonicalize().is_ok_and(|r| r == root));
    Ok(found.context("jj doesn't list this workspace")?.name)
}

/// A turn in workspace `key`, where `@` is `here` (its change and commit).
pub struct Point<'a> {
    pub key: &'a str,
    pub here: (&'a str, &'a str),
    pub session: &'a str,
    /// The tool call, around a tool.
    pub tool: Option<&'a str>,
    pub prompt: Option<&'a str>,
}

/// Records what a hook saw at `point` of a turn. Returns the new turn's id when one started,
/// and whether the workspace's turns changed.
pub async fn record(
    store: &Mutex<Store>,
    jj: &Jj,
    p: &Point<'_>,
    point: Checkpoint,
) -> anyhow::Result<(Option<i64>, bool)> {
    let (change, commit) = p.here;
    let running = store.lock().unwrap().running_turn(p.session)?;
    let key = p.tool.unwrap_or(p.session);
    match point {
        Checkpoint::Start => {
            if let Some(turn) = running {
                end_turn(store, jj, turn, change, commit).await?;
            }
            let store = store.lock().unwrap();
            store.prune_turns(now_ms() - KEEP_MS)?;
            let id =
                store.start_turn(p.key, Some(p.session), p.prompt, change, commit, now_ms())?;
            Ok((Some(id), true))
        }
        Checkpoint::Before => {
            store
                .lock()
                .unwrap()
                .set_turn_before(key, commit, now_ms())?;
            Ok((None, false))
        }
        Checkpoint::After => {
            let before = store.lock().unwrap().take_turn_before(key)?;
            let (Some(turn), Some(before)) = (running, before) else {
                return Ok((None, false));
            };
            let range = Range {
                from: Some(before),
                to: commit.to_string(),
            };
            let diffs = jj.diff(&range).await?;
            let paths: Vec<String> = diffs
                .iter()
                .flat_map(|d| {
                    [Some(d.path.clone()), d.old_path.clone()]
                        .into_iter()
                        .flatten()
                })
                .collect();
            if paths.is_empty() {
                return Ok((None, false));
            }
            store.lock().unwrap().add_turn_paths(turn.id, &paths)?;
            Ok((None, true))
        }
        Checkpoint::End => match running {
            Some(turn) => {
                end_turn(store, jj, turn, change, commit).await?;
                Ok((None, true))
            }
            None => Ok((None, false)),
        },
    }
}

/// Ends `turn` on `commit`, or drops it when its tools changed no files.
pub async fn end_turn(
    store: &Mutex<Store>,
    jj: &Jj,
    turn: Turn,
    change: &str,
    commit: &str,
) -> anyhow::Result<()> {
    if turn.paths.is_empty() {
        return Ok(store.lock().unwrap().delete_turn(turn.id)?);
    }
    let diffs = span_diff(jj, &turn.start, commit, Some(&turn.paths)).await?;
    let lines = counts(diffs.iter());
    let store = store.lock().unwrap();
    Ok(store.end_turn(turn.id, change, commit, lines, now_ms())?)
}

/// Starts a step named `label` in workspace `key`, where `@` is `here`: a new turn for the
/// agent's running one, else a checkpoint named `label`.
pub async fn start_step(
    store: &Mutex<Store>,
    jj: &Jj,
    key: &str,
    here: (&str, &str),
    label: &str,
) -> anyhow::Result<()> {
    let running = store.lock().unwrap().running_agent_turn(key)?;
    match running.and_then(|t| t.session) {
        Some(session) => {
            let point = Point {
                key,
                here,
                session: &session,
                tool: None,
                prompt: Some(label),
            };
            record(store, jj, &point, Checkpoint::Start).await?;
        }
        None => {
            let store = store.lock().unwrap();
            store.start_turn(key, None, Some(label), here.0, here.1, now_ms())?;
        }
    }
    Ok(())
}

/// What a turn is called in lists: its label or prompt's first line, else what it is.
pub fn turn_name(t: &Turn) -> String {
    let first = t.prompt.as_deref().and_then(|p| p.lines().next());
    match first.map(str::trim).filter(|l| !l.is_empty()) {
        Some(line) => line.to_string(),
        None if t.human => "Edit".into(),
        None if t.session.is_none() => "Checkpoint".into(),
        None => "Agent turn".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::test_support::{repo, write};

    async fn here(jj: &Jj) -> (String, String) {
        at(jj).await.unwrap()
    }

    #[tokio::test]
    async fn a_step_named_mid_turn_ends_the_agents_turn_and_without_one_is_a_checkpoint() {
        let (dir, jj) = repo();
        let store = Mutex::new(Store::in_memory());
        let (change, commit) = here(&jj).await;
        start_step(&store, &jj, "", (&change, &commit), "plan")
            .await
            .unwrap();
        let plan = store.lock().unwrap().turns_on("", &change).unwrap();
        assert_eq!(
            (turn_name(&plan[0]), plan[0].session.as_deref()),
            ("plan".to_string(), None)
        );

        // Each hook runs in its own process, so the snapshot before a tool is in the store.
        let (c, start) = here(&jj).await;
        let p = Point {
            key: "",
            here: (&c, &start),
            session: "s",
            tool: None,
            prompt: Some("add a"),
        };
        record(&store, &jj, &p, Checkpoint::Start).await.unwrap();
        let before = Point {
            tool: Some("t1"),
            ..p
        };
        record(&store, &jj, &before, Checkpoint::Before)
            .await
            .unwrap();
        write(dir.path(), "a.rs", "fn a() {}\n");
        let (c, after) = here(&jj).await;
        let p = Point {
            key: "",
            here: (&c, &after),
            session: "s",
            tool: Some("t1"),
            prompt: None,
        };
        assert!(record(&store, &jj, &p, Checkpoint::After).await.unwrap().1);

        start_step(&store, &jj, "", (&c, &after), "add b")
            .await
            .unwrap();
        let turns = store.lock().unwrap().turns_on("", &c).unwrap();
        let names: Vec<String> = turns.iter().map(turn_name).collect();
        assert_eq!(names, ["add b", "add a", "plan"]);
        assert_eq!(turns[1].end.as_deref(), Some(after.as_str()));
        assert_eq!(turns[1].paths, ["a.rs"]);
        assert_eq!(turns[0].session.as_deref(), Some("s"));
        let starting = store
            .lock()
            .unwrap()
            .turns_starting_at(std::slice::from_ref(&after))
            .unwrap();
        assert_eq!(turn_name(&starting[&after]), "add b");
    }

    /// Runs one tool call of session `s`'s turn that writes `text` to `a.rs`.
    async fn tool(store: &Mutex<Store>, jj: &Jj, dir: &Path, id: &str, text: &str) {
        let (c, before) = here(jj).await;
        let mut p = Point {
            key: "",
            here: (&c, &before),
            session: "s",
            tool: Some(id),
            prompt: None,
        };
        record(store, jj, &p, Checkpoint::Before).await.unwrap();
        write(dir, "a.rs", text);
        let (c, after) = here(jj).await;
        p.here = (&c, &after);
        record(store, jj, &p, Checkpoint::After).await.unwrap();
    }

    #[tokio::test]
    async fn a_turn_keeps_each_file_its_tools_wrote_once() {
        let (dir, jj) = repo();
        write(dir.path(), "a.rs", "old\n");
        let store = Mutex::new(Store::in_memory());
        let (c, start) = here(&jj).await;
        let p = Point {
            key: "",
            here: (&c, &start),
            session: "s",
            tool: None,
            prompt: Some("go"),
        };
        record(&store, &jj, &p, Checkpoint::Start).await.unwrap();
        tool(&store, &jj, dir.path(), "t1", "new1\n").await;
        tool(&store, &jj, dir.path(), "t2", "new2\n").await;
        let turn = store.lock().unwrap().running_turn("s").unwrap().unwrap();
        assert_eq!(turn.paths, ["a.rs"]);
    }
}
