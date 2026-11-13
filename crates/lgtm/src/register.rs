//! Registers this binary with Claude Code on start: `lgtm mcp` as a user-scope MCP server and
//! `lgtm hook` in its user settings. A cargo build never registers, so trying one out leaves
//! the installed command in place.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use serde_json::{Value, json};

/// The hook events lgtm listens to.
const EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// Registers this binary when it isn't yet, in the background so the TUI starts at once.
pub fn on_start() {
    let Some(exe) = exe() else {
        return;
    };
    let mcp = !claude_json()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|c| mcp_in(&c, &exe));
    let hooks = !settings_json()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|c| hooks_in(&c, &exe));
    if !mcp && !hooks {
        return;
    }
    std::thread::spawn(move || {
        if mcp {
            let _ = register_mcp(&exe);
        }
        if hooks {
            let _ = register_hooks(&exe);
        }
    });
}

/// This binary, unless it's a cargo build's.
fn exe() -> Option<String> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let exe = exe.to_str()?.to_string();
    (!dev_build(&exe)).then_some(exe)
}

fn dev_build(command: &str) -> bool {
    command.contains("/target/debug/") || command.contains("/target/release/")
}

fn config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from)
}

/// Claude Code's user config, which holds user-scope MCP servers.
fn claude_json() -> Option<PathBuf> {
    match config_dir() {
        Some(dir) => Some(dir.join(".claude.json")),
        None => std::env::home_dir().map(|h| h.join(".claude.json")),
    }
}

/// Claude Code's user settings, which hold user hooks.
fn settings_json() -> Option<PathBuf> {
    match config_dir() {
        Some(dir) => Some(dir.join("settings.json")),
        None => std::env::home_dir().map(|h| h.join(".claude/settings.json")),
    }
}

/// Whether a `.claude.json` registers `exe mcp` as the `lgtm` server.
fn mcp_in(config: &str, exe: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(config) else {
        return false;
    };
    let server = &v["mcpServers"]["lgtm"];
    server["command"] == exe && server["args"] == json!(["mcp"])
}

/// Registers `exe mcp` through the `claude` command, replacing any earlier registration.
fn register_mcp(exe: &str) -> anyhow::Result<()> {
    let claude = || {
        let mut c = Command::new("claude");
        c.stdin(std::process::Stdio::null());
        c
    };
    // Fails when there's nothing to remove, which is fine.
    claude()
        .args(["mcp", "remove", "-s", "user", "lgtm"])
        .output()
        .context("couldn't run Claude Code")?;
    let out = claude()
        .args(["mcp", "add", "-s", "user", "lgtm", "--", exe, "mcp"])
        .output()?;
    anyhow::ensure!(out.status.success(), "`claude mcp add` failed");
    Ok(())
}

/// The hook command for this binary.
fn hook_command(exe: &str) -> String {
    format!("'{}' hook", exe.replace('\'', "'\\''"))
}

/// Whether a hook command runs some lgtm binary's `hook`.
fn is_lgtm_hook(command: &str) -> bool {
    command.ends_with("/lgtm' hook") || command.ends_with("/lgtm-app' hook")
}

/// Whether settings JSON has this binary's hook on every event lgtm listens to.
fn hooks_in(config: &str, exe: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(config) else {
        return false;
    };
    let command = hook_command(exe);
    EVENTS.iter().all(|event| {
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
    for event in EVENTS {
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
    write_hooks(&path, exe)
}

fn write_hooks(path: &Path, exe: &str) -> anyhow::Result<()> {
    let config = std::fs::read_to_string(path).unwrap_or_default();
    let updated = with_hooks(&config, exe)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.lgtm-tmp");
    std::fs::write(&tmp, updated)?;
    std::fs::rename(&tmp, path).context("couldn't save Claude Code's settings.json")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_registration_from_claude_json() {
        let config = |cmd: &str, args: &str| {
            format!(
                r#"{{"mcpServers":{{"lgtm":{{"type":"stdio","command":"{cmd}","args":{args}}}}}}}"#
            )
        };
        assert!(mcp_in(&config("/a/lgtm", r#"["mcp"]"#), "/a/lgtm"));
        assert!(!mcp_in(&config("/old/lgtm-app", r#"["mcp"]"#), "/a/lgtm"));
        assert!(!mcp_in(&config("/a/lgtm", r#"["hook"]"#), "/a/lgtm"));
        assert!(!mcp_in("{}", "/a/lgtm"));
        assert!(!mcp_in("", "/a/lgtm"));
    }

    #[test]
    fn adds_hooks_replacing_other_lgtm_builds_and_keeping_the_users() {
        let mine = r#"{"type":"command","command":"say hi"}"#;
        let config = format!(
            r#"{{"model":"opus","hooks":{{"Stop":[{{"hooks":[{mine},{{"type":"command","command":"'/old/lgtm-app' hook"}}]}}],
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
        assert!(with_hooks("[1]", "/a/lgtm").is_err());
    }

    #[test]
    fn writes_hooks_into_a_settings_file_that_doesnt_exist_yet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude/settings.json");
        write_hooks(&path, "/a/lgtm").unwrap();
        assert!(hooks_in(
            &std::fs::read_to_string(&path).unwrap(),
            "/a/lgtm"
        ));
    }
}
