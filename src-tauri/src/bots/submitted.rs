//! Findings an agent publishes through the MCP server, stored as a reviewer's section of a
//! report so they show and act like a persona's.

use std::collections::HashMap;

use anyhow::{Context, ensure};

use super::output::{Action, Finding, Location, Minimal, Output, Question, Review};
use super::{BotOptions, BotRun, Part, Persona, Status, Step};
use crate::review::{RepoCtx, now_ms};

/// An agent's own suggested fixes, naming findings by their 1-based place in what it published.
pub struct Summary {
    pub task: String,
    pub fixes: Vec<(String, Vec<u32>)>,
    pub off_task: Vec<u32>,
    pub questions: Vec<(String, Vec<u32>)>,
}

impl Summary {
    /// The filter's output for `summary`, with places turned into the finding ids `parse`
    /// gives them once it sorts them by severity.
    pub(super) fn to_filter(&self, reviewer: &str, review: &serde_json::Value) -> anyhow::Result<Minimal> {
        let given: Review = serde_json::from_value(review.clone())?;
        let mut order: Vec<usize> = (0..given.findings.len()).collect();
        order.sort_by_key(|&i| given.findings[i].severity);
        let mut ids = vec![String::new(); order.len()];
        for (rank, &i) in order.iter().enumerate() {
            ids[i] = format!("{reviewer}#{}", rank + 1);
        }
        let id = |n: &u32| {
            (*n as usize)
                .checked_sub(1)
                .and_then(|i| ids.get(i).cloned())
                .with_context(|| format!("there's no finding {n}; number them from 1"))
        };
        let ids_of = |ns: &[u32]| ns.iter().map(id).collect::<anyhow::Result<Vec<_>>>();
        Ok(Minimal {
            task: self.task.clone(),
            fixes: self
                .fixes
                .iter()
                .map(|(action, closes)| {
                    Ok(Action {
                        action: action.clone(),
                        closes: ids_of(closes)?,
                    })
                })
                .collect::<anyhow::Result<_>>()?,
            off_task: ids_of(&self.off_task)?,
            questions: self
                .questions
                .iter()
                .map(|(question, findings)| {
                    Ok(Question {
                        question: question.clone(),
                        findings: ids_of(findings)?,
                    })
                })
                .collect::<anyhow::Result<_>>()?,
        })
    }
}

impl RepoCtx {
    /// Stores `review` (a verdict and findings, as a persona returns them) from `reviewer`,
    /// with the agent's own suggested fixes when it gives them. With `report`, it replaces
    /// that reviewer's section of that report, or adds one, and the user's dismissals and
    /// fixes follow findings with the same title and locations. Without, it's a new report
    /// against the workspace's current review. Returns the report and whether it's new.
    pub fn publish_findings(
        &self,
        workspace: Option<&str>,
        report: Option<&str>,
        reviewer: &str,
        review: serde_json::Value,
        summary: Option<Summary>,
    ) -> anyhow::Result<(BotRun, bool)> {
        let filter = summary
            .map(|s| s.to_filter(reviewer, &review))
            .transpose()?;
        let output = Output::parse(Step::Reports, Some(reviewer), review)?;
        let Some(label) = report else {
            return Ok((self.new_report(workspace, reviewer, output, filter)?, true));
        };
        let id = self.find_report(workspace, label)?.id;
        ensure!(
            !self.bots.is_active(id),
            "report {label} is running its reviewers; publish when it's done"
        );
        self.edit_run(id, |run| {
            replace_section(run, reviewer, output);
            set_filter(run, filter);
        })?;
        Ok((self.bot_run(id)?, false))
    }

    fn new_report(
        &self,
        workspace: Option<&str>,
        reviewer: &str,
        output: Output,
        filter: Option<Minimal>,
    ) -> anyhow::Result<BotRun> {
        let (_, (revset, from, to)) = self.with_current(workspace, |r| r.target())?;
        let mut run = BotRun {
            id: 0,
            name: None,
            workspace: workspace.map(str::to_string),
            revset,
            from,
            to,
            personas: vec![persona(reviewer)],
            challengers: vec![],
            options: BotOptions::default(),
            task: None,
            context: String::new(),
            parts: vec![Part {
                status: Status::Done,
                output: Some(output),
                ..Part::running(Step::Reports, Some(reviewer.to_string()))
            }],
            done: vec![Step::Reports],
            created_at: now_ms() as f64,
            fixes: vec![],
            dismissed: vec![],
            synthesis: 0,
            active: false,
        };
        set_filter(&mut run, filter);
        run.id = self.store.lock().unwrap().add_bot_run(&run)?;
        Ok(run)
    }
}

/// Drops what lgtm worked out from the findings before, which no longer match them, and makes
/// the agent's suggested fixes the report's when it gave some.
fn set_filter(run: &mut BotRun, filter: Option<Minimal>) {
    let stale = [Step::Challenges, Step::Rebuttals, Step::Synthesis, Step::Minimal];
    if run.parts.iter().any(|p| p.step == Step::Synthesis) {
        // Ids of the dropped synthesis no longer name anything.
        run.dismissed.retain(|id| id.contains('#'));
        run.synthesis += 1;
    }
    run.parts.retain(|p| !stale.contains(&p.step));
    run.done.retain(|s| !stale.contains(s));
    run.challengers.clear();
    run.options = BotOptions {
        cross_review: false,
        suggest: filter.is_some(),
    };
    if let Some(f) = filter {
        run.parts.push(Part {
            status: Status::Done,
            output: Some(Output::Minimal(f)),
            ..Part::running(Step::Minimal, None)
        });
        run.done.push(Step::Minimal);
    }
}

fn persona(reviewer: &str) -> Persona {
    Persona {
        name: reviewer.to_string(),
        description: "an agent in a terminal".into(),
        off_lens: false,
        profile: String::new(),
    }
}

/// What a finding is for carrying the user's marks across a re-publish.
type Key = (String, Vec<Location>);

fn key(f: &Finding) -> Key {
    (f.title.clone(), f.locations.clone())
}

fn findings(part: &Part) -> &[Finding] {
    match &part.output {
        Some(Output::Review(r)) => &r.findings,
        _ => &[],
    }
}

/// Puts `output` in place of `reviewer`'s section of `run`, or adds it as a new section, and
/// moves dismissals and fixes of the old section's findings to their matches in the new one.
fn replace_section(run: &mut BotRun, reviewer: &str, output: Output) {
    let part = Part {
        status: Status::Done,
        output: Some(output),
        ..Part::running(Step::Reports, Some(reviewer.to_string()))
    };
    let at = run
        .parts
        .iter()
        .position(|p| p.step == Step::Reports && p.persona.as_deref() == Some(reviewer));
    let old: HashMap<String, Key> = at
        .map(|i| findings(&run.parts[i]).iter().map(|f| (f.id.clone(), key(f))).collect())
        .unwrap_or_default();
    let new: HashMap<Key, String> = findings(&part)
        .iter()
        .map(|f| (key(f), f.id.clone()))
        .collect();
    match at {
        Some(i) => run.parts[i] = part,
        None => {
            run.personas.push(persona(reviewer));
            run.parts.push(part);
        }
    }
    // Ids of the old section map to the new finding with the same key, or are dropped.
    let moved = |ids: &mut Vec<String>| {
        *ids = ids
            .iter()
            .filter_map(|id| match old.get(id) {
                Some(k) => new.get(k).cloned(),
                None => Some(id.clone()),
            })
            .collect();
    };
    moved(&mut run.dismissed);
    for fix in &mut run.fixes {
        moved(&mut fix.findings);
    }
}
