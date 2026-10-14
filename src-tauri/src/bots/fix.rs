//! Fixing picked findings with a headless agent that edits the working copy.

use std::sync::Arc;

use anyhow::ensure;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{BotRun, Notify, Status};
use crate::agent::{self, Progress, Runs};
use crate::review::{NOT_AT_WORKING_COPY, RepoCtx, now_ms};

/// One agent run fixing findings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Fix {
    /// Ids of the findings it was asked to fix.
    pub findings: Vec<String>,
    pub status: Status,
    /// The reply, or the reply so far while it runs.
    pub text: String,
    pub error: Option<String>,
    pub created_at: f64,
    /// The run's `synthesis` when it started. Its synthesis ids name that synthesis's findings.
    #[serde(default)]
    pub synthesis: u32,
}

impl Fix {
    /// Whether it took finding `id` of a run whose synthesis is `synthesis`. A synthesis id
    /// from an earlier synthesis names a finding that's gone.
    pub fn covers(&self, synthesis: u32, id: &str) -> bool {
        (id.contains('#') || self.synthesis == synthesis) && self.findings.iter().any(|f| f == id)
    }
}

/// What the fix agent is told. `findings` are the picked findings as markdown.
fn prompt(revset: &str, findings: &str) -> String {
    format!(
        "Fix these findings from a code review of `{revset}` in this repository. Edit the files \
         directly and change only what each finding needs. If one turns out to be wrong, leave \
         the code alone and say why. Then say in a line or two per finding what you changed.\n\n\
         {findings}"
    )
}

/// Outside `run_prefix`, so stopping a run leaves its fixes running.
pub(super) fn fix_key(run: u32, fix: usize) -> String {
    format!("bots-fix-{run}-{fix}")
}

impl BotRun {
    /// Whether finding `id` is "fixed" by a finished fix, "dismissed", or "open", as the
    /// Reviewers tab settles it.
    pub fn finding_state(&self, id: &str) -> &'static str {
        let fixed = self
            .fixes
            .iter()
            .any(|f| f.status == Status::Done && f.covers(self.synthesis, id));
        if fixed {
            "fixed"
        } else if self.dismissed.iter().any(|d| d == id) {
            "dismissed"
        } else {
            "open"
        }
    }
}

impl RepoCtx {
    /// Dismisses `findings`, or with `dismissed` false, brings them back.
    pub fn dismiss_findings(
        &self,
        id: u32,
        findings: &[String],
        dismissed: bool,
    ) -> anyhow::Result<BotRun> {
        self.edit_run(id, |run| {
            run.dismissed.retain(|f| !findings.contains(f));
            if dismissed {
                run.dismissed.extend(findings.iter().cloned());
            }
            run.clone()
        })
    }

    /// Starts an agent fixing `findings` in the run's workspace, in the background. `text` is
    /// those findings as markdown. Streams its reply as events and snapshots the working copy
    /// when it's done.
    pub async fn fix_findings(
        self: Arc<Self>,
        id: u32,
        findings: Vec<String>,
        text: String,
        runs: Runs,
        notify: Notify,
    ) -> anyhow::Result<()> {
        ensure!(!findings.is_empty(), "pick a finding to fix");
        let run = self.bot_run(id)?;
        let ws = self.workspace(run.workspace.as_deref()).await?;
        ensure!(
            ws.jj.includes_working_copy(&run.revset).await?,
            NOT_AT_WORKING_COPY
        );
        let busy = ws.claim()?;
        let idx = self.edit_run(id, |run| {
            run.fixes.push(Fix {
                findings,
                status: Status::Running,
                text: String::new(),
                error: None,
                created_at: now_ms() as f64,
                synthesis: run.synthesis,
            });
            let idx = run.fixes.len() - 1;
            self.bots.begin_fix(id, idx);
            idx
        })?;
        notify(super::Event::Changed { run: id });
        let prompt = prompt(&run.revset, &text);
        tauri::async_runtime::spawn(async move {
            let _busy = busy;
            let task = agent::Task {
                cwd: &ws.root,
                prompt: &prompt,
                resume: None,
                edit: true,
                schema: None,
                purpose: agent::Purpose::Ask,
            };
            let reply = agent::run(&runs, &self.agent_key(&fix_key(id, idx)), task, |p| {
                if let Progress::Text(text) = p {
                    notify(super::Event::FixText {
                        run: id,
                        fix: idx as u32,
                        text: self.bots.append_fix(id, idx, &text),
                    });
                }
            })
            .await;
            // Show the edits now rather than waiting on the watcher.
            ws.snapshot().await.ok();
            self.edit_run(id, |run| {
                let Some(fix) = run.fixes.get_mut(idx) else {
                    return;
                };
                match reply {
                    Ok(r) if r.stopped => {
                        fix.status = Status::Stopped;
                        fix.text = r.text;
                    }
                    Ok(r) => {
                        fix.status = Status::Done;
                        fix.text = r.text;
                    }
                    Err(e) => {
                        fix.status = Status::Failed;
                        fix.text = self.bots.append_fix(id, idx, "");
                        fix.error = Some(format!("{e:#}"));
                    }
                }
            })
            .ok();
            self.bots.end_fix(id, idx);
            notify(super::Event::Changed { run: id });
        });
        Ok(())
    }

    pub fn stop_fix(&self, id: u32, fix: u32, runs: &Runs) {
        runs.stop(&self.agent_key(&fix_key(id, fix as usize)));
    }
}
