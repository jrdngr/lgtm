//! The agents lgterm knows, and what sets each apart: how to spot it, resume it, and read its
//! screen.

use serde::{Deserialize, Serialize};

use super::status::Status;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Claude,
    Codex,
    Pi,
}

/// What lgterm knows about one kind of agent.
pub struct Behavior {
    pub kind: Kind,
    pub name: &'static str,
    /// Parts of a path that run it: the native installer's versions folder or the npm package.
    install_paths: &'static [&'static str],
    /// What follows the agent's command to pick a session back up, before the session id.
    resume: &'static str,
    /// Its hooks report every change of status, so its screen only fills gaps.
    pub hooks_say_everything: bool,
    /// What its screen shows while it asks permission.
    blocked: &'static [&'static str],
    /// What its screen shows right after the user stopped it.
    interrupted: &'static [&'static str],
}

const BEHAVIORS: &[Behavior] = &[
    Behavior {
        kind: Kind::Claude,
        name: "claude",
        install_paths: &["/claude/versions/", "@anthropic-ai/claude-code/"],
        resume: "--resume",
        hooks_say_everything: true,
        blocked: &["Do you want to proceed?", "Do you want to make this edit"],
        interrupted: &["Interrupted by user", "What should Claude do instead"],
    },
    Behavior {
        kind: Kind::Codex,
        name: "codex",
        install_paths: &["@openai/codex/"],
        resume: "resume",
        hooks_say_everything: false,
        blocked: &[
            "Would you like to run the following command?",
            "Would you like to make the following edits?",
            "Allow command?",
        ],
        interrupted: &[],
    },
    Behavior {
        kind: Kind::Pi,
        name: "pi",
        install_paths: &["@mariozechner/pi-coding-agent/"],
        resume: "--session",
        hooks_say_everything: false,
        blocked: &[],
        interrupted: &[],
    },
];

/// Lines from the bottom of the screen that count for a stop.
const INTERRUPT_LINES: usize = 8;

impl Kind {
    pub fn behavior(self) -> &'static Behavior {
        BEHAVIORS
            .iter()
            .find(|b| b.kind == self)
            .expect("every kind has a behavior")
    }

    pub fn name(self) -> &'static str {
        self.behavior().name
    }

    pub fn from_process(name: &str) -> Option<Kind> {
        BEHAVIORS.iter().find(|b| b.name == name).map(|b| b.kind)
    }

    /// The agent a process runs, from its executable path and arguments: the command itself,
    /// an installed version of it, or its npm package run by node.
    pub fn from_command(exe: &str, argv: &[String]) -> Option<Kind> {
        let base = |p: &str| p.rsplit('/').next().unwrap_or(p).to_owned();
        let mut words = vec![exe, argv.first().map_or("", String::as_str)];
        if words.iter().any(|w| base(w) == "node") {
            words.extend(argv.get(1).map(String::as_str));
        }
        words.into_iter().find_map(|w| {
            Kind::from_process(&base(w)).or_else(|| {
                BEHAVIORS
                    .iter()
                    .find(|b| b.install_paths.iter().any(|p| w.contains(p)))
                    .map(|b| b.kind)
            })
        })
    }

    /// The command line that picks a session back up, run with the user's `command` for it.
    pub fn resume(self, command: &str, session: &str) -> String {
        format!("{} {} {session}", command.trim(), self.behavior().resume)
    }

    /// What the agent's screen shows it doing, if anything telling.
    pub fn screen_status(self, screen: &str) -> Option<Status> {
        if self.behavior().blocked.iter().any(|b| screen.contains(b)) {
            return Some(Status::Blocked);
        }
        if screen.contains("esc to interrupt") {
            return Some(Status::Working);
        }
        None
    }

    /// Whether the screen shows the agent was just stopped, as when the user turns down a
    /// permission prompt. Only the bottom lines count, so an older stop further up doesn't.
    pub fn interrupted(self, screen: &str) -> bool {
        let marks = self.behavior().interrupted;
        !marks.is_empty()
            && screen
                .lines()
                .rev()
                .filter(|l| !l.trim().is_empty())
                .take(INTERRUPT_LINES)
                .any(|l| marks.iter().any(|m| l.contains(m)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_show_work_and_prompts() {
        let working = "◦ Working (1m 12s • esc to interrupt)";
        assert_eq!(Kind::Codex.screen_status(working), Some(Status::Working));
        let ask = "Would you like to run the following command?\n esc to interrupt";
        assert_eq!(Kind::Codex.screen_status(ask), Some(Status::Blocked));
        assert_eq!(Kind::Claude.screen_status(ask), Some(Status::Working));
        assert_eq!(Kind::Claude.screen_status("> "), None);
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
        let pi = "/opt/lib/node_modules/@mariozechner/pi-coding-agent/dist/cli.js";
        assert_eq!(
            Kind::from_command("/opt/bin/node", &argv(&["node", pi])),
            Some(Kind::Pi)
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
    fn stops_count_only_at_the_bottom_of_the_screen() {
        let stopped = "⏺ Bash(rm -rf build)\n  ⎿  Interrupted · What should Claude do instead?\n\n╭─╮\n│ > │\n╰─╯";
        assert!(Kind::Claude.interrupted(stopped));
        assert!(!Kind::Codex.interrupted(stopped));
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
        assert!(!Kind::Claude.interrupted(&dialog));
    }

    #[test]
    fn sessions_resume_with_each_agents_command() {
        assert_eq!(Kind::Claude.resume("cc", "abc"), "cc --resume abc");
        assert_eq!(Kind::Codex.resume("codex", "th1"), "codex resume th1");
    }
}
