//! Opening the editor at a line: in a pane beside this one in the herdr desktop app, as its
//! own window for a GUI editor, else in this terminal while the review waits.

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

/// The user's editor: `$VISUAL`, else `$EDITOR`, else vi.
fn editor() -> String {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|e| !e.trim().is_empty())
        .unwrap_or_else(|| "vi".into())
}

/// Editors with windows of their own.
fn gui(name: &str) -> bool {
    matches!(
        name,
        "code" | "code-insiders" | "cursor" | "windsurf" | "zed" | "subl" | "sublime_text"
    )
}

/// The command that opens `path` at `line` in `editor`, a command line with any arguments.
pub fn argv(editor: &str, path: &Path, line: u32) -> Vec<String> {
    let mut argv: Vec<String> = editor.split_whitespace().map(String::from).collect();
    let name = argv
        .first()
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let path = path.to_string_lossy();
    match name.as_str() {
        "code" | "code-insiders" | "cursor" | "windsurf" => {
            argv.extend(["-g".into(), format!("{path}:{line}")])
        }
        "zed" | "subl" | "sublime_text" => argv.push(format!("{path}:{line}")),
        _ => argv.extend([format!("+{line}"), path.into_owned()]),
    }
    argv
}

/// How to open `path` at `line`. In a herdr terminal a terminal editor gets a pane to the
/// right.
pub fn open(path: &Path, line: u32) -> Open {
    let editor = editor();
    let argv = argv(&editor, path, line);
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
    fn each_kind_of_editor_gets_the_line_its_own_way() {
        let p = Path::new("/r/src/a.rs");
        assert_eq!(argv("nvim", p, 12), ["nvim", "+12", "/r/src/a.rs"]);
        assert_eq!(argv("code -w", p, 3), ["code", "-w", "-g", "/r/src/a.rs:3"]);
        assert_eq!(
            argv("/usr/local/bin/zed", p, 7),
            ["/usr/local/bin/zed", "/r/src/a.rs:7"]
        );
    }
}
