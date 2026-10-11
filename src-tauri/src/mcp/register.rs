//! Registering `lgtm mcp` with Claude Code at user scope, so every `claude` in an lgtm
//! terminal gets the tools.

use std::path::PathBuf;

use anyhow::{Context, ensure};
use serde::Serialize;
use specta::Type;
use tokio::process::Command;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum McpSetup {
    /// Registered to run this app.
    Registered,
    /// Registered to run another lgtm binary that exists, like a dev build.
    Elsewhere,
    /// Not registered, or registered to a binary that's gone.
    Missing,
}

#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub setup: McpSetup,
    /// The command that registers this app.
    pub command: String,
}

fn exe() -> anyhow::Result<String> {
    Ok(std::env::current_exe()?.to_string_lossy().into_owned())
}

/// Claude Code's user config, which holds user-scope MCP servers.
fn claude_json() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => Some(PathBuf::from(dir).join(".claude.json")),
        None => std::env::home_dir().map(|h| h.join(".claude.json")),
    }
}

/// How `lgtm` is registered in a `.claude.json`, for this binary.
fn setup_in(config: &str, exe: &str) -> McpSetup {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(config) else {
        return McpSetup::Missing;
    };
    let server = &v["mcpServers"]["lgtm"];
    let Some(command) = server["command"].as_str() else {
        return McpSetup::Missing;
    };
    let mcp = server["args"]
        .as_array()
        .is_some_and(|a| a.len() == 1 && a[0] == "mcp");
    match (
        command == exe && mcp,
        std::path::Path::new(command).exists(),
    ) {
        (true, _) => McpSetup::Registered,
        (false, true) => McpSetup::Elsewhere,
        (false, false) => McpSetup::Missing,
    }
}

pub fn status() -> anyhow::Result<McpStatus> {
    let exe = exe()?;
    let config = claude_json()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    Ok(McpStatus {
        setup: setup_in(&config, &exe),
        command: format!("claude mcp add -s user lgtm -- '{exe}' mcp"),
    })
}

/// Registers this app as the `lgtm` MCP server, replacing any earlier registration.
pub async fn register() -> anyhow::Result<()> {
    let exe = exe()?;
    // Fails when there's nothing to remove, which is fine.
    Command::new("claude")
        .args(["mcp", "remove", "-s", "user", "lgtm"])
        .output()
        .await
        .context("couldn't run `claude`")?;
    let out = Command::new("claude")
        .args(["mcp", "add", "-s", "user", "lgtm", "--", &exe, "mcp"])
        .output()
        .await?;
    ensure!(
        out.status.success(),
        "`claude mcp add` failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_registration_from_claude_json() {
        let config = |cmd: &str| {
            format!(
                r#"{{"mcpServers":{{"lgtm":{{"type":"stdio","command":"{cmd}","args":["mcp"]}}}}}}"#
            )
        };
        assert_eq!(
            setup_in(&config("/a/lgtm"), "/a/lgtm"),
            McpSetup::Registered
        );
        assert_eq!(
            setup_in(&config("/gone/lgtm"), "/a/lgtm"),
            McpSetup::Missing
        );
        let other = std::env::current_exe().unwrap();
        let other = other.to_string_lossy();
        assert_eq!(setup_in(&config(&other), "/a/lgtm"), McpSetup::Elsewhere);
        assert_eq!(setup_in("{}", "/a/lgtm"), McpSetup::Missing);
        assert_eq!(setup_in("", "/a/lgtm"), McpSetup::Missing);
    }
}
