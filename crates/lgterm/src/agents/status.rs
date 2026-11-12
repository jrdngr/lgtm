//! What an agent in a terminal is doing, and what its hooks say about it.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::kind::Kind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Idle,
    Working,
    Blocked,
    Done,
}

impl Status {
    /// How much a status wants attention. Collapsed rows show their most urgent one.
    pub fn urgency(self) -> u8 {
        match self {
            Status::Idle => 0,
            Status::Working => 1,
            Status::Done => 2,
            Status::Blocked => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Status::Idle => "idle",
            Status::Working => "working",
            Status::Blocked => "blocked",
            Status::Done => "done",
        }
    }

    /// Whether the agent is waiting for its next prompt.
    pub fn ready(self) -> bool {
        matches!(self, Status::Idle | Status::Done)
    }
}

/// What one hook report says about an agent.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub kind: Kind,
    /// None when the report doesn't change the status.
    pub status: Option<Status>,
    pub session: Option<String>,
    /// The agent's session ended.
    pub ended: bool,
    /// The same process moved on to this session: `/clear`, `/resume` or a compaction.
    pub next_session: bool,
}

/// Reads a Claude Code hook payload, or a Codex `notify` payload.
pub fn report(event: &Value) -> Option<Report> {
    if let Some(kind) = event["type"].as_str() {
        return (kind == "agent-turn-complete").then(|| Report {
            kind: Kind::Codex,
            status: Some(Status::Done),
            session: event["thread-id"].as_str().map(str::to_owned),
            ended: false,
            next_session: true,
        });
    }
    let name = event["hook_event_name"].as_str()?;
    let status = match name {
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => Some(Status::Working),
        "PermissionRequest" => Some(Status::Blocked),
        "Notification" => match event["notification_type"].as_str() {
            Some("permission_prompt" | "elicitation_dialog") => Some(Status::Blocked),
            Some("idle_prompt") => Some(Status::Idle),
            _ => None,
        },
        "Stop" | "StopFailure" => Some(Status::Done),
        "SessionStart" => Some(Status::Idle),
        _ => None,
    };
    Some(Report {
        kind: Kind::Claude,
        status,
        session: event["session_id"].as_str().map(str::to_owned),
        // `/clear` and `/resume` end one session and start the next in the same process.
        ended: name == "SessionEnd"
            && !matches!(event["reason"].as_str(), Some("clear" | "resume")),
        next_session: name == "SessionStart"
            && matches!(
                event["source"].as_str(),
                Some("clear" | "resume" | "compact")
            ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn claude_hooks_map_to_statuses() {
        let r = |v| report(&v).unwrap().status;
        assert_eq!(
            r(json!({"hook_event_name": "PreToolUse"})),
            Some(Status::Working)
        );
        assert_eq!(
            r(json!({"hook_event_name": "Notification", "notification_type": "permission_prompt"})),
            Some(Status::Blocked)
        );
        assert_eq!(
            r(json!({"hook_event_name": "Notification", "notification_type": "idle_prompt"})),
            Some(Status::Idle)
        );
        assert_eq!(
            r(json!({"hook_event_name": "Notification", "notification_type": "auth_success"})),
            None
        );
        assert_eq!(r(json!({"hook_event_name": "Stop"})), Some(Status::Done));
    }

    #[test]
    fn hooks_carry_the_session_and_its_end() {
        let end = report(&json!({"hook_event_name": "SessionEnd", "session_id": "abc"})).unwrap();
        assert!(end.ended);
        assert_eq!(end.session.as_deref(), Some("abc"));
        assert_eq!(end.kind, Kind::Claude);
    }

    #[test]
    fn clearing_a_session_doesnt_end_the_agent() {
        let end = |reason| report(&json!({"hook_event_name": "SessionEnd", "reason": reason}));
        assert!(!end("clear").unwrap().ended);
        assert!(!end("resume").unwrap().ended);
        assert!(end("prompt_input_exit").unwrap().ended);
    }

    #[test]
    fn codex_turns_finish_as_done() {
        let r = report(&json!({"type": "agent-turn-complete", "thread-id": "th1"})).unwrap();
        assert_eq!(r.kind, Kind::Codex);
        assert_eq!(r.status, Some(Status::Done));
        assert_eq!(r.session.as_deref(), Some("th1"));
        assert_eq!(report(&json!({"type": "something-else"})), None);
    }
}
