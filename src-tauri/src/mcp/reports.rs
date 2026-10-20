//! Reports as agents read them.

use serde_json::{Value, json};

use crate::store::BotRun;
use crate::store::report::output::{Finding, Output};

/// Every reviewer's findings.
fn findings(run: &BotRun) -> Vec<&Finding> {
    run.parts
        .iter()
        .filter_map(|p| match &p.output {
            Output::Review(r) => Some(&r.findings),
            _ => None,
        })
        .flatten()
        .collect()
}

/// A line per report for `list_reports`.
pub fn summary(run: &BotRun) -> Value {
    let findings = findings(run);
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
        "createdAt": run.created_at,
    })
}

/// The report with each finding's state, for `get_report`.
pub fn report(run: &BotRun) -> Result<Value, String> {
    let verdicts: Vec<_> = run
        .parts
        .iter()
        .filter_map(|p| match (&p.persona, &p.output) {
            (Some(who), Output::Review(r)) => {
                Some(json!({ "reviewer": who, "verdict": r.verdict }))
            }
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for f in findings(run) {
        let mut value = serde_json::to_value(f).map_err(|e| e.to_string())?;
        value["state"] = run.finding_state(&f.id).into();
        out.push(value);
    }
    let fixes = run.parts.iter().rev().find_map(|p| match &p.output {
        Output::Minimal(m) => Some(m),
        _ => None,
    });
    Ok(json!({
        "id": run.label(),
        "revset": run.revset,
        "verdicts": verdicts,
        "findings": out,
        "suggestedFixes": fixes,
    }))
}
