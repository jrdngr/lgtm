//! What an agent in a terminal is doing, from its hooks or, failing that, its screen.

use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Claude,
    Codex,
}

impl Kind {
    pub fn from_process(name: &str) -> Option<Kind> {
        match name {
            "claude" => Some(Kind::Claude),
            "codex" => Some(Kind::Codex),
            _ => None,
        }
    }

    /// The agent a process runs, from its executable path and arguments: the `claude` or `codex`
    /// command, the native installer's `claude/versions/<version>`, or either npm package run by
    /// node.
    pub fn from_command(exe: &str, argv: &[String]) -> Option<Kind> {
        let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_owned();
        let mut words = vec![exe, argv.first().map_or("", String::as_str)];
        if words.iter().any(|w| base(w) == "node") {
            words.extend(argv.get(1).map(String::as_str));
        }
        words.into_iter().find_map(|w| {
            Kind::from_process(&base(w)).or_else(|| {
                if w.contains("/claude/versions/") || w.contains("@anthropic-ai/claude-code/") {
                    Some(Kind::Claude)
                } else if w.contains("@openai/codex/") {
                    Some(Kind::Codex)
                } else {
                    None
                }
            })
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Claude => "claude",
            Kind::Codex => "codex",
        }
    }

    /// The command that picks a session back up.
    pub fn resume(self, session: &str) -> String {
        match self {
            Kind::Claude => format!("claude --resume {session}"),
            Kind::Codex => format!("codex resume {session}"),
        }
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
}

/// Reads a Claude Code hook payload, or a Codex `notify` payload.
pub fn report(event: &Value) -> Option<Report> {
    if let Some(kind) = event["type"].as_str() {
        return (kind == "agent-turn-complete").then(|| Report {
            kind: Kind::Codex,
            status: Some(Status::Done),
            session: event["thread-id"].as_str().map(str::to_owned),
            ended: false,
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
    })
}

/// What an agent's screen shows it doing, for agents whose hooks don't say.
pub fn from_screen(kind: Kind, screen: &str) -> Option<Status> {
    let blocked: &[&str] = match kind {
        Kind::Claude => &["Do you want to proceed?", "Do you want to make this edit"],
        Kind::Codex => &[
            "Would you like to run the following command?",
            "Would you like to make the following edits?",
            "Allow command?",
        ],
    };
    if blocked.iter().any(|b| screen.contains(b)) {
        return Some(Status::Blocked);
    }
    if screen.contains("esc to interrupt") {
        return Some(Status::Working);
    }
    None
}

/// Whether Claude's screen shows it was just stopped, as when the user turns down a
/// permission prompt. Only the bottom lines count, so an older stop further up doesn't.
pub fn claude_interrupted(screen: &str) -> bool {
    screen
        .lines()
        .rev()
        .filter(|l| !l.trim().is_empty())
        .take(8)
        .any(|l| l.contains("Interrupted by user") || l.contains("What should Claude do instead"))
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

    #[test]
    fn screens_show_work_and_prompts() {
        let working = "◦ Working (1m 12s • esc to interrupt)";
        assert_eq!(from_screen(Kind::Codex, working), Some(Status::Working));
        let ask = "Would you like to run the following command?\n esc to interrupt";
        assert_eq!(from_screen(Kind::Codex, ask), Some(Status::Blocked));
        assert_eq!(from_screen(Kind::Claude, "> "), None);
    }

    #[test]
    fn agents_are_found_by_their_command() {
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let versions = "/Users/j/.local/share/claude/versions/2.1.3";
        assert_eq!(
            Kind::from_command(versions, &argv(&["2.1.3"])),
            Some(Kind::Claude)
        );
        let npm = "/opt/lib/node_modules/@anthropic-ai/claude-code/cli.js";
        assert_eq!(
            Kind::from_command("/opt/bin/node", &argv(&["node", npm])),
            Some(Kind::Claude)
        );
        let codex = "/opt/lib/node_modules/@openai/codex/bin/codex.js";
        assert_eq!(
            Kind::from_command("/opt/bin/node", &argv(&["node", codex])),
            Some(Kind::Codex)
        );
        assert_eq!(
            Kind::from_command("/usr/bin/less", &argv(&["less", "claude"])),
            None
        );
        assert_eq!(
            Kind::from_command("/opt/bin/node", &argv(&["node", "server.js"])),
            None
        );
    }

    #[test]
    fn claude_stops_count_only_at_the_bottom_of_the_screen() {
        let stopped = "⏺ Bash(rm -rf build)\n  ⎿  Interrupted · What should Claude do instead?\n\n╭─╮\n│ > │\n╰─╯";
        assert!(claude_interrupted(stopped));
        let dialog = format!(
            "{stopped}\n{}",
            [
                "Bash command",
                "rm x",
                "Do you want to proceed?",
                "1. Yes",
                "2. No"
            ]
            .repeat(2)
            .join("\n")
        );
        assert!(!claude_interrupted(&dialog));
    }
}
