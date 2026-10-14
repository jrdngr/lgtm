//! Headless agent runs. Everything specific to one agent lives in its own module; v1 runs
//! Claude Code.

mod claude;

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, bail};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::oneshot;

use claude::Line;

use crate::settings;
pub use crate::settings::Purpose;

/// How a run ended.
pub struct Reply {
    pub text: String,
    /// The reply as JSON, for a task with a schema.
    pub structured: Option<serde_json::Value>,
    /// The agent's session, for continuing the conversation.
    pub session: Option<String>,
    /// Stopped by the user. `text` is what had streamed so far.
    pub stopped: bool,
}

/// Running agents by key, so they can be stopped. Clones share the same runs.
#[derive(Clone, Default)]
pub struct Runs(Arc<Mutex<HashMap<String, Entry>>>);

/// A run's id and the sender that stops it.
type Entry = (u64, oneshot::Sender<()>);

impl Runs {
    pub fn stop(&self, key: &str) {
        if let Some((_, tx)) = self.0.lock().unwrap().remove(key) {
            tx.send(()).ok();
        }
    }

    /// Stops every run whose key starts with `prefix`.
    pub fn stop_all(&self, prefix: &str) {
        let mut map = self.0.lock().unwrap();
        let keys: Vec<String> = map
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        for k in keys {
            if let Some((_, tx)) = map.remove(&k) {
                tx.send(()).ok();
            }
        }
    }

    /// Records a run under `key`, so it can be stopped from now on, even before it starts.
    pub fn register(&self, key: &str) -> anyhow::Result<Registered<'_>> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, stop) = oneshot::channel();
        let mut map = self.0.lock().unwrap();
        if map.contains_key(key) {
            bail!("the agent is already working on this");
        }
        map.insert(key.to_string(), (id, tx));
        Ok(Registered {
            runs: self,
            key: key.to_string(),
            id,
            stop,
        })
    }
}

/// A registered run. `stop` fires when it's stopped, and the entry goes however it ends.
pub struct Registered<'a> {
    runs: &'a Runs,
    key: String,
    id: u64,
    stop: oneshot::Receiver<()>,
}

impl Drop for Registered<'_> {
    /// Leaves the entry alone when it belongs to a newer run under the same key.
    fn drop(&mut self) {
        let mut map = self.runs.0.lock().unwrap();
        if map.get(&self.key).is_some_and(|(id, _)| *id == self.id) {
            map.remove(&self.key);
        }
    }
}

/// The run's process group, which holds the agent even when a shell function starts it.
/// It's killed however the run ends.
struct Group(Option<u32>);

impl Drop for Group {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            // SAFETY: killpg takes no pointers. The group is the run's own, from setsid.
            unsafe { libc::killpg(pid as libc::pid_t, libc::SIGKILL) };
        }
    }
}

/// What a run is for.
pub struct Task<'a> {
    pub cwd: &'a Path,
    pub prompt: &'a str,
    /// The session to continue.
    pub resume: Option<&'a str>,
    /// Whether it may edit files in `cwd`.
    pub edit: bool,
    /// A JSON schema the reply must match.
    pub schema: Option<&'a str>,
    /// What the run is for, which picks its model.
    pub purpose: Purpose,
}

/// What a run reports as it goes.
pub enum Progress {
    /// A piece of the reply.
    Text(String),
    /// A tool call, summed up like "Read src/a.rs".
    Tool(String),
}

/// Runs the agent on `task`, reporting the reply and tool calls through `on` as they arrive.
pub async fn run(
    runs: &Runs,
    key: &str,
    task: Task<'_>,
    on: impl Fn(Progress),
) -> anyhow::Result<Reply> {
    run_registered(runs.register(key)?, task, on).await
}

/// Like `run`, for a run registered ahead of the work that sets it up.
pub async fn run_registered(
    mut run: Registered<'_>,
    task: Task<'_>,
    on: impl Fn(Progress),
) -> anyhow::Result<Reply> {
    let stopped = |text| Reply {
        text,
        structured: None,
        session: None,
        stopped: true,
    };
    if run.stop.try_recv().is_ok() {
        return Ok(stopped(String::new()));
    }

    let model = settings::get()
        .models
        .for_purpose(task.purpose)
        .map(String::from);
    let mut command = settings::claude::command()?;
    command
        .args(claude::args(
            task.resume,
            task.edit,
            task.schema,
            model.as_deref(),
        ))
        .current_dir(task.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // A new session, so the agent has its own process group and no controlling terminal. A
    // login shell left on the app's terminal in a background group would stop itself on it.
    // SAFETY: setsid is async-signal-safe and touches no memory.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .context("couldn't start Claude Code. Check the Claude command in Settings.")?;
    let _group = Group(child.id());
    let mut stderr = child.stderr.take().context("no stderr")?;
    // Read as it comes, so an agent that logs a lot never blocks on a full pipe.
    let stderr = tokio::spawn(async move {
        let mut err = String::new();
        stderr.read_to_string(&mut err).await.ok();
        err
    });
    let mut stdin = child.stdin.take().context("no stdin")?;
    let wrote = stdin.write_all(task.prompt.as_bytes()).await;
    drop(stdin);
    if let Err(e) = wrote {
        let status = child.wait().await?;
        let err = stderr.await.unwrap_or_default();
        let why = if err.trim().is_empty() {
            e.to_string()
        } else {
            err.trim().to_string()
        };
        bail!("the agent exited ({status}) before reading the prompt: {why}");
    }
    let mut lines = BufReader::new(child.stdout.take().context("no stdout")?).lines();

    let mut streamed = String::new();
    loop {
        let line = tokio::select! {
            line = lines.next_line() => line?,
            _ = &mut run.stop => {
                child.kill().await.ok();
                return Ok(stopped(streamed));
            }
        };
        let Some(line) = line else { break };
        match claude::parse(&line) {
            Line::Text(t) => {
                streamed.push_str(&t);
                on(Progress::Text(t));
            }
            // Text after a tool call starts a new paragraph rather than running on.
            Line::TextStart if !streamed.is_empty() && !streamed.ends_with('\n') => {
                streamed.push_str("\n\n");
                on(Progress::Text("\n\n".into()));
            }
            Line::Tool(t) => on(Progress::Tool(t)),
            Line::Done { text, error, .. } if error => bail!("the agent failed: {text}"),
            Line::Done {
                text,
                session,
                structured,
                ..
            } => {
                child.wait().await.ok();
                return Ok(Reply {
                    text,
                    structured,
                    session,
                    stopped: false,
                });
            }
            Line::TextStart | Line::Other => {}
        }
    }
    let status = child.wait().await?;
    let err = stderr.await.unwrap_or_default();
    bail!(
        "the agent exited ({status}) without replying: {}",
        err.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_run_ending_late_leaves_the_restarted_run_stoppable() {
        let runs = Runs::default();
        let old = runs.register("k").unwrap();
        runs.stop("k");
        let mut new = runs.register("k").unwrap();
        drop(old);
        runs.stop("k");
        assert!(new.stop.try_recv().is_ok());
    }

    /// Runs the real `claude`, so it's skipped unless asked for with `--ignored`.
    #[tokio::test]
    #[ignore]
    async fn runs_claude_continues_the_session_and_edits() {
        let runs = Runs::default();
        let dir = tempfile::tempdir().unwrap();
        let streamed = Mutex::new(String::new());
        let on_text = |p: Progress| {
            if let Progress::Text(t) = p {
                streamed.lock().unwrap().push_str(&t)
            }
        };
        let task = |prompt, resume| Task {
            cwd: dir.path(),
            prompt,
            resume,
            edit: true,
            schema: None,
            purpose: Purpose::Ask,
        };
        let first = run(
            &runs,
            "k",
            task("Reply with just the word: kiwi", None),
            on_text,
        )
        .await
        .unwrap();
        assert!(first.text.to_lowercase().contains("kiwi"));
        assert!(streamed.lock().unwrap().to_lowercase().contains("kiwi"));
        let prompt = "Write the word you just replied with to a new file named word.txt.";
        let next = run(&runs, "k", task(prompt, first.session.as_deref()), |_| {})
            .await
            .unwrap();
        let word = std::fs::read_to_string(dir.path().join("word.txt")).unwrap_or_default();
        assert!(word.to_lowercase().contains("kiwi"), "{}", next.text);
    }
}
