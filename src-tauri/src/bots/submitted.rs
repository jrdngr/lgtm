//! Findings an agent publishes through the MCP server, stored as a reviewer's section of a
//! report so they show and act like a persona's.

use std::collections::HashMap;

use anyhow::ensure;

use super::output::{Finding, Location, Output};
use super::{BotOptions, BotRun, Part, Persona, Status, Step};
use crate::review::{RepoCtx, now_ms};

impl RepoCtx {
    /// Stores `review` (a verdict and findings, as a persona returns them) from `reviewer`.
    /// With `report`, it replaces that reviewer's section of that report, or adds one, and
    /// the user's dismissals and fixes follow findings with the same title and locations.
    /// Without, it's a new report against the workspace's current review. Returns the report
    /// and whether it's new.
    pub fn publish_findings(
        &self,
        workspace: Option<&str>,
        report: Option<&str>,
        reviewer: &str,
        review: serde_json::Value,
    ) -> anyhow::Result<(BotRun, bool)> {
        let output = Output::parse(Step::Reports, Some(reviewer), review)?;
        let Some(label) = report else {
            return Ok((self.new_report(workspace, reviewer, output)?, true));
        };
        let id = self.find_report(workspace, label)?.id;
        ensure!(
            !self.bots.is_active(id),
            "report {label} is running its reviewers; publish when it's done"
        );
        self.edit_run(id, |run| replace_section(run, reviewer, output))?;
        Ok((self.bot_run(id)?, false))
    }

    fn new_report(
        &self,
        workspace: Option<&str>,
        reviewer: &str,
        output: Output,
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
            options: BotOptions::default(),
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
        run.id = self.store.lock().unwrap().add_bot_run(&run)?;
        Ok(run)
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
