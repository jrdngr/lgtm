//! `lgtm hook`: what Claude Code runs on its hook events. Inside an lgtm terminal it tells the
//! app how the terminal's agent stands, for its workspace's row, and on a prompt prints the
//! context the app adds when the terminal is linked to a PR review. Anywhere else it does
//! nothing. It never fails, so it can't get in Claude Code's way.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::events::AgentHookState;
use crate::turns::Checkpoint;

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
    /// A file edit the agent just made, on `PostToolUse`.
    #[serde(default)]
    pub edit: Option<HookEdit>,
    /// The terminal's workspace (`LGTM_WORKSPACE`), absent for the main one.
    #[serde(default)]
    pub workspace: Option<String>,
    /// Where in the agent's turn the app snapshots the working copy, if anywhere.
    #[serde(default)]
    pub checkpoint: Option<Checkpoint>,
    /// The tool call a `PreToolUse` or `PostToolUse` event is about.
    #[serde(default)]
    pub tool_use: Option<String>,
}

/// Claude Code's tools that can change files.
const WRITERS: [&str; 5] = ["Edit", "Write", "MultiEdit", "NotebookEdit", "Bash"];

/// How long the hook waits for the app to snapshot the working copy.
const SNAPSHOT_WAIT: Duration = Duration::from_secs(15);

/// The file an edit tool wrote and the text it wrote there.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct HookEdit {
    pub path: String,
    pub text: String,
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

/// Where in a turn the event is, for the app to snapshot the working copy: a prompt, either
/// side of a tool that can change files, or the agent stopping.
fn checkpoint(input: &serde_json::Value) -> Option<Checkpoint> {
    let writes = || WRITERS.contains(&input["tool_name"].as_str().unwrap_or(""));
    match input["hook_event_name"].as_str()? {
        "UserPromptSubmit" => Some(Checkpoint::Start),
        "PreToolUse" if writes() => Some(Checkpoint::Before),
        "PostToolUse" if writes() => Some(Checkpoint::After),
        "Stop" | "StopFailure" => Some(Checkpoint::End),
        _ => None,
    }
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

/// The edit a `PostToolUse` event of Claude Code's file tools made: the new text of an Edit
/// or each of a MultiEdit's edits, or a Write's whole content.
fn edit(input: &serde_json::Value) -> Option<HookEdit> {
    if input["hook_event_name"] != "PostToolUse" {
        return None;
    }
    let tool = &input["tool_input"];
    let text = match input["tool_name"].as_str()? {
        "Edit" => tool["new_string"].as_str()?.to_string(),
        "Write" => tool["content"].as_str()?.to_string(),
        "MultiEdit" => tool["edits"]
            .as_array()?
            .iter()
            .filter_map(|e| e["new_string"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    Some(HookEdit {
        path: tool["file_path"].as_str()?.to_string(),
        text,
    })
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
        edit: edit(&input),
        workspace: var("LGTM_WORKSPACE"),
        checkpoint: checkpoint(&input),
        tool_use: input["tool_use_id"].as_str().map(str::to_string),
    };
    // The app answers once it has the prompt's context, and after any snapshot, which has
    // to land before the tool runs.
    let wait = match (report.checkpoint, report.prompt.is_some()) {
        (Some(_), _) => SNAPSHOT_WAIT,
        (None, true) => Duration::from_secs(2),
        (None, false) => Duration::ZERO,
    };
    let Ok(mut stream) = UnixStream::connect(socket) else {
        return 0;
    };
    stream.set_write_timeout(Some(Duration::from_secs(1))).ok();
    let line = serde_json::json!({ "hook": report });
    if writeln!(stream, "{line}").is_err() || wait.is_zero() {
        return 0;
    }
    stream.set_read_timeout(Some(wait)).ok();
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
        assert_eq!(
            of(r#"{"hook_event_name":"UserPromptSubmit"}"#),
            Some(Working)
        );
        assert_eq!(
            of(r#"{"hook_event_name":"PermissionRequest"}"#),
            Some(Input)
        );
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
        assert_eq!(
            out["hookSpecificOutput"]["hookEventName"],
            "UserPromptSubmit"
        );
        assert!(prompt_output(r#"{"context":null}"#).is_none());
        assert!(prompt_output("").is_none());
    }

    #[test]
    fn snapshots_around_prompts_stops_and_tools_that_can_change_files() {
        let at = |event: &str, tool: &str| {
            checkpoint(&serde_json::json!({ "hook_event_name": event, "tool_name": tool }))
        };
        assert_eq!(at("UserPromptSubmit", ""), Some(Checkpoint::Start));
        assert_eq!(at("PreToolUse", "Edit"), Some(Checkpoint::Before));
        assert_eq!(at("PostToolUse", "Bash"), Some(Checkpoint::After));
        assert_eq!(at("PreToolUse", "Read"), None);
        assert_eq!(at("PostToolUse", "Grep"), None);
        assert_eq!(at("StopFailure", ""), Some(Checkpoint::End));
        assert_eq!(at("Notification", ""), None);
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
    fn carries_what_the_file_tools_wrote() {
        let input = |json: serde_json::Value| edit(&json);
        let post = |tool: &str, tool_input: serde_json::Value| {
            serde_json::json!({
                "hook_event_name": "PostToolUse", "tool_name": tool, "tool_input": tool_input,
            })
        };
        let at = |text: &str| {
            Some(HookEdit {
                path: "/r/a.rs".into(),
                text: text.into(),
            })
        };
        assert_eq!(
            input(post("Edit", serde_json::json!({"file_path": "/r/a.rs", "new_string": "x"}))),
            at("x")
        );
        assert_eq!(
            input(post("Write", serde_json::json!({"file_path": "/r/a.rs", "content": "all"}))),
            at("all")
        );
        let multi = serde_json::json!({
            "file_path": "/r/a.rs",
            "edits": [{"new_string": "one"}, {"new_string": "two"}],
        });
        assert_eq!(input(post("MultiEdit", multi)), at("one\ntwo"));
        assert_eq!(input(post("Read", serde_json::json!({"file_path": "/r/a.rs"}))), None);
        let pre = serde_json::json!({
            "hook_event_name": "PreToolUse", "tool_name": "Edit",
            "tool_input": {"file_path": "/r/a.rs", "new_string": "x"},
        });
        assert_eq!(input(pre), None);
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
        let unanswered = [
            transcript[1],
            r#"{"type":"user","message":{"content":"And?"}}"#,
        ];
        assert_eq!(last_text(&unanswered.join("\n")), None);
    }
}
