//! Splits: a herdr pane beside or below this one running another `lgtm` on a file and line.
//! That one is editor only and leaves the MCP socket to this one.

use std::path::Path;
use std::process::{Command, Stdio};

/// The flag that starts `lgtm` as a split.
pub const FLAG: &str = "--split";

/// Whether this terminal is a herdr pane, which can split.
pub fn herdr() -> bool {
    std::env::var_os("HC_TERMINAL_ID").is_some_and(|v| !v.is_empty())
}

/// The command that splits this herdr pane, to the right or down, for `lgtm` at `exe` on
/// `file`, an absolute path, at `line`.
pub fn argv(exe: &Path, file: &Path, line: u32, right: bool) -> Vec<String> {
    let lgtm = [
        exe.to_string_lossy().into_owned(),
        FLAG.to_string(),
        format!("{}:{line}", file.display()),
    ];
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
pub fn open(file: &Path, line: u32, right: bool) -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let argv = argv(&exe, file, line, right);
    Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_split_runs_lgtm_as_a_split_on_the_file_and_line() {
        let argv = argv(Path::new("/bin/lgtm"), Path::new("/r/src/a.rs"), 12, false);
        assert_eq!(argv[..3], ["herdr-app", "do", "pane.split"]);
        let split: serde_json::Value = serde_json::from_str(&argv[3]).unwrap();
        assert_eq!(split["direction"], "down");
        assert_eq!(
            split["argv"],
            serde_json::json!(["/bin/lgtm", "--split", "/r/src/a.rs:12"])
        );
    }
}
