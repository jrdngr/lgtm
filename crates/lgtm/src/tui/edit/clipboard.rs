//! The system clipboard, through macOS's `pbcopy` and `pbpaste`.

use std::io::Write;
use std::process::{Command, Stdio};

pub fn copy(text: &str) -> std::io::Result<()> {
    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(text.as_bytes())?;
    }
    child.wait()?;
    Ok(())
}

pub fn paste() -> std::io::Result<String> {
    let out = Command::new("pbpaste").output()?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
