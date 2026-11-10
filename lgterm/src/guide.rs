//! What agents read about driving lgterm: the playbook `lgterm guide` prints, and the Claude
//! Code skill that points to it.

use std::path::Path;

pub const GUIDE: &str = r#"# Working in lgterm

You run in an lgterm terminal. $LGTERM_TERMINAL_ID is your terminal, like t4. Terminals sit
in panes (splits), panes in spaces, and spaces in the window's layout.

## Starting agents

  lgterm split --right --agent claude --name "api lead" -- "Build the API. Report to t4 with lgterm send."
  lgterm split --down --from t7 --agent codex --name tests -- "Write the tests"
  lgterm tab --agent claude --name "auth" -- "Own the auth subsystem"

- split opens a pane beside yours (--right or --down), tab a tab in your pane, space a new
  space beside yours. --from T opens beside terminal T instead of yours.
- --agent starts the user's configured command for that agent; words after -- are its
  arguments, so a quoted prompt becomes its first message.
- --name labels the tab. Name every agent you start.
- Each command prints the new terminal's id. You are its opener.
- Build a grid by splitting: split --right once, then split the new pane --down, then split
  each of those --right with --from.

## Talking

  lgterm send t9 "Status?"         message an agent; it arrives when the agent is idle
  lgterm wait t9 [--timeout 600]   block until t9 is done, blocked or gone
  lgterm read t9 [--lines 50]      the text on t9's screen
  lgterm ls [--json]               every terminal: id, status, space, name

Messages arrive as "[lgterm message from t4 · name] ..." with the command to reply. Tell the
agents you start who you are and to report back with lgterm send.

## Cleaning up

  lgterm close t9

You can close terminals you opened, and the ones they opened. Close an agent's tab once its
work is done and reported.

## Limits

- The user can lock a space. In a locked space split, tab, space, layout, close, focus and
  type are refused; send, wait, read and ls still work. Don't work around a lock; tell the
  user.
- Don't close terminals you didn't open, and don't move focus unless asked.
"#;

const SKILL: &str = r#"---
name: lgterm
description: Use when running in lgterm ($LGTERM_TERMINAL_ID is set) and asked to start, coordinate, message or close other agents, split panes, or set up a team of agents.
---

Run `lgterm guide` and follow it. It covers starting agents in panes and tabs, messaging
them, waiting on them, and closing their tabs.
"#;

/// Writes the skill into `~/.claude/skills/lgterm` when Claude Code is set up and the file
/// says something else.
pub fn install_skill(home: &Path) {
    let claude = home.join(".claude");
    if !claude.is_dir() {
        return;
    }
    let dir = claude.join("skills").join("lgterm");
    let file = dir.join("SKILL.md");
    if std::fs::read_to_string(&file).is_ok_and(|s| s == SKILL) {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, SKILL)) {
        eprintln!("lgterm: can't write {}: {e}", file.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_goes_in_only_where_claude_is_set_up() {
        let home = std::env::temp_dir().join(format!("lgterm-guide-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        install_skill(&home);
        assert!(!home.join(".claude").exists());
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_skill(&home);
        let file = home.join(".claude/skills/lgterm/SKILL.md");
        assert_eq!(std::fs::read_to_string(file).unwrap(), SKILL);
        let _ = std::fs::remove_dir_all(&home);
    }
}
