//! Registering `lgtm mcp` with Claude Code at user scope, so every `claude` in an lgtm
//! terminal gets the tools, and `lgtm hook` in its user settings, so the app sees how each
//! terminal's agent stands.

use std::path::PathBuf;

use anyhow::{Context, ensure};
use serde::Serialize;
use serde_json::{Value, json};
use specta::Type;

use super::hook;

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

/// Claude Code's user settings, which hold user hooks.
fn settings_json() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) => Some(PathBuf::from(dir).join("settings.json")),
        None => std::env::home_dir().map(|h| h.join(".claude/settings.json")),
    }
}

/// The hook command for this binary.
fn hook_command(exe: &str) -> String {
    format!("'{}' hook", exe.replace('\'', "'\\''"))
}

/// Whether a hook command runs some lgtm binary's `hook`.
fn is_lgtm_hook(command: &str) -> bool {
    command.ends_with("/lgtm' hook")
}

/// Whether settings JSON has this binary's hook on every event lgtm listens to.
fn hooks_in(config: &str, exe: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(config) else {
        return false;
    };
    let command = hook_command(exe);
    hook::EVENTS.iter().all(|event| {
        v["hooks"][event].as_array().is_some_and(|groups| {
            groups.iter().any(|g| {
                g["hooks"]
                    .as_array()
                    .is_some_and(|hs| hs.iter().any(|h| h["command"] == command.as_str()))
            })
        })
    })
}

/// Settings JSON with this binary's hook on every event lgtm listens to, replacing any other
/// lgtm binary's and keeping everything else.
fn with_hooks(config: &str, exe: &str) -> anyhow::Result<String> {
    let mut v: Value = if config.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(config).context("Claude Code's settings.json isn't valid JSON")?
    };
    let root = v
        .as_object_mut()
        .context("Claude Code's settings.json isn't an object")?;
    let hooks = root.entry("hooks").or_insert_with(|| json!({}));
    let hooks = hooks
        .as_object_mut()
        .context("`hooks` in settings.json isn't an object")?;
    for event in hook::EVENTS {
        let groups = hooks.entry(event).or_insert_with(|| json!([]));
        let groups = groups
            .as_array_mut()
            .with_context(|| format!("`hooks.{event}` in settings.json isn't a list"))?;
        for g in groups.iter_mut() {
            if let Some(hs) = g["hooks"].as_array_mut() {
                hs.retain(|h| !h["command"].as_str().is_some_and(is_lgtm_hook));
            }
        }
        groups.retain(|g| g["hooks"].as_array().is_none_or(|hs| !hs.is_empty()));
        groups.push(json!({
            "hooks": [{ "type": "command", "command": hook_command(exe), "timeout": 5 }]
        }));
    }
    Ok(serde_json::to_string_pretty(&v)? + "\n")
}

/// Adds this binary's hooks to Claude Code's user settings.
fn register_hooks(exe: &str) -> anyhow::Result<()> {
    let path = settings_json().context("there's no home folder for Claude Code's settings")?;
    let config = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = with_hooks(&config, exe)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.lgtm-tmp");
    std::fs::write(&tmp, updated)?;
    std::fs::rename(&tmp, &path).context("couldn't save Claude Code's settings.json")?;
    Ok(())
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
    let claude = crate::tools::claude::status()
        .resolved
        .unwrap_or_else(|| "claude".into());
    let config = claude_json()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default();
    let hooks = settings_json()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|c| hooks_in(&c, &exe));
    let setup = match setup_in(&config, &exe) {
        McpSetup::Registered if !hooks => McpSetup::Missing,
        setup => setup,
    };
    Ok(McpStatus {
        setup,
        command: format!("{claude} mcp add -s user lgtm -- '{exe}' mcp"),
    })
}

/// Registers this app as the `lgtm` MCP server and its hooks, replacing any earlier
/// registration.
pub async fn register() -> anyhow::Result<()> {
    let exe = exe()?;
    // Fails when there's nothing to remove, which is fine.
    crate::tools::claude::command()?
        .args(["mcp", "remove", "-s", "user", "lgtm"])
        .output()
        .await
        .context("couldn't run Claude Code")?;
    let out = crate::tools::claude::command()?
        .args(["mcp", "add", "-s", "user", "lgtm", "--", &exe, "mcp"])
        .output()
        .await?;
    ensure!(
        out.status.success(),
        "`claude mcp add` failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    register_hooks(&exe)
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

    #[test]
    fn adds_hooks_replacing_other_lgtm_builds_and_keeping_the_users() {
        let mine = r#"{"type":"command","command":"say hi"}"#;
        let config = format!(
            r#"{{"model":"opus","hooks":{{"Stop":[{{"hooks":[{mine},{{"type":"command","command":"'/old/lgtm' hook"}}]}}],
            "Notification":[{{"hooks":[{{"type":"command","command":"'/old/lgtm' hook"}}]}}]}}}}"#
        );
        assert!(!hooks_in(&config, "/a/lgtm"));
        let out = with_hooks(&config, "/a/lgtm").unwrap();
        assert!(hooks_in(&out, "/a/lgtm"));
        assert!(!out.contains("/old/lgtm"));
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["model"], "opus");
        assert_eq!(v["hooks"]["Stop"][0]["hooks"][0]["command"], "say hi");
        assert_eq!(v["hooks"]["Notification"].as_array().unwrap().len(), 1);
        assert_eq!(with_hooks(&out, "/a/lgtm").unwrap(), out);
        assert!(hooks_in(&with_hooks("", "/a/lgtm").unwrap(), "/a/lgtm"));
        assert!(with_hooks("[1]", "/a/lgtm").is_err());
    }
}
