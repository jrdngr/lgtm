//! Splits: a pane beside or below this one, in lgterm or herdr, running another `lgtm` on a
//! file and line. That one is editor only and leaves the MCP socket to this one.

use std::path::Path;
use std::process::{Command, Stdio};

/// The flag that starts `lgtm` as a split.
pub const FLAG: &str = "--split";

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// What can split this terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Host {
    /// lgterm, through its binary.
    Lgterm(String),
    Herdr,
}

/// The app this terminal is a pane of, if it can split.
pub fn host() -> Option<Host> {
    if var(lgterm_proto::TERMINAL_ENV).is_some()
        && let Some(bin) = var(lgterm_proto::BIN_ENV)
    {
        return Some(Host::Lgterm(bin));
    }
    var("HC_TERMINAL_ID").map(|_| Host::Herdr)
}

/// The command that splits this pane, to the right or down, for `lgtm` at `exe` on `file`, an
/// absolute path, at `line`.
pub fn argv(host: &Host, exe: &Path, file: &Path, line: u32, right: bool) -> Vec<String> {
    let lgtm = [
        exe.to_string_lossy().into_owned(),
        FLAG.to_string(),
        format!("{}:{line}", file.display()),
    ];
    if let Host::Lgterm(bin) = host {
        let side = if right { "--right" } else { "--down" };
        let mut argv = vec![
            bin.clone(),
            "split".into(),
            side.into(),
            "--focus".into(),
            "--".into(),
        ];
        argv.extend(lgtm);
        return argv;
    }
    let split = serde_json::json!({
        "direction": if right { "right" } else { "down" },
        "argv": lgtm,
    });
    vec![
        "herdr-app".into(),
        "do".into(),
        "pane.split".into(),
        split.to_string(),
    ]
}

/// Opens a split on `file` at `line`.
pub fn open(host: &Host, file: &Path, line: u32, right: bool) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let argv = argv(host, &exe, file, line, right);
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_split_runs_lgtm_as_a_split_on_the_file_and_line() {
        let argv = argv(
            &Host::Herdr,
            Path::new("/bin/lgtm"),
            Path::new("/r/src/a.rs"),
            12,
            false,
        );
        assert_eq!(argv[..3], ["herdr-app", "do", "pane.split"]);
        let split: serde_json::Value = serde_json::from_str(&argv[3]).unwrap();
        assert_eq!(split["direction"], "down");
        assert_eq!(
            split["argv"],
            serde_json::json!(["/bin/lgtm", "--split", "/r/src/a.rs:12"])
        );
    }

    #[test]
    fn lgterm_splits_through_its_own_command() {
        let host = Host::Lgterm("/Applications/lgterm.app/lgterm".into());
        let argv = argv(&host, Path::new("/bin/lgtm"), Path::new("/r/a.rs"), 3, true);
        assert_eq!(
            argv,
            [
                "/Applications/lgterm.app/lgterm",
                "split",
                "--right",
                "--focus",
                "--",
                "/bin/lgtm",
                "--split",
                "/r/a.rs:3"
            ]
        );
    }
}
