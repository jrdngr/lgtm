//! The command that starts Claude Code, from the `claudeTerminal` setting, for running it
//! outside a terminal. A command on PATH runs as
//! is. Otherwise the login shell says what the name is: an alias is expanded once and run
//! directly, and a function runs through the shell on every call.

use std::io::Read;
use std::process::Stdio;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde::Serialize;
use specta::Type;

use crate::pty::default_shell;

#[derive(Clone, Debug, PartialEq)]
struct Resolved {
    program: String,
    args: Vec<String>,
    via_shell: bool,
}

/// How the `claudeTerminal` setting resolved, for the Settings tab.
#[derive(Clone, Debug, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCommand {
    pub setting: String,
    /// The command line that runs, like `ucode claude` for the alias `cc`.
    pub resolved: Option<String>,
    /// It's a shell function, so each run goes through the login shell.
    pub via_shell: bool,
    pub error: Option<String>,
}

static RESOLVED: RwLock<Option<(String, Result<Resolved, String>)>> = RwLock::new(None);

/// What `command -v` printed for a name.
#[derive(Debug, PartialEq)]
enum Lookup {
    Path(String),
    Alias(Vec<String>),
    /// A function or builtin, which only the shell can run.
    Shell,
    Missing,
}

/// Reads `command -v` output: a path, `alias cc='ucode claude'` (zsh and bash), or a bare name
/// for a function.
fn interpret(out: &str) -> Lookup {
    let out = out.trim();
    if out.is_empty() {
        return Lookup::Missing;
    }
    if out.starts_with('/') {
        return Lookup::Path(out.to_string());
    }
    let Some(alias) = out.strip_prefix("alias ") else {
        return Lookup::Shell;
    };
    // The value is quoted once by the shell's printout, then split like a command line.
    let value = alias.split_once('=').map(|(_, v)| v).unwrap_or_default();
    let words = shlex::split(value)
        .map(|w| w.join(" "))
        .and_then(|inner| shlex::split(&inner));
    match words {
        Some(words) if !words.is_empty() => Lookup::Alias(words),
        _ => Lookup::Shell,
    }
}

fn on_path(name: &str) -> bool {
    super::which(name).is_some()
}

/// How long the login shell gets to say what a name is. Shell startup can hang without a
/// terminal, and tool checks wait on this.
const SHELL_TIMEOUT: Duration = Duration::from_secs(10);

/// Asks the login shell what `name` is.
fn ask_shell(shell: &str, name: &str) -> anyhow::Result<String> {
    let quoted = shlex::try_quote(name)?;
    let script = format!("printf '\\n__LGTM_CMD__%s' \"$(command -v -- {quoted})\"");
    let mut child = std::process::Command::new(shell)
        .args(["-l", "-i", "-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("couldn't run {shell}"))?;
    let mut stdout = child.stdout.take().context("no stdout")?;
    // Read alongside the wait, so a full pipe can't stall the shell.
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).ok();
        out
    });
    let start = Instant::now();
    while child.try_wait()?.is_none() {
        if start.elapsed() > SHELL_TIMEOUT {
            child.kill().ok();
            child.wait().ok();
            bail!(
                "your login shell ({shell}) didn't start in {}s",
                SHELL_TIMEOUT.as_secs()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let out = reader.join().unwrap_or_default();
    let out = String::from_utf8_lossy(&out);
    Ok(out
        .rsplit_once("__LGTM_CMD__")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default())
}

/// `word` with a leading `~` swapped for the home directory, as the shell would.
fn expand_home(word: &str) -> String {
    match (word.strip_prefix('~'), std::env::var("HOME")) {
        (Some(rest), Ok(home)) if rest.is_empty() || rest.starts_with('/') => {
            format!("{home}{rest}")
        }
        _ => word.to_string(),
    }
}

fn resolve_with(
    setting: &str,
    shell: &str,
    on_path: impl Fn(&str) -> bool,
    lookup: impl Fn(&str) -> anyhow::Result<String>,
) -> anyhow::Result<Resolved> {
    let words = shlex::split(setting).context("the Claude command has unmatched quotes")?;
    // Each word gets `~` expanded, as the shell would, since direct runs skip the shell.
    let words: Vec<String> = words.iter().map(|w| expand_home(w)).collect();
    let Some((first, rest)) = words.split_first() else {
        bail!("the Claude command is empty");
    };
    let direct = |program: &str, before: &[String]| Resolved {
        program: program.to_string(),
        args: before.iter().chain(rest).cloned().collect(),
        via_shell: false,
    };
    if on_path(first) {
        return Ok(direct(first, &[]));
    }
    let through_shell = Resolved {
        program: shell.to_string(),
        args: ["-l", "-i", "-c", &format!("{setting} \"$@\""), "lgtm"]
            .map(String::from)
            .to_vec(),
        via_shell: true,
    };
    Ok(match interpret(&lookup(first)?) {
        Lookup::Path(path) => direct(&path, &[]),
        Lookup::Alias(alias) if on_path(&alias[0]) => direct(&alias[0], &alias[1..]),
        Lookup::Alias(_) | Lookup::Shell => through_shell,
        Lookup::Missing => {
            bail!("`{first}` isn't a command, alias or function in your login shell")
        }
    })
}

fn resolve_setting(setting: &str) -> Result<Resolved, String> {
    let shell = default_shell();
    resolve_with(setting, &shell, on_path, |name| ask_shell(&shell, name))
        .map_err(|e| format!("{e:#}"))
}

/// Resolves the current setting again, after it changed or the shell's config did.
pub fn refresh() {
    let setting = crate::settings::get().claude_terminal;
    let resolved = resolve_setting(&setting);
    *RESOLVED.write().unwrap() = Some((setting, resolved));
}

fn current() -> Result<Resolved, String> {
    let setting = crate::settings::get().claude_terminal;
    if let Some((s, r)) = RESOLVED.read().unwrap().as_ref()
        && *s == setting
    {
        return r.clone();
    }
    refresh();
    current()
}

/// A command that starts Claude Code, to add arguments to.
pub fn command() -> anyhow::Result<tokio::process::Command> {
    let r = current().map_err(anyhow::Error::msg)?;
    let mut cmd = tokio::process::Command::new(&r.program);
    cmd.args(&r.args);
    Ok(cmd)
}

pub fn status() -> ClaudeCommand {
    let setting = crate::settings::get().claude_terminal;
    let (resolved, via_shell, error) = match current() {
        Ok(r) if r.via_shell => (Some(setting.clone()), true, None),
        Ok(r) => {
            let words: Vec<&str> = std::iter::once(r.program.as_str())
                .chain(r.args.iter().map(String::as_str))
                .collect();
            (shlex::try_join(words).ok(), false, None)
        }
        Err(e) => (None, false, Some(e)),
    };
    ClaudeCommand {
        setting,
        resolved,
        via_shell,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_what_the_shell_says_a_name_is() {
        assert_eq!(
            interpret("alias cc='ucode claude'\n"),
            Lookup::Alias(vec!["ucode".into(), "claude".into()])
        );
        assert_eq!(
            interpret(r#"alias cc='ucode claude --append "a b"'"#),
            Lookup::Alias(vec![
                "ucode".into(),
                "claude".into(),
                "--append".into(),
                "a b".into()
            ])
        );
        assert_eq!(
            interpret("/opt/bin/claude"),
            Lookup::Path("/opt/bin/claude".into())
        );
        assert_eq!(interpret("cc"), Lookup::Shell);
        assert_eq!(interpret(""), Lookup::Missing);
    }

    #[test]
    fn resolves_commands_aliases_and_functions() {
        let on_path = |n: &str| n == "claude" || n == "ucode";
        let shell = |out: &'static str| move |_: &str| Ok(out.to_string());
        let resolve = |setting, out| resolve_with(setting, "/bin/zsh", on_path, shell(out));

        let plain = resolve("claude --debug", "").unwrap();
        assert_eq!(
            (plain.program.as_str(), plain.args.as_slice()),
            ("claude", &["--debug".to_string()][..])
        );

        let alias = resolve("cc -x", "alias cc='ucode claude'").unwrap();
        assert_eq!(alias.program, "ucode");
        assert_eq!(alias.args, ["claude", "-x"]);
        assert!(!alias.via_shell);

        let function = resolve("cc", "cc").unwrap();
        assert_eq!(function.program, "/bin/zsh");
        assert_eq!(function.args, ["-l", "-i", "-c", "cc \"$@\"", "lgtm"]);

        // An alias to something that isn't on PATH, like another alias, needs the shell too.
        assert!(resolve("cc", "alias cc='uc claude'").unwrap().via_shell);

        let home = std::env::var("HOME").unwrap();
        let installed = format!("{home}/.local/bin/claude");
        let tilde = resolve_with(
            "~/.local/bin/claude",
            "/bin/zsh",
            |n| n == installed,
            shell(""),
        );
        assert_eq!(tilde.unwrap().program, installed);
        let settings = resolve("claude --settings ~/.claude/s.json", "").unwrap();
        assert_eq!(settings.args, ["--settings", &format!("{home}/.claude/s.json")]);

        assert!(resolve("cc", "").is_err());
        assert!(resolve("  ", "").is_err());
        assert!(resolve("'claude", "").is_err());
    }
}
