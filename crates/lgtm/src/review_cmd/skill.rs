//! The skill that points Claude Code and Codex at `lgtm agent review guide`.

use std::path::Path;

const SKILL: &str = r#"---
name: lgtm-review
description: Use when the user mentions lgtm or its review, refers to review comments like C3 or P2, asks you to review a change in lgtm or says `lgtm flag review`, or asks for work in separate lgtm snapshots.
---

Run `lgtm agent review guide` and follow it. It covers reading the review open in lgtm, the user's
comments and resolving them, checking files, marking lines, leaving comments, and snapshots.
"#;

/// Writes the skill for each agent set up under `home` whose copy says something else.
pub fn install(home: &Path) {
    for agent in [".claude", ".codex"] {
        let root = home.join(agent);
        if root.is_dir() {
            write(&root.join("skills/lgtm-review/SKILL.md"));
        }
    }
}

fn write(file: &Path) {
    if std::fs::read_to_string(file).is_ok_and(|s| s == SKILL) {
        return;
    }
    let written = match file.parent() {
        Some(dir) => std::fs::create_dir_all(dir).and_then(|()| std::fs::write(file, SKILL)),
        None => return,
    };
    if let Err(e) = written {
        eprintln!("lgtm: can't write {}: {e}", file.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_goes_only_to_agents_that_are_set_up() {
        let home = tempfile::tempdir().unwrap();
        install(home.path());
        assert!(!home.path().join(".claude").exists());
        std::fs::create_dir_all(home.path().join(".codex")).unwrap();
        install(home.path());
        let codex = home.path().join(".codex/skills/lgtm-review/SKILL.md");
        assert_eq!(std::fs::read_to_string(codex).unwrap(), SKILL);
        assert!(!home.path().join(".claude").exists());
    }
}
