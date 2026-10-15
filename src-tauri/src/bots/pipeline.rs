//! Running a persona review step by step. Each step's parts run at once, and a step whose
//! parts didn't all finish halts the run until it's retried.

use std::sync::Arc;

use anyhow::{Context, bail, ensure};
use futures::future::join_all;

use super::output::Output;
use super::prompts::{self, Target};
use super::{BotOptions, BotRun, Part, Persona, Status, Step, personas, run_key, run_prefix};
use crate::agent::{self, Progress, Runs};
use crate::review::{RepoCtx, now_ms};

/// What changed in a run, for the window to show.
pub enum Event {
    /// A part's agent moved on to something else, like reading a file.
    Activity {
        run: u32,
        part: u32,
        activity: String,
    },
    /// A fix's reply grew. `text` is the whole reply so far.
    FixText { run: u32, fix: u32, text: String },
    /// Anything else: parts or fixes added or finished, or the run started or stopped.
    Changed { run: u32 },
}

pub type Notify = Arc<dyn Fn(Event) + Send + Sync>;

/// The first step the run's options call for that isn't done.
pub(super) fn next_step(run: &BotRun) -> Option<Step> {
    let o = run.options;
    [
        (Step::Reports, true),
        (Step::Challenges, o.cross_review),
        (Step::Rebuttals, o.cross_review),
        (Step::Synthesis, run.synthesizes()),
        (Step::Minimal, o.suggest),
    ]
    .into_iter()
    .find(|&(s, wanted)| wanted && !run.done.contains(&s))
    .map(|(s, _)| s)
}

/// The parts `step` needs that haven't already finished: one per persona, or one in all.
/// Synthesis and the filter always run again, since the step isn't done.
pub(super) fn wanted(step: Step, run: &BotRun) -> Vec<Option<String>> {
    let names = run.personas.iter().map(|p| p.name.clone());
    let personas: Vec<String> = match step {
        Step::Reports => names.collect(),
        // Each persona reviews the others' findings, if they have any worth challenging.
        Step::Challenges => names
            .chain(run.challengers.iter().map(|p| p.name.clone()))
            .filter(|n| !prompts::challengeable(run, n).is_empty())
            .collect(),
        Step::Rebuttals => names
            .filter(|n| !prompts::challenges_to(run, n).is_empty())
            .collect(),
        Step::Synthesis | Step::Minimal => return vec![None],
    };
    personas
        .into_iter()
        .filter(|p| !finished(run, step, Some(p)))
        .map(Some)
        .collect()
}

fn finished(run: &BotRun, step: Step, persona: Option<&str>) -> bool {
    run.parts
        .iter()
        .any(|p| p.step == step && p.persona.as_deref() == persona && p.status == Status::Done)
}

/// Drops parts that didn't finish, so a retry runs them again.
pub(super) fn drop_unfinished(run: &mut BotRun) {
    run.parts.retain(|p| p.status == Status::Done);
}

/// Adds `options` to the run's. Turning on cross-review redoes synthesis and the filter, since
/// its rulings change them, and gives a lone persona the other installed ones as challengers.
pub(super) fn extend(run: &mut BotRun, options: BotOptions, installed: &[Persona]) {
    let old = run.options;
    run.options = BotOptions {
        cross_review: old.cross_review || options.cross_review,
        suggest: old.suggest || options.suggest,
    };
    if run.options.cross_review && !old.cross_review {
        let redo = [Step::Synthesis, Step::Minimal];
        run.done.retain(|s| !redo.contains(s));
        run.parts.retain(|p| !redo.contains(&p.step));
        // The new synthesis renumbers its findings, so its ids no longer name the old ones.
        run.dismissed.retain(|id| id.contains('#'));
        run.synthesis += 1;
        run.challengers = challengers(&run.personas, installed);
    }
}

/// The installed personas that cross-review `personas` when there's only one of them.
fn challengers(personas: &[Persona], installed: &[Persona]) -> Vec<Persona> {
    if personas.len() != 1 {
        return vec![];
    }
    installed
        .iter()
        .filter(|p| p.name != personas[0].name)
        .cloned()
        .collect()
}

/// Every persona whose file loads.
fn installed() -> Vec<Persona> {
    personas::load()
        .personas
        .into_iter()
        .map(|p| p.persona)
        .collect()
}

/// Sets the task for the filter, or with a blank `task` lets it work one out, and has the
/// filter run again. Its last suggestions stay until the new ones replace them.
pub(super) fn retask(run: &mut BotRun, task: &str) {
    let task = task.trim();
    run.task = (!task.is_empty()).then(|| task.to_string());
    run.options.suggest = true;
    run.done.retain(|s| *s != Step::Minimal);
}

/// Keeps only the newest finished filter part.
pub(super) fn drop_old_filters(run: &mut BotRun) {
    let Some(last) = run
        .parts
        .iter()
        .rposition(|p| p.step == Step::Minimal && p.status == Status::Done)
    else {
        return;
    };
    let mut i = 0;
    run.parts.retain(|p| {
        let keep = p.step != Step::Minimal || i == last;
        i += 1;
        keep
    });
}

/// The named personas, as their files define them now.
fn pick(names: &[String]) -> anyhow::Result<Vec<Persona>> {
    let found = personas::load().personas;
    let mut picked = Vec::new();
    for name in names {
        let p = found
            .iter()
            .find(|p| &p.persona.name == name)
            .with_context(|| format!("there's no persona named `{name}` any more"))?;
        if !picked.contains(&p.persona) {
            picked.push(p.persona.clone());
        }
    }
    ensure!(!picked.is_empty(), "pick at least one persona");
    Ok(picked)
}

impl RepoCtx {
    /// Starts the named personas reviewing `revset` in the background.
    pub async fn start_bots(
        self: Arc<Self>,
        workspace: Option<&str>,
        revset: &str,
        personas: &[String],
        options: BotOptions,
        context: String,
        runs: Runs,
        notify: Notify,
    ) -> anyhow::Result<BotRun> {
        let ws = self.workspace(workspace).await?;
        let personas = pick(personas)?;
        let challengers = if options.cross_review {
            challengers(&personas, &installed())
        } else {
            vec![]
        };
        ws.snapshot().await?;
        let range = ws.jj.resolve(revset).await?;
        let mut run = BotRun {
            id: 0,
            name: None,
            workspace: ws.name.clone(),
            revset: revset.to_string(),
            from: range.from,
            to: range.to,
            options: BotOptions {
                suggest: true,
                ..options
            },
            context,
            personas,
            challengers,
            task: None,
            parts: vec![],
            done: vec![],
            created_at: now_ms() as f64,
            fixes: vec![],
            dismissed: vec![],
            synthesis: 0,
            active: false,
        };
        run.id = self.store.lock().unwrap().add_bot_run(&run)?;
        self.clone().spawn_bots(run.id, runs, notify)?;
        Ok(self.bots.fill(run))
    }

    /// Adds steps to a run that isn't running, and runs whatever it's missing, retrying parts
    /// that failed or were stopped.
    pub fn continue_bots(
        self: Arc<Self>,
        id: u32,
        options: BotOptions,
        runs: Runs,
        notify: Notify,
    ) -> anyhow::Result<()> {
        ensure!(
            !self.bots.is_active(id),
            "this review run is already running"
        );
        let installed = installed();
        self.edit_run(id, |run| extend(run, options, &installed))?;
        self.spawn_bots(id, runs, notify)
    }

    /// Runs the filter again with `task`, or with a blank one, the task it works out.
    pub fn retask_bots(
        self: Arc<Self>,
        id: u32,
        task: &str,
        runs: Runs,
        notify: Notify,
    ) -> anyhow::Result<()> {
        ensure!(
            !self.bots.is_active(id),
            "this review run is already running"
        );
        self.edit_run(id, |run| retask(run, task))?;
        self.spawn_bots(id, runs, notify)
    }

    /// Stops the run's agents, and keeps it from starting its next step.
    pub fn stop_bots(&self, id: u32, runs: &Runs) {
        self.bots.stop(id);
        runs.stop_all(&self.agent_key(&run_prefix(id)));
    }

    pub fn delete_bots(&self, id: u32) -> anyhow::Result<()> {
        ensure!(
            !self.bots.is_active(id) && !self.bots.has_fix(id),
            "stop the run first"
        );
        Ok(self.store.lock().unwrap().delete_bot_run(id)?)
    }

    pub fn bot_runs(&self, workspace: Option<&str>) -> anyhow::Result<Vec<BotRun>> {
        let runs = self
            .store
            .lock()
            .unwrap()
            .bot_runs(workspace.unwrap_or(""))?;
        Ok(runs.into_iter().map(|r| self.bots.fill(r)).collect())
    }

    pub fn bot_run(&self, id: u32) -> anyhow::Result<BotRun> {
        let run = self
            .store
            .lock()
            .unwrap()
            .bot_run(id)?
            .context("the review run is gone")?;
        Ok(self.bots.fill(run))
    }

    /// Applies `f` to the stored run and saves it.
    pub(super) fn edit_run<T>(
        &self,
        id: u32,
        f: impl FnOnce(&mut BotRun) -> T,
    ) -> anyhow::Result<T> {
        let store = self.store.lock().unwrap();
        let mut run = store.bot_run(id)?.context("the review run is gone")?;
        let out = f(&mut run);
        store.save_bot_run(&run)?;
        Ok(out)
    }

    fn spawn_bots(self: Arc<Self>, id: u32, runs: Runs, notify: Notify) -> anyhow::Result<()> {
        if !self.bots.begin(id) {
            bail!("this review run is already running");
        }
        tauri::async_runtime::spawn(async move {
            if let Err(e) = self.drive(id, &runs, &notify).await {
                // Shown as a failed part, which a retry drops.
                self.edit_run(id, |run| {
                    let mut part = Part::running(next_step(run).unwrap_or(Step::Reports), None);
                    part.status = Status::Failed;
                    part.error = Some(format!("{e:#}"));
                    run.parts.push(part);
                })
                .ok();
            }
            self.bots.end(id);
            notify(Event::Changed { run: id });
        });
        Ok(())
    }

    async fn drive(&self, id: u32, runs: &Runs, notify: &Notify) -> anyhow::Result<()> {
        let mut run = self.edit_run(id, |run| {
            drop_unfinished(run);
            run.clone()
        })?;
        notify(Event::Changed { run: id });
        let ws = self.workspace(run.workspace.as_deref()).await?;
        let target = Target::load(&ws, &run).await?;
        while let Some(step) = next_step(&run) {
            if self.bots.stopping(id) {
                break;
            }
            let wanted = wanted(step, &run);
            let first = self.edit_run(id, |run| {
                let first = run.parts.len();
                run.parts
                    .extend(wanted.iter().map(|p| Part::running(step, p.clone())));
                first
            })?;
            notify(Event::Changed { run: id });
            let schema = super::schema::schema(step).to_string();
            let parts = wanted.iter().enumerate().map(|(i, name)| {
                let idx = first + i;
                let persona = name.as_deref().and_then(|n| run.persona(n));
                // A rebuttal continues the persona's own review.
                let resume = (step == Step::Rebuttals)
                    .then(|| session(&run, name.as_deref()))
                    .flatten();
                let prompt = prompts::prompt(step, persona, &target, &run, resume.is_some());
                let (ws, schema) = (&ws, &schema);
                async move {
                    let task = agent::Task {
                        cwd: &ws.root,
                        prompt: &prompt,
                        resume: resume.as_deref(),
                        fork: false,
                        edit: false,
                        schema: Some(schema),
                        purpose: agent::Purpose::Reviewers,
                    };
                    let reply = agent::run(runs, &self.agent_key(&run_key(id, idx)), task, |p| {
                        if let Progress::Tool(activity) = p {
                            self.bots.set_activity(id, idx, activity.clone());
                            notify(Event::Activity {
                                run: id,
                                part: idx as u32,
                                activity,
                            });
                        }
                    })
                    .await;
                    self.edit_run(id, |run| finish(&mut run.parts[idx], reply))?;
                    notify(Event::Changed { run: id });
                    anyhow::Ok(())
                }
            });
            for r in join_all(parts).await {
                r?;
            }
            let (ok, latest) = self.edit_run(id, |run| {
                let ok = run.parts[first..].iter().all(|p| p.status == Status::Done);
                if ok {
                    run.done.push(step);
                    drop_old_filters(run);
                }
                (ok, run.clone())
            })?;
            if !ok {
                break;
            }
            run = latest;
        }
        Ok(())
    }
}

/// The session of `persona`'s review.
fn session(run: &BotRun, persona: Option<&str>) -> Option<String> {
    run.parts
        .iter()
        .find(|p| {
            p.step == Step::Reports && p.persona.as_deref() == persona && p.status == Status::Done
        })
        .and_then(|p| p.session.clone())
}

fn finish(part: &mut Part, reply: anyhow::Result<agent::Reply>) {
    let fail = |part: &mut Part, error: String| {
        part.status = Status::Failed;
        part.error = Some(error);
    };
    match reply {
        Ok(r) if r.stopped => part.status = Status::Stopped,
        Ok(r) => {
            let parsed = r
                .structured
                .context("the agent didn't return structured output")
                .and_then(|v| Output::parse(part.step, part.persona.as_deref(), v));
            match parsed {
                Ok(output) => {
                    part.status = Status::Done;
                    part.output = Some(output);
                    part.session = r.session;
                }
                Err(e) => fail(part, format!("{e:#}")),
            }
        }
        Err(e) => fail(part, format!("{e:#}")),
    }
}
