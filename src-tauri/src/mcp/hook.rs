//! `lgtm hook`: what Claude Code runs on its hook events. Inside an lgtm terminal it tells the
//! app how the terminal's agent stands, for its workspace's row. Anywhere else it does nothing.
//! It never fails or prints, so it can't get in Claude Code's way.

use std::io::{Read, Write};
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
    let Some(state) = serde_json::from_str(&input).ok().as_ref().and_then(state) else {
        return 0;
    };
    let report = HookReport {
        root,
        terminal,
        state,
    };
    if let Ok(mut stream) = UnixStream::connect(socket) {
        stream.set_write_timeout(Some(Duration::from_secs(1))).ok();
        let line = serde_json::json!({ "hook": report });
        writeln!(stream, "{line}").ok();
    }
    0
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
}
