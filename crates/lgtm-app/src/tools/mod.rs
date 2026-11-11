//! Checks that the command-line tools the app runs are installed and new enough.

pub mod claude;

use std::path::{Path, PathBuf};
use std::time::Duration;

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
    /// Why it couldn't be checked, like a `--version` that never finished.
    pub error: Option<String>,
}

/// How long `--version` gets. A Claude command that goes through a wrapper or the login
/// shell can hang, and the app waits on this check before showing anything.
const VERSION_TIMEOUT: Duration = Duration::from_secs(10);

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

fn problem(tool: &Tool, out: Result<&str, Option<&str>>) -> Option<ToolProblem> {
    let (major, minor, patch) = tool.minimum;
    let minimum = format!("{major}.{minor}.{patch}");
    let make = |found: Option<String>, fix: &str| ToolProblem {
        tool: tool.name.to_string(),
        found,
        minimum: minimum.clone(),
        fix: fix.to_string(),
        required: tool.required,
        error: None,
    };
    let out = match out {
        Ok(out) => out,
        Err(error) => {
            return Some(ToolProblem {
                error: error.map(str::to_string),
                ..make(None, tool.install)
            });
        }
    };
    match parse_version(out) {
        Some(v) if v >= tool.minimum => None,
        Some((a, b, c)) => Some(make(Some(format!("{a}.{b}.{c}")), tool.update)),
        None => Some(make(Some(out.trim().to_string()), tool.update)),
    }
}

/// A tool as found: where it is, its version, and what's wrong with it.
#[derive(Clone, Debug, Serialize, Type)]
pub struct ToolStatus {
    pub tool: String,
    /// The file on PATH, or for claude the command line the Claude setting runs.
    pub location: Option<String>,
    pub version: Option<String>,
    pub problem: Option<ToolProblem>,
}

/// The file `name` runs from PATH.
pub fn which(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        return Path::new(name).is_file().then(|| PathBuf::from(name));
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// `name --version`'s output. Fails with why it couldn't be checked, or `None` when it isn't
/// installed.
async fn version(name: &str) -> Result<String, Option<String>> {
    let mut cmd = if name == "claude" {
        claude::command().map_err(|e| Some(format!("{e:#}")))?
    } else {
        Command::new(name)
    };
    cmd.arg("--version").kill_on_drop(true);
    let out = match tokio::time::timeout(VERSION_TIMEOUT, cmd.output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(_)) => return Err(None),
        Err(_) => {
            return Err(Some(format!(
                "`{name} --version` didn't finish in {}s",
                VERSION_TIMEOUT.as_secs()
            )));
        }
    };
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        .ok_or(None)
}

/// Every tool, found or not.
pub async fn status() -> Vec<ToolStatus> {
    // The Claude command may have been installed or changed in the shell since it resolved.
    tokio::task::spawn_blocking(claude::refresh).await.ok();
    let mut found = Vec::new();
    for tool in &TOOLS {
        let out = version(tool.name).await;
        let location = if tool.name == "claude" {
            claude::status().resolved
        } else {
            which(tool.name).map(|p| p.to_string_lossy().into_owned())
        };
        found.push(ToolStatus {
            tool: tool.name.to_string(),
            location,
            version: out
                .as_deref()
                .ok()
                .and_then(parse_version)
                .map(|(a, b, c)| format!("{a}.{b}.{c}")),
            problem: problem(tool, out.as_deref().map_err(Option::as_deref)),
        });
    }
    found
}

/// Every tool that's missing or too old.
pub async fn check() -> Vec<ToolProblem> {
    status()
        .await
        .into_iter()
        .filter_map(|t| t.problem)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_checked_against_the_minimum() {
        assert_eq!(parse_version("jj 0.46.0-abc\n"), Some((0, 46, 0)));
        assert_eq!(parse_version("2.1.296 (Claude Code)"), Some((2, 1, 296)));
        let [jj, claude, gh] = &TOOLS;
        assert_eq!(problem(jj, Ok("jj 0.46.0")), None);
        let old = problem(jj, Ok("jj 0.39.1")).unwrap();
        assert_eq!(
            (old.found.as_deref(), old.fix.as_str()),
            (Some("0.39.1"), "brew upgrade jj")
        );
        let missing = problem(claude, Err(None)).unwrap();
        assert!(missing.found.is_none() && !missing.required);
        assert_eq!(problem(claude, Ok("2.1.22 (Claude Code)")), None);
        assert_eq!(problem(gh, Ok("gh version 2.102.0 (2026-09-30)\n")), None);
    }
}
