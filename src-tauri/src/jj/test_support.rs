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

/// Holds the flock on `path`, as another jj would, until the file drops.
pub fn hold(path: &Path) -> std::fs::File {
    let file = std::fs::File::create(path).unwrap();
    rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive).unwrap();
    file
}

/// The repo-wide lock every snapshot and write in a colocated repo takes first.
pub fn git_lock(root: &Path) -> std::path::PathBuf {
    root.join(".jj/repo/git_import_export.lock")
}

/// PIDs with `path` open.
pub fn openers(path: &Path) -> Vec<u32> {
    let out = Command::new("lsof").arg("-t").arg(path).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}
