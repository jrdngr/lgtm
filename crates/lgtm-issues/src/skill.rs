//! The Claude Code skill that points agents at `lgtm issues guide`.

use std::path::Path;

const SKILL: &str = r#"---
name: lgtm-issues
description: Use when asked to file, claim, work on, fix, close or orchestrate issues in an lgtm issues folder (one with ISSUES.md), to ask the user a question through issues, or to coordinate agents through an issue tracker.
---

Run `lgtm issues guide` and follow it. It covers filing, claiming and closing issues, logging
progress, and asking the user questions they answer in `lgtm issues`.

When orchestrating other agents, also run `lgterm guide` to start and message them. Tell each
agent its issue id and to follow `lgtm issues guide`.
"#;

/// Writes the skill into `~/.claude/skills/lgtm-issues` when Claude Code is set up and the
/// file says something else.
pub fn install(home: &Path) {
    let claude = home.join(".claude");
    if !claude.is_dir() {
        return;
    }
    let dir = claude.join("skills").join("lgtm-issues");
    let file = dir.join("SKILL.md");
    if std::fs::read_to_string(&file).is_ok_and(|s| s == SKILL) {
        return;
    }
    if let Err(e) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, SKILL)) {
        eprintln!("lgtm: can't write {}: {e}", file.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_goes_in_only_where_claude_is_set_up() {
        let home = std::env::temp_dir().join(format!("lgtm-issues-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        install(&home);
        assert!(!home.join(".claude").exists());
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install(&home);
        let file = home.join(".claude/skills/lgtm-issues/SKILL.md");
        assert_eq!(std::fs::read_to_string(file).unwrap(), SKILL);
        let _ = std::fs::remove_dir_all(&home);
    }
}
