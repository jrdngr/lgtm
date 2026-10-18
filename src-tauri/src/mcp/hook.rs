//! `lgtm hook`: what Claude Code runs on its hook events. Inside an lgtm terminal it tells the
//! app how the terminal's agent stands, for its workspace's row, and on a prompt prints the
//! context the app adds when the terminal is linked to a PR review. Anywhere else it does
//! nothing. It never fails, so it can't get in Claude Code's way.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;

/// The hook events lgtm listens to.
pub const EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// How a terminal's agent stands after a hook event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AgentHookState {
    Working,
    Input,
    Done,
    Failed,
    Idle,
    Gone,
}

/// What the hook sends the app, as the connection's only line.
#[derive(Debug, Serialize, Deserialize)]
pub(super) struct HookReport {
    /// The repo's main workspace root (`LGTM_ROOT`).
    pub root: String,
    /// The terminal panel (`LGTM_TERMINAL`).
    pub terminal: String,
    pub state: AgentHookState,
    /// The prompt the user submitted, on `UserPromptSubmit`.
    #[serde(default)]
    pub prompt: Option<String>,
    /// The agent's last message, on `Stop`.
    #[serde(default)]
    pub reply: Option<String>,
    /// Claude Code's session id, for resuming it.
    #[serde(default)]
    pub session: Option<String>,
    /// The user ended the session, so it isn't one to resume. A session the app stopped by
    /// closing its terminal ends with reason `other`.
    #[serde(default)]
    pub ended: bool,
}

/// The state a hook event's input means, if any.
fn state(input: &serde_json::Value) -> Option<AgentHookState> {
    use AgentHookState::*;
    Some(match input["hook_event_name"].as_str()? {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => Working,
        "PermissionRequest" => Input,
        "Notification" => match input["notification_type"].as_str()? {
            "permission_prompt" | "elicitation_dialog" => Input,
            _ => return None,
        },
        "Stop" => Done,
        "StopFailure" => Failed,
        "SessionStart" => Idle,
        "SessionEnd" => Gone,
        _ => return None,
    })
}

/// Whether the event is the user ending the session, by exiting or clearing it.
fn ended(input: &serde_json::Value) -> bool {
    input["hook_event_name"] == "SessionEnd" && input["reason"] != "other"
}

/// The prompt a `UserPromptSubmit` event carries.
fn prompt(input: &serde_json::Value) -> Option<String> {
    (input["hook_event_name"] == "UserPromptSubmit")
        .then(|| input["prompt"].as_str().map(str::to_string))
        .flatten()
}

/// The agent's last message on a `Stop` event: the event's own copy, else the last text in
/// its transcript since the user last spoke.
fn reply(input: &serde_json::Value) -> Option<String> {
    if input["hook_event_name"] != "Stop" {
        return None;
    }
    if let Some(text) = input["last_assistant_message"].as_str() {
        return Some(text.to_string());
    }
    let transcript = std::fs::read_to_string(input["transcript_path"].as_str()?).ok()?;
    last_text(&transcript)
}

/// The text of the last assistant entry in a JSONL transcript that has any, unless the user
/// spoke after it.
fn last_text(transcript: &str) -> Option<String> {
    for line in transcript.lines().rev() {
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let content = &entry["message"]["content"];
        match entry["type"].as_str() {
            Some("user") if content.is_string() => return None,
            Some("assistant") => {
                let text: Vec<&str> = content
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect();
                if !text.is_empty() {
                    return Some(text.join("\n\n"));
                }
            }
            _ => {}
        }
    }
    None
}

pub fn main() -> i32 {
    let var = |name| std::env::var(name).ok().filter(|v: &String| !v.is_empty());
    let (Some(socket), Some(root), Some(terminal)) =
        (var("LGTM_SOCKET"), var("LGTM_ROOT"), var("LGTM_TERMINAL"))
    else {
        return 0;
    };
    let mut input = String::new();
    std::io::stdin()
        .take(1 << 20)
        .read_to_string(&mut input)
        .ok();
    let Ok(input) = serde_json::from_str::<serde_json::Value>(&input) else {
        return 0;
    };
    let Some(state) = state(&input) else {
        return 0;
    };
    let report = HookReport {
        root,
        terminal,
        state,
        prompt: prompt(&input),
        reply: reply(&input),
        session: input["session_id"].as_str().map(str::to_string),
        ended: ended(&input),
    };
    let asks = report.prompt.is_some();
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return 0;
    };
    stream.set_write_timeout(Some(Duration::from_secs(1))).ok();
    let line = serde_json::json!({ "hook": report });
    if writeln!(stream, "{line}").is_err() || !asks {
        return 0;
    }
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    let mut answer = String::new();
    BufReader::new(stream).read_line(&mut answer).ok();
    if let Some(output) = prompt_output(&answer) {
        println!("{output}");
    }
    0
}

/// What Claude Code reads from a `UserPromptSubmit` hook to add context, from the app's
/// answer, if it has any to add.
fn prompt_output(answer: &str) -> Option<serde_json::Value> {
    let answer: serde_json::Value = serde_json::from_str(answer).ok()?;
    let context = answer["context"].as_str()?;
    Some(serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": context,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentHookState::*;

    fn of(json: &str) -> Option<AgentHookState> {
        state(&serde_json::from_str(json).unwrap())
    }

    #[test]
    fn maps_hook_events_to_agent_states() {
        assert_eq!(of(r#"{"hook_event_name":"UserPromptSubmit"}"#), Some(Working));
        assert_eq!(of(r#"{"hook_event_name":"PermissionRequest"}"#), Some(Input));
        assert_eq!(
            of(r#"{"hook_event_name":"Notification","notification_type":"permission_prompt"}"#),
            Some(Input)
        );
        assert_eq!(
            of(r#"{"hook_event_name":"Notification","notification_type":"idle_prompt"}"#),
            None
        );
        assert_eq!(of(r#"{"hook_event_name":"StopFailure"}"#), Some(Failed));
        assert_eq!(of(r#"{"hook_event_name":"SubagentStop"}"#), None);
        assert_eq!(of("{}"), None);
    }

    #[test]
    fn adds_the_apps_context_to_a_prompt_only_when_there_is_some() {
        let out = prompt_output(r#"{"context":"lgtm: thread 3"}"#).unwrap();
        assert_eq!(
            out["hookSpecificOutput"]["additionalContext"],
            "lgtm: thread 3"
        );
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "UserPromptSubmit");
        assert!(prompt_output(r#"{"context":null}"#).is_none());
        assert!(prompt_output("").is_none());
    }

    #[test]
    fn only_a_session_the_user_ends_is_ended() {
        let end = |reason: &str| {
            ended(&serde_json::json!({ "hook_event_name": "SessionEnd", "reason": reason }))
        };
        assert!(end("prompt_input_exit"));
        assert!(end("clear"));
        assert!(!end("other"));
        assert!(!ended(&serde_json::json!({ "hook_event_name": "Stop" })));
    }

    #[test]
    fn carries_the_prompt_and_the_agents_last_message() {
        let input = |json: &str| serde_json::from_str::<serde_json::Value>(json).unwrap();
        let submit = input(r#"{"hook_event_name":"UserPromptSubmit","prompt":"Is it?"}"#);
        assert_eq!(prompt(&submit).as_deref(), Some("Is it?"));
        assert_eq!(reply(&submit), None);
        let stop = input(r#"{"hook_event_name":"Stop","last_assistant_message":"Yes."}"#);
        assert_eq!(reply(&stop).as_deref(), Some("Yes."));
        assert_eq!(prompt(&stop), None);
    }

    #[test]
    fn finds_the_last_text_in_a_transcript_since_the_user_spoke() {
        let transcript = [
            r#"{"type":"user","message":{"content":"Is it?"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Checking."}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Yes."}]}}"#,
            r#"{"type":"system"}"#,
        ];
        assert_eq!(last_text(&transcript.join("\n")).as_deref(), Some("Yes."));
        let unanswered = [transcript[1], r#"{"type":"user","message":{"content":"And?"}}"#];
        assert_eq!(last_text(&unanswered.join("\n")), None);
    }
}
