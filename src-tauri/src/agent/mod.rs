//! Headless agent runs. Everything specific to one agent lives in its own module; v1 runs
//! Claude Code.

mod claude;

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
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
pub struct Runs(Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>);

impl Runs {
    pub fn stop(&self, key: &str) {
        if let Some(tx) = self.0.lock().unwrap().remove(key) {
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
            if let Some(tx) = map.remove(&k) {
                tx.send(()).ok();
            }
        }
    }

    /// Records a run under `key`. The receiver fires when it's stopped.
    fn register(&self, key: &str) -> anyhow::Result<(Registered<'_>, oneshot::Receiver<()>)> {
        let (tx, rx) = oneshot::channel();
        let mut map = self.0.lock().unwrap();
        if map.contains_key(key) {
            bail!("the agent is already working on this");
        }
        map.insert(key.to_string(), tx);
        Ok((Registered(self, key.to_string()), rx))
    }
}

/// Removes a run's entry however the run ends.
struct Registered<'a>(&'a Runs, String);

impl Drop for Registered<'_> {
    fn drop(&mut self) {
        self.0.0.lock().unwrap().remove(&self.1);
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
    let (_registered, mut stop) = runs.register(key)?;

    let model = settings::get()
        .models
        .for_purpose(task.purpose)
        .map(String::from);
    let mut child = settings::claude::command()?
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
        .kill_on_drop(true)
        .spawn()
        .context("couldn't start Claude Code. Check the Claude command in Settings.")?;
    let mut stdin = child.stdin.take().context("no stdin")?;
    stdin.write_all(task.prompt.as_bytes()).await?;
    drop(stdin);
    let mut stderr = child.stderr.take().context("no stderr")?;
    let mut lines = BufReader::new(child.stdout.take().context("no stdout")?).lines();

    let mut streamed = String::new();
    loop {
        let line = tokio::select! {
            line = lines.next_line() => line?,
            _ = &mut stop => {
                child.kill().await.ok();
                return Ok(Reply { text: streamed, structured: None, session: None, stopped: true });
            }
        };
        let Some(line) = line else { break };
        match claude::parse(&line) {
            Line::Text(t) => {
                streamed.push_str(&t);
                on(Progress::Text(t));
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
            Line::Other => {}
        }
    }
    let status = child.wait().await?;
    let mut err = String::new();
    stderr.read_to_string(&mut err).await.ok();
    bail!(
        "the agent exited ({status}) without replying: {}",
        err.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
