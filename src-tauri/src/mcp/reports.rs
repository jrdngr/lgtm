//! Reports as agents read them.

use serde_json::{Value, json};

use crate::bots::BotRun;
use crate::bots::output::{Finding, Output};

/// The findings the report shows first: the synthesis when there is one, otherwise each
/// reviewer's own.
fn shown(run: &BotRun) -> Vec<&Finding> {
    let outputs = || run.parts.iter().filter_map(|p| p.output.as_ref());
    let synthesis = outputs().find_map(|o| match o {
        Output::Synthesis(s) => Some(s.findings.iter().collect()),
        _ => None,
    });
    synthesis.unwrap_or_else(|| {
        outputs()
            .filter_map(|o| match o {
                Output::Review(r) => Some(&r.findings),
                _ => None,
            })
            .flatten()
            .collect()
    })
}

/// A line per report for `list_reports`.
pub fn summary(run: &BotRun) -> Value {
    let findings = shown(run);
    let open = findings
        .iter()
        .filter(|f| run.finding_state(&f.id) == "open")
        .count();
    json!({
        "id": run.label(),
        "revset": run.revset,
        "reviewers": run.personas.iter().map(|p| &p.name).collect::<Vec<_>>(),
        "findings": findings.len(),
        "open": open,
        "running": run.active,
        "createdAt": run.created_at,
    })
}

/// The report with each finding's state, for `get_report`.
pub fn report(run: &BotRun) -> Result<Value, String> {
    let verdicts: Vec<_> = run
        .parts
        .iter()
        .filter_map(|p| match (&p.persona, &p.output) {
            (Some(who), Some(Output::Review(r))) => Some(json!({ "reviewer": who, "verdict": r.verdict })),
            _ => None,
        })
        .collect();
    let mut findings = Vec::new();
    for f in shown(run) {
        let mut value = serde_json::to_value(f).map_err(|e| e.to_string())?;
        value["state"] = run.finding_state(&f.id).into();
        findings.push(value);
    }
    let filter = run.parts.iter().rev().find_map(|p| match &p.output {
        Some(Output::Minimal(m)) if p.status == crate::bots::Status::Done => Some(m),
        _ => None,
    });
    Ok(json!({
        "id": run.label(),
        "revset": run.revset,
        "verdicts": verdicts,
        "findings": findings,
        "suggestedFixes": filter,
    }))
}
