//! Findings an agent publishes through the MCP server, stored as its section of a report.

use std::collections::HashMap;

use anyhow::Context;

use super::output::{Action, Finding, Location, Minimal, Output, Question, Review};
use super::{BotRun, Part, Persona};
use crate::review::{RepoCtx, now_ms};

/// An agent's own suggested fixes, naming findings by their 1-based place in what it published.
pub struct Summary {
    pub task: String,
    pub fixes: Vec<(String, Vec<u32>)>,
    pub off_task: Vec<u32>,
    pub questions: Vec<(String, Vec<u32>)>,
}

impl Summary {
    /// The suggested fixes, with places turned into the finding ids `Review::parse` gives them
    /// once it sorts them by severity.
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
    /// Stores `review` (a verdict and findings) from `reviewer`, with the agent's own suggested
    /// fixes when it gives them. With `report`, it replaces that reviewer's section of that
    /// report, or adds one, and the user's dismissals follow findings with the same title and
    /// locations. Without, it's a new report against the workspace's current review. Returns
    /// the report and whether it's new.
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
        let output = Output::Review(Review::parse(reviewer, review)?);
        let Some(label) = report else {
            return Ok((self.new_report(workspace, reviewer, output, filter)?, true));
        };
        let id = self.find_report(workspace, label)?.id;
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
            parts: vec![Part {
                persona: Some(reviewer.to_string()),
                output,
            }],
            created_at: now_ms() as f64,
            dismissed: vec![],
        };
        set_filter(&mut run, filter);
        run.id = self.store.lock().unwrap().add_bot_run(&run)?;
        Ok(run)
    }
}

/// Makes the agent's suggested fixes the report's, or drops the old ones, which no longer
/// match the findings.
fn set_filter(run: &mut BotRun, filter: Option<Minimal>) {
    run.parts.retain(|p| matches!(p.output, Output::Review(_)));
    if let Some(f) = filter {
        run.parts.push(Part {
            persona: None,
            output: Output::Minimal(f),
        });
    }
}

fn persona(reviewer: &str) -> Persona {
    Persona {
        name: reviewer.to_string(),
        description: "an agent in a terminal".into(),
    }
}

/// What a finding is for carrying the user's marks across a re-publish.
type Key = (String, Vec<Location>);

fn key(f: &Finding) -> Key {
    (f.title.clone(), f.locations.clone())
}

fn findings(part: &Part) -> &[Finding] {
    match &part.output {
        Output::Review(r) => &r.findings,
        _ => &[],
    }
}

/// Puts `output` in place of `reviewer`'s section of `run`, or adds it as a new section, and
/// moves dismissals of the old section's findings to their matches in the new one.
fn replace_section(run: &mut BotRun, reviewer: &str, output: Output) {
    let part = Part {
        persona: Some(reviewer.to_string()),
        output,
    };
    let at = run
        .parts
        .iter()
        .position(|p| p.persona.as_deref() == Some(reviewer));
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
    run.dismissed = run
        .dismissed
        .iter()
        .filter_map(|id| match old.get(id) {
            Some(k) => new.get(k).cloned(),
            None => Some(id.clone()),
        })
        .collect();
}
