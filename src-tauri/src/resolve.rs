//! Resolving a conflicted commit with a headless agent. The agent edits a new commit on top of
//! the conflicted one, which is then squashed into it, and the workspace's `@` goes back to
//! where it was.

use anyhow::{Context, ensure};
use serde::Serialize;
use specta::Type;

use crate::agent::{self, Progress, Runs};
use crate::review::RepoCtx;

#[derive(Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ResolveDto {
    /// The agent's reply.
    pub text: String,
    pub stopped: bool,
    /// The commit still has conflicts.
    pub conflicted: bool,
}

/// What the agent is told. `files` are the conflicted files.
pub fn prompt(change: &str, description: &str, files: &[String]) -> String {
    let files = files
        .iter()
        .map(|f| format!("- {f}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Resolve the conflicts in jj commit {change} (\"{description}\"), which this working \
         copy sits on. These files have conflict markers:\n\n{files}\n\n\
         jj's markers are like git's, but a side can be written as a diff: after the \
         `%%%%%%%` line and the line under it come lines to apply to the base (`-` removes, `+` \
         adds, a leading space keeps), and after `+++++++` a side as it is. Edit each file so \
         it keeps what both sides meant, and remove every marker. Don't change other files. \
         Then say in a line per file how you resolved it."
    )
}

fn key(workspace: Option<&str>) -> String {
    format!("resolve-{}", workspace.unwrap_or(""))
}

impl RepoCtx {
    /// Has an agent resolve conflicted `change` in `workspace`, streaming its reply through
    /// `on_text`. Squashes the resolution into the change, or drops it when the run fails or
    /// is stopped.
    pub async fn resolve_conflict(
        &self,
        workspace: Option<&str>,
        change: &str,
        runs: &Runs,
        on_text: impl Fn(String),
    ) -> anyhow::Result<ResolveDto> {
        let ws = self.workspace(workspace).await?;
        let _busy = ws.claim()?;
        let run = runs.register(&self.agent_key(&key(workspace)))?;
        let jj = &ws.jj;
        ws.snapshot().await?;
        let target = jj.changes(change).await?;
        ensure!(target.len() == 1, "{change} isn't one commit");
        let target = &target[0];
        ensure!(target.conflict, "{change} has no conflicts");
        let files = jj.conflicted_files(&target.commit_id).await?;
        let back = jj.changes("@").await?.pop().context("no working copy")?;
        let back_parents = jj.changes("@-").await?;
        jj.new_on(&target.change_id).await?;
        let prompt = prompt(&target.change_id, &target.description, &files);
        let task = agent::Task {
            cwd: &ws.root,
            prompt: &prompt,
            resume: None,
            fork: false,
            edit: true,
            schema: None,
            purpose: agent::Purpose::Ask,
        };
        let reply = agent::run_registered(run, task, |p| {
            if let Progress::Text(t) = p {
                on_text(t)
            }
        })
        .await;
        let finished = matches!(&reply, Ok(r) if !r.stopped);
        let kept = async {
            ws.snapshot().await?;
            if finished {
                jj.squash_into_parent().await
            } else {
                jj.abandon("@").await
            }
        }
        .await;
        // Whatever happened, `@` goes back. `jj new` abandons an empty `@`, so it may be gone:
        // then a new one goes where it was.
        if jj.edit(&back.change_id).await.is_err() {
            let parents: Vec<&str> = back_parents.iter().map(|c| c.change_id.as_str()).collect();
            jj.new_on(&parents.join(" | ")).await?;
        }
        kept?;
        let reply = reply?;
        let conflicted = jj
            .changes(&target.change_id)
            .await?
            .iter()
            .any(|c| c.conflict);
        Ok(ResolveDto {
            text: reply.text,
            stopped: reply.stopped,
            conflicted,
        })
    }

    pub fn stop_resolve(&self, workspace: Option<&str>, runs: &Runs) {
        runs.stop(&self.agent_key(&key(workspace)));
    }
}

#[cfg(test)]
mod tests {
    use crate::agent::Runs;
    use crate::review::tests::{jj, repo, write};

    /// Runs the real `claude`, so it's skipped unless asked for with `--ignored`.
    #[tokio::test]
    #[ignore]
    async fn an_agent_resolves_a_conflict_and_the_working_copy_stays_put() {
        let (dir, ctx) = repo();
        let d = dir.path();
        write(d, "f.txt", "a\n");
        jj(d, &["commit", "-m", "base"]);
        write(d, "f.txt", "b\n");
        jj(d, &["commit", "-m", "left"]);
        jj(d, &["new", "description(exact:\"base\\n\")"]);
        write(d, "f.txt", "c\n");
        jj(d, &["commit", "-m", "right"]);
        jj(
            d,
            &[
                "rebase",
                "-r",
                "description(exact:\"left\\n\")",
                "-o",
                "description(exact:\"right\\n\")",
            ],
        );
        let j = &ctx.main.jj;
        let conflicted = j.changes("conflicts() ~ @").await.unwrap();
        let change = conflicted[0].change_id.clone();
        let got = ctx
            .resolve_conflict(None, &change, &Runs::default(), |_| {})
            .await
            .unwrap();
        assert!(!got.conflicted, "{}", got.text);
        assert!(j.changes("conflicts()").await.unwrap().is_empty());
        // `@` was an empty commit on `right`, which `jj new` abandoned: a new one replaces it.
        assert_eq!(j.changes("@-").await.unwrap()[0].description, "right");
    }
}
