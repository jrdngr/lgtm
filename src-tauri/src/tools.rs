//! Checks that the command-line tools the app runs are installed and new enough.

use serde::Serialize;
use specta::Type;
use tokio::process::Command;

/// A tool that's missing or too old.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
pub struct ToolProblem {
    pub tool: String,
    /// The version found, or `None` when it isn't installed.
    pub found: Option<String>,
    pub minimum: String,
    /// What to run to fix it.
    pub fix: String,
    /// Whether the app can't work without it. Otherwise only some features need it.
    pub required: bool,
}

struct Tool {
    name: &'static str,
    minimum: (u32, u32, u32),
    install: &'static str,
    update: &'static str,
    required: bool,
}

const TOOLS: [Tool; 3] = [
    // `WorkspaceRef.root()` in workspace list templates came in 0.40.
    Tool {
        name: "jj",
        minimum: (0, 40, 0),
        install: "brew install jj",
        update: "brew upgrade jj",
        required: true,
    },
    // `-p` with `--json-schema` works from 2.1.22.
    Tool {
        name: "claude",
        minimum: (2, 1, 22),
        install: "curl -fsSL https://claude.ai/install.sh | bash",
        update: "claude update",
        required: false,
    },
    // `gh api graphql --input -` and the PR review thread mutations.
    Tool {
        name: "gh",
        minimum: (2, 40, 0),
        install: "brew install gh && gh auth login",
        update: "brew upgrade gh",
        required: false,
    },
];

/// The first `x.y.z` in `--version` output, like `jj 0.46.0` or `2.1.296 (Claude Code)`.
fn parse_version(out: &str) -> Option<(u32, u32, u32)> {
    out.split(|c: char| c.is_whitespace() || c == '-' || c == '+')
        .find_map(|word| {
            let mut parts = word.trim_start_matches('v').split('.');
            let v = (
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
                parts.next()?.parse().ok()?,
            );
            parts.next().is_none().then_some(v)
        })
}

fn problem(tool: &Tool, out: Option<&str>) -> Option<ToolProblem> {
    let (major, minor, patch) = tool.minimum;
    let minimum = format!("{major}.{minor}.{patch}");
    let make = |found: Option<String>, fix: &str| ToolProblem {
        tool: tool.name.to_string(),
        found,
        minimum: minimum.clone(),
        fix: fix.to_string(),
        required: tool.required,
    };
    let Some(out) = out else {
        return Some(make(None, tool.install));
    };
    match parse_version(out) {
        Some(v) if v >= tool.minimum => None,
        Some((a, b, c)) => Some(make(Some(format!("{a}.{b}.{c}")), tool.update)),
        None => Some(make(Some(out.trim().to_string()), tool.update)),
    }
}

async fn version(name: &str) -> Option<String> {
    let out = Command::new(name).arg("--version").output().await.ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every tool that's missing or too old.
pub async fn check() -> Vec<ToolProblem> {
    let mut problems = Vec::new();
    for tool in &TOOLS {
        let out = version(tool.name).await;
        problems.extend(problem(tool, out.as_deref()));
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_checked_against_the_minimum() {
        assert_eq!(parse_version("jj 0.46.0-abc\n"), Some((0, 46, 0)));
        assert_eq!(parse_version("2.1.296 (Claude Code)"), Some((2, 1, 296)));
        let [jj, claude, gh] = &TOOLS;
        assert_eq!(problem(jj, Some("jj 0.46.0")), None);
        let old = problem(jj, Some("jj 0.39.1")).unwrap();
        assert_eq!(
            (old.found.as_deref(), old.fix.as_str()),
            (Some("0.39.1"), "brew upgrade jj")
        );
        let missing = problem(claude, None).unwrap();
        assert!(missing.found.is_none() && !missing.required);
        assert_eq!(problem(claude, Some("2.1.22 (Claude Code)")), None);
        assert_eq!(problem(gh, Some("gh version 2.102.0 (2026-09-30)\n")), None);
    }
}
