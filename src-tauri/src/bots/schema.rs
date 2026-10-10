//! The JSON schema each step's agent must answer with, matching the types in `output`.

use serde_json::{Value, json};

use super::Step;

fn object(props: Value, required: &[&str]) -> Value {
    json!({ "type": "object", "properties": props, "required": required, "additionalProperties": false })
}

fn string(description: &str) -> Value {
    json!({ "type": "string", "description": description })
}

fn strings(description: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": description })
}

fn severity() -> Value {
    json!({ "type": "string", "enum": ["critical", "high", "medium", "low", "info"] })
}

fn finding(merged: bool) -> Value {
    let mut props = json!({
        "severity": severity(),
        "title": string("A concise but complete headline."),
        "locations": {
            "type": "array",
            "description": "Where it is. Paths are repository-relative; lines are on the new side.",
            "items": object(json!({
                "path": string("Repository-relative path."),
                "start": { "type": ["integer", "null"], "description": "First line, new side." },
                "end": { "type": ["integer", "null"], "description": "Last line, new side." },
            }), &["path", "start", "end"]),
        },
        "facts": strings("Verifiable facts, one per item."),
        "explanation": string("A short paragraph: the issue, its consequence, anything else that matters."),
        "fix": { "type": ["string", "null"], "description": "The suggested resolution. Null for off-lens findings." },
        "offLens": { "type": "boolean", "description": "Outside your lens." },
    });
    let mut required = vec![
        "severity",
        "title",
        "locations",
        "facts",
        "explanation",
        "fix",
        "offLens",
    ];
    if merged {
        let extra = json!({
            "id": string("This finding's number, \"1\", \"2\", … in descending severity."),
            "sources": strings("Ids of the persona findings this one combines."),
            "raisedBy": {
                "type": "array",
                "items": object(json!({
                    "persona": string("The persona's name."),
                    "severity": severity(),
                    "inLens": { "type": "boolean" },
                }), &["persona", "severity", "inLens"]),
            },
            "contested": { "type": ["string", "null"], "description": "How personas disagreed and the ruling, or null." },
        });
        props
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        required.extend(["id", "sources", "raisedBy", "contested"]);
    }
    object(props, &required)
}

fn list(item: Value) -> Value {
    json!({ "type": "array", "items": item })
}

/// The schema `step`'s agent must answer with.
pub fn schema(step: Step) -> Value {
    match step {
        Step::Reports => object(
            json!({
                "verdict": string("One line: what to do with this change."),
                "findings": list(finding(false)),
            }),
            &["verdict", "findings"],
        ),
        Step::Challenges => object(
            json!({ "challenges": list(object(json!({
                "finding": string("The challenged finding's id."),
                "argument": string("Your argument and evidence."),
            }), &["finding", "argument"])) }),
            &["challenges"],
        ),
        Step::Rebuttals => object(
            json!({ "responses": list(object(json!({
                "finding": string("Your finding's id."),
                "stance": { "type": "string", "enum": ["concede", "defend", "partlyConcede"] },
                "evidence": string("Why, with evidence."),
            }), &["finding", "stance", "evidence"])) }),
            &["responses"],
        ),
        Step::Synthesis => object(
            json!({
                "verdict": string("One line: what to do with this change."),
                "findings": list(finding(true)),
                "rulings": list(object(json!({
                    "finding": string("The challenged persona finding's id."),
                    "outcome": { "type": "string", "enum": ["kept", "dropped", "downgraded", "upgraded"] },
                    "severity": { "anyOf": [severity(), { "type": "null" }], "description": "The new severity, if it changed." },
                    "reason": string("The evidence the ruling rests on."),
                }), &["finding", "outcome", "severity", "reason"])),
                "quickWins": list(object(json!({
                    "finding": string("A merged finding's id."),
                    "fix": string("The fix, in one line."),
                }), &["finding", "fix"])),
                "questions": list(object(json!({
                    "question": string("An ambiguity in the requirements that blocks a correct fix."),
                    "findings": strings("Ids of the merged findings it affects."),
                }), &["question", "findings"])),
                "fixFirst": list(object(json!({
                    "action": string("One action."),
                    "closes": strings("Ids of the merged findings it closes."),
                }), &["action", "closes"])),
            }),
            &[
                "verdict",
                "findings",
                "rulings",
                "quickWins",
                "questions",
                "fixFirst",
            ],
        ),
        Step::Minimal => object(
            json!({
                "task": string("What the change is meant to do, in a few words."),
                "kept": strings("Ids of the merged findings that matter to the task."),
                "offTask": strings("Ids of critical and high findings left out as unrelated to the task."),
                "recommendations": string("What to fix, in a few sentences or bullets."),
            }),
            &["task", "kept", "offTask", "recommendations"],
        ),
    }
}
