//! Prompts for each step of a persona review. Earlier steps' findings go in as JSON, and every
//! step answers with JSON matching its schema in `output`.

use super::output::{Finding, Output, Severity};
use super::{BotRun, Part, Persona, Status, Step};
use crate::jj::Range;
use crate::workspace::Workspace;

/// Diffs longer than this are cut short, and the agents read the rest from the files.
const MAX_DIFF: usize = 200_000;

const READ_ONLY: &str = "You're in the repository and can read any file. Don't edit files or \
change anything.";

const LOCATIONS: &str = "Give every finding at least one location: a repository-relative path, \
with the first and last lines on the new side of the diff when it's about specific lines.";

const RUBRIC: &str = "Rate each finding by its real impact on this codebase as it's used, not \
by how central it is to your priority:

- critical: an outside party or an ordinary user can exploit it today, or it loses or corrupts \
data in normal use. For example, an auth bypass, exposed credentials, arbitrary file reads, or \
injection on a live path.
- high: a core flow is broken or wrong, an authorization check is missing, or a likely failure \
has serious consequences.
- medium: a real bug off the main path, a problem that will bite once nearby code changes, or \
a design issue with a clear ongoing cost.
- low: hygiene, a minor inefficiency, or a small inconsistency.
- info: worth knowing, with nothing to do.

Dead code, and paths no planned change would make reachable, are at most medium unless one \
trivial change makes them live. Rate an issue that another finding currently hides as it will \
be once that finding is fixed. Never cap missing authorization or exposed credentials on a path \
that serves requests.";

/// The change a run reviews, as the prompts describe it.
pub struct Target {
    context: String,
}

impl Target {
    pub async fn load(ws: &Workspace, run: &BotRun) -> anyhow::Result<Self> {
        let mut context = describe(ws, &run.revset, run.from.clone(), run.to.clone()).await?;
        if !run.context.trim().is_empty() {
            context = format!(
                "{context}\n\nWhat I want you to know about this change:\n\n{}",
                run.context.trim()
            );
        }
        Ok(Self { context })
    }
}

/// Describes the change from `from` to `to` for an agent: its commits, stat and diff.
pub async fn describe(
    ws: &Workspace,
    revset: &str,
    from: Option<String>,
    to: String,
) -> anyhow::Result<String> {
    let short = |c: &str| c.chars().take(8).collect::<String>();
    let span = match &from {
        Some(from) => format!("from commit {} to {}", short(from), short(&to)),
        None => format!("up to commit {}", short(&to)),
    };
    let range = Range { from, to };
    let (commits, stat, diff) = tokio::try_join!(
        ws.jj.descriptions(&range),
        ws.jj.diff_stat(&range),
        ws.jj.git_diff(&range),
    )?;
    Ok(context(revset, &span, &commits, &stat, &diff))
}

fn context(revset: &str, span: &str, commits: &str, stat: &str, diff: &str) -> String {
    let diff = if diff.len() > MAX_DIFF {
        let cut = diff[..diff.floor_char_boundary(MAX_DIFF)]
            .rfind('\n')
            .unwrap_or(0);
        format!(
            "{}\n(The diff is cut short here. Read the changed files for the rest.)",
            &diff[..cut]
        )
    } else {
        diff.to_string()
    };
    let fence = fence(&diff);
    format!(
        "The change under review is `{revset}` in this jj repository, {span}. Files you read \
         are the working copy, which may differ from those commits; where they disagree, the \
         diff is the change.\n\n\
         Its commits, oldest first:\n\n{commits}\n\nFiles changed:\n\n{stat}\n\n\
         The diff:\n\n{fence}diff\n{diff}\n{fence}"
    )
}

/// A code fence longer than any run of backticks in `text`.
fn fence(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}

fn persona_intro(p: &Persona) -> String {
    format!(
        "You are {name}, a code reviewer with one priority: {description}. Keep to it, but treat \
         the change's requirements, safety, and the evidence in the code as constraints, not \
         preferences.\n\nHow {name} works:\n\n{profile}",
        name = p.name,
        description = p.description.trim_end_matches('.'),
        profile = p.profile,
    )
}

/// The prompt for `step`, for `persona` when the step has one per persona. `resumed` is whether
/// it goes to the persona's own review session.
pub fn prompt(
    step: Step,
    persona: Option<&Persona>,
    target: &Target,
    run: &BotRun,
    resumed: bool,
) -> String {
    match (step, persona) {
        (Step::Reports, Some(p)) => review(p, target),
        (Step::Challenges, Some(p)) => challenge(p, target, run),
        (Step::Rebuttals, Some(p)) => rebuttal(p, target, run, resumed),
        (Step::Minimal, _) => minimal(target, run),
        _ => synthesis(target, run),
    }
}

fn review(p: &Persona, target: &Target) -> String {
    let off_lens = if p.off_lens {
        "Also report issues outside your priority, but only ones you'd rate critical or high. \
         Mark them off-lens, keep them to a title, location and one fact, and give them no fix."
    } else {
        "You have no single lens, so every finding is in-lens."
    };
    format!(
        "{intro}\n\nReview this change through your priority. {READ_ONLY}\n\n{ctx}\n\n\
         {RUBRIC}\n\n{off_lens} {LOCATIONS} Order findings by severity, most severe first. \
         Report conclusions and evidence, not the steps you took. If you find nothing, return \
         no findings and say so in the verdict.",
        intro = persona_intro(p),
        ctx = target.context,
    )
}

/// Persona findings at medium or above from anyone but `except`, for the challenge pass.
pub fn challengeable<'a>(run: &'a BotRun, except: &str) -> Vec<&'a Finding> {
    reviews(run)
        .filter(|(who, _)| *who != except)
        .flat_map(|(_, f)| f)
        .filter(|f| f.severity <= Severity::Medium)
        .collect()
}

fn challenge(p: &Persona, target: &Target, run: &BotRun) -> String {
    let stance = if run.options.adversarial {
        "Argue against every one of them, making the strongest case for the opposite."
    } else {
        "Challenge only the ones you think are wrong, overrated, or have the wrong fix, with \
         evidence. Leave out the ones you agree with."
    };
    format!(
        "{intro}\n\nOther reviewers reviewed the change below. Cross-review their medium and \
         higher findings through your priority. {stance} {READ_ONLY}\n\n{RUBRIC}\n\n{ctx}\n\n\
         Their findings, as JSON:\n\n{findings}",
        intro = persona_intro(p),
        ctx = target.context,
        findings = json(&challengeable(run, &p.name)),
    )
}

/// `persona`'s own findings, from its finished review.
fn own_findings<'a>(run: &'a BotRun, persona: &str) -> Vec<&'a Finding> {
    reviews(run)
        .filter(|(who, _)| *who == persona)
        .flat_map(|(_, f)| f)
        .collect()
}

/// The challenges to `persona`'s findings, with who made each and the finding's severity and
/// title, so the persona needn't work out which of its findings an id means.
pub fn challenges_to(run: &BotRun, persona: &str) -> Vec<serde_json::Value> {
    let prefix = format!("{persona}#");
    let own = own_findings(run, persona);
    let own = &own;
    done(run, Step::Challenges)
        .filter_map(|p| match &p.output {
            Some(Output::Challenges { challenges }) => Some((p, challenges)),
            _ => None,
        })
        .flat_map(|(p, cs)| {
            cs.iter()
                .filter(|c| c.finding.starts_with(&prefix))
                .map(move |c| {
                    let f = own.iter().find(|f| f.id == c.finding);
                    serde_json::json!({
                        "finding": c.finding,
                        "severity": f.map(|f| f.severity),
                        "title": f.map(|f| &f.title),
                        "by": p.persona,
                        "argument": c.argument,
                    })
                })
        })
        .collect()
}

/// Sent to the persona's own session when there is one, which already holds the change and its
/// findings. Without one, the prompt gives it them again.
fn rebuttal(p: &Persona, target: &Target, run: &BotRun, resumed: bool) -> String {
    let context = if resumed {
        String::new()
    } else {
        format!(
            "{intro}\n\nYou reviewed the change below through your priority. {READ_ONLY}\n\n\
             {ctx}\n\nYour findings, as JSON:\n\n{findings}\n\n",
            intro = persona_intro(p),
            ctx = target.context,
            findings = json(&own_findings(run, &p.name)),
        )
    };
    format!(
        "{context}Other reviewers cross-reviewed your findings. Your findings have ids \
         `{name}#1`, `{name}#2`, … in order of severity, most severe first. For each challenge \
         below, concede, defend, or partly concede, with evidence.\n\nThe challenges, as JSON:\
         \n\n{challenges}",
        name = p.name,
        challenges = json(&challenges_to(run, &p.name)),
    )
}

fn synthesis(target: &Target, run: &BotRun) -> String {
    let cross = if run.options.cross_review {
        format!(
            "The personas cross-reviewed each other's findings. Rule on every challenged \
             finding: keep it, drop it, or change its severity. Base each ruling on the \
             evidence and the code, not on how many personas took each side, and check any \
             claim that a path is exploitable or unreachable before relying on it. Record every \
             ruling, and put the ones that changed a finding in its `contested` too. Dropped \
             findings don't appear in the merged findings.\n\nThe challenges, as JSON:\n\n\
             {}\n\nThe rebuttals, as JSON:\n\n{}\n\n",
            json(&outputs(run, Step::Challenges)),
            json(&outputs(run, Step::Rebuttals)),
        )
    } else {
        String::new()
    };
    format!(
        "You combine several reviewers' findings on one change into a single set. Each \
         reviewer had its own priority. {READ_ONLY}\n\n{ctx}\n\n{RUBRIC}\n\n\
         How to merge:\n\n\
         - Merge findings about the same problem into one. List every persona finding it \
         absorbs in `sources`, and every persona that raised it in `raisedBy`, with that \
         persona's severity and whether it was in its lens.\n\
         - When an off-lens finding repeats another persona's in-lens one, keep the in-lens \
         severity unless the rubric says otherwise. When severities differ, rule by the rubric \
         and say how in `contested`.\n\
         - When one finding absorbs others (for example, \"delete the dead code\"), name each \
         absorbed problem in its facts so none disappears.\n\
         - Keep each persona's most significant findings, even ones you downgrade.\n\
         - Don't decide by majority. One well-supported concern can outweigh several weaker \
         ones.\n\
         - Number the merged findings \"1\", \"2\", … in descending severity.\n\
         - Quick wins are findings of any severity whose fix takes a line or two.\n\
         - Questions are ambiguities in the requirements that block a correct fix.\n\
         - Fix first is three to seven ordered actions, each with the findings it closes.\n\n\
         {LOCATIONS}\n\n{cross}The personas' findings, as JSON:\n\n{findings}",
        ctx = target.context,
        findings = json(&reviews(run).collect::<Vec<_>>()),
    )
}

fn minimal(target: &Target, run: &BotRun) -> String {
    let findings = match done(run, Step::Synthesis).find_map(|p| p.output.as_ref()) {
        Some(Output::Synthesis(s)) => json(&s.findings),
        _ => json(&reviews(run).flat_map(|(_, f)| f).collect::<Vec<_>>()),
    };
    format!(
        "You pick which review findings matter to the task a change is meant to complete. Work \
         out the task from the commit descriptions and the change itself. {READ_ONLY}\n\n\
         {ctx}\n\n\
         Keep a finding if fixing it is needed for the task to be done correctly, or if it's a \
         cheap fix to code the change adds or touches that would otherwise ship a lasting \
         defect, such as unwanted public API or wrong docs. Leave out everything else, \
         including hygiene nitpicks and findings that another kept finding's fix already \
         resolves. List the critical and high findings you left out as off-task. Then \
         recommend what to fix.\n\nThe findings, as JSON:\n\n{findings}",
        ctx = target.context,
    )
}

fn done(run: &BotRun, step: Step) -> impl Iterator<Item = &Part> {
    run.parts
        .iter()
        .filter(move |p| p.step == step && p.status == Status::Done)
}

/// Each persona's name and findings, from its finished review.
fn reviews(run: &BotRun) -> impl Iterator<Item = (&str, &Vec<Finding>)> {
    done(run, Step::Reports).filter_map(|p| match (&p.persona, &p.output) {
        (Some(who), Some(Output::Review(r))) => Some((who.as_str(), &r.findings)),
        _ => None,
    })
}

/// Each finished part of `step`, with who it's from.
fn outputs(run: &BotRun, step: Step) -> Vec<serde_json::Value> {
    done(run, step)
        .map(|p| serde_json::json!({ "from": p.persona, "output": p.output }))
        .collect()
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_outrun_backticks_in_the_diff() {
        assert_eq!(fence("plain"), "```");
        assert_eq!(fence("a ```` b"), "`````");
    }

    #[test]
    fn long_diffs_are_cut_at_a_line() {
        let diff = "+line\n".repeat(MAX_DIFF / 6 + 10);
        let ctx = context("@", "up to x", "", "", &diff);
        assert!(ctx.contains("+line\n(The diff is cut short here."));
        assert!(ctx.len() < MAX_DIFF + 1_000);
    }
}
