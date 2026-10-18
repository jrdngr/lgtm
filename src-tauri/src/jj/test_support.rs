//! Real jj repos for tests.

use std::path::Path;
use std::process::Command;

use super::Jj;

pub fn jj(dir: &Path, args: &[&str]) {
    let out = Command::new("jj")
        .arg("-R")
        .arg(dir)
        .args(args)
        .output()
        .expect("jj on PATH");
    assert!(
        out.status.success(),
        "jj {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub fn repo() -> (tempfile::TempDir, Jj) {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new("jj")
        .args(["git", "init"])
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    jj(dir.path(), &["config", "set", "--repo", "user.name", "t"]);
    jj(
        dir.path(),
        &["config", "set", "--repo", "user.email", "t@t"],
    );
    let j = Jj::new(dir.path());
    (dir, j)
}

pub fn write(dir: &Path, path: &str, text: &str) {
    std::fs::write(dir.join(path), text).unwrap();
}
