//! `lgtm gc`: removes workspace directories whose workspace is gone, and forgets workspaces whose
//! directory is gone.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::under;
use crate::jj::workspace::{forget_blocking, workspaces_blocking};

/// The main workspace root of the repo a workspace directory belongs to, from the path in its
/// `.jj/repo` file. `None` when it isn't a workspace or its repo is gone.
fn main_root(dir: &Path) -> Option<PathBuf> {
    let link = std::fs::read_to_string(dir.join(".jj/repo")).ok()?;
    let store = dir.join(".jj").join(link.trim()).canonicalize().ok()?;
    Some(store.parent()?.parent()?.to_path_buf())
}

fn same(a: &Path, b: &Path) -> bool {
    a.canonicalize().ok() == b.canonicalize().ok()
}

fn remove(dir: &Path, why: &str, log: &mut Vec<String>) {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => log.push(format!("removed {} ({why})", dir.display())),
        Err(e) => log.push(format!("couldn't remove {}: {e}", dir.display())),
    }
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return vec![];
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect()
}

/// Cleans up `base` (laid out `<repo>/<workspace>`). `repos` are main workspace roots to check
/// for forgotten directories as well as the ones found under `base`. Returns what it did.
pub fn run(base: &Path, repos: &[PathBuf]) -> Vec<String> {
    let mut log = Vec::new();
    let mut roots: BTreeSet<PathBuf> = repos.iter().cloned().collect();
    for repo_dir in subdirs(base) {
        for dir in subdirs(&repo_dir) {
            let Some(root) = main_root(&dir) else {
                remove(&dir, "its repo is gone", &mut log);
                continue;
            };
            match workspaces_blocking(&root) {
                Ok(listed) if !listed.iter().any(|w| same(&w.root, &dir)) => {
                    remove(&dir, "its workspace was forgotten", &mut log);
                }
                Ok(_) => {}
                Err(e) => log.push(format!("skipped {}: {e:#}", dir.display())),
            }
            roots.insert(root);
        }
    }
    for root in &roots {
        let Ok(listed) = workspaces_blocking(root) else {
            continue;
        };
        for w in listed
            .iter()
            .filter(|w| under(&w.root, base) && !w.root.exists())
        {
            match forget_blocking(root, &w.name) {
                Ok(()) => log.push(format!("forgot workspace {} (directory is gone)", w.name)),
                Err(e) => log.push(format!("{e:#}")),
            }
        }
        if root.join(".git").exists() {
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(["worktree", "prune"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .ok();
        }
    }
    for repo_dir in subdirs(base) {
        // Fails unless empty, which is the point.
        std::fs::remove_dir(&repo_dir).ok();
    }
    log
}

/// Entry point for `lgtm gc [repo]`. Also checks the repo containing the current directory.
pub fn main(args: &[String]) -> i32 {
    let Some(base) = super::ws_base() else {
        eprintln!("lgtm gc: no home directory");
        return 1;
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let repos: Vec<PathBuf> = args
        .iter()
        .map(|a| cwd.join(a))
        .chain([cwd.clone()])
        .filter_map(|p| crate::repo::find_root(&p))
        .collect();
    let log = run(&base, &repos);
    if log.is_empty() {
        println!("lgtm gc: nothing to clean up in {}", base.display());
    }
    for line in log {
        println!("{line}");
    }
    0
}
