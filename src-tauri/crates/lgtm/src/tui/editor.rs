//! Opening the editor at a line: in a pane beside this one in the herdr desktop app, as its
//! own window for a GUI editor, else in this terminal while the review waits. The command comes
//! from the config's `editor` template.

use std::path::Path;
use std::process::{Command, Stdio};

/// How the editor opens.
#[derive(Debug, PartialEq, Eq)]
pub enum Open {
    /// Started on its own; the review keeps the terminal.
    Detached(Vec<String>),
    /// Takes over this terminal until it exits.
    Here(Vec<String>),
}

/// Editors with windows of their own.
fn gui(name: &str) -> bool {
    matches!(
        name,
        "code" | "code-insiders" | "cursor" | "windsurf" | "zed" | "subl" | "sublime_text"
    )
}

/// The command that opens `path` at `line`, from `template`, a command line with `{file}` and
/// `{line}` in it. Without `{file}` the path goes at the end.
pub fn argv(template: &str, path: &Path, line: u32) -> Vec<String> {
    let path = path.to_string_lossy();
    let mut argv: Vec<String> = template
        .split_whitespace()
        .map(|a| {
            a.replace("{file}", &path)
                .replace("{line}", &line.to_string())
        })
        .collect();
    if !template.contains("{file}") {
        argv.push(path.into_owned());
    }
    argv
}

/// How to open `path` at `line`. In a herdr terminal a terminal editor gets a pane to the
/// right.
pub fn open(template: &str, path: &Path, line: u32) -> Open {
    let argv = argv(template, path, line);
    let name = Path::new(&argv[0])
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if gui(&name) {
        return Open::Detached(argv);
    }
    let herdr = std::env::var_os("HC_TERMINAL_ID").is_some_and(|v| !v.is_empty());
    if herdr {
        let split = serde_json::json!({ "direction": "right", "argv": argv });
        return Open::Detached(vec![
            "herdr-app".into(),
            "do".into(),
            "pane.split".into(),
            split.to_string(),
        ]);
    }
    Open::Here(argv)
}

/// Starts `argv` without waiting for it or giving it the terminal.
pub fn spawn(argv: &[String]) -> std::io::Result<()> {
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
    fn the_template_gets_the_file_and_line() {
        let p = Path::new("/r/src/a.rs");
        assert_eq!(argv("hx {file}:{line}", p, 12), ["hx", "/r/src/a.rs:12"]);
        assert_eq!(
            argv("nvim +{line} {file}", p, 3),
            ["nvim", "+3", "/r/src/a.rs"]
        );
        assert_eq!(argv("subl -w", p, 7), ["subl", "-w", "/r/src/a.rs"]);
    }
}
