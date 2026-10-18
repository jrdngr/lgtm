//! `lgtm gc`: removes workspace directories whose workspace is gone, and forgets workspaces whose
//! directory is gone.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{same_path, under};
use crate::jj::git::worktree_prune_blocking;
use crate::jj::workspace::{
    bookmark_delete_blocking, bookmarks_blocking, forget_blocking, workspaces_blocking,
};
use crate::repo::main_root;

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

/// Deletes `lgtm/pr-N` bookmarks without a `pr-N` workspace, which deleted PR workspaces left.
fn clean_prs(root: &Path, log: &mut Vec<String>) {
    let Ok(listed) = workspaces_blocking(root) else {
        return;
    };
    let has = |name: &str| listed.iter().any(|w| w.name == name);
    for b in bookmarks_blocking(root, "lgtm/pr-*").unwrap_or_default() {
        let ws = b.trim_start_matches("lgtm/");
        if has(ws) {
            continue;
        }
        match bookmark_delete_blocking(root, &b) {
            Ok(()) => log.push(format!("deleted bookmark {b} ({ws} is gone)")),
            Err(e) => log.push(format!("{e:#}")),
        }
    }
}

/// Cleans up `base` (laid out `<repo>/<workspace>`). `repos` are main workspace roots to check
/// for forgotten directories as well as the ones found under `base`. A workspace whose repo
/// can't be found (moved, or on an unmounted volume) is kept unless `force`. Returns what it did.
pub fn run(base: &Path, repos: &[PathBuf], force: bool) -> Vec<String> {
    let mut log = Vec::new();
    let mut roots: BTreeSet<PathBuf> = repos.iter().cloned().collect();
    for repo_dir in subdirs(base) {
        for dir in subdirs(&repo_dir) {
            let Some(root) = main_root(&dir) else {
                if !dir.join(".jj").exists() {
                    remove(&dir, "it isn't a workspace", &mut log);
                } else if force {
                    remove(&dir, "its repo can't be found", &mut log);
                } else {
                    log.push(format!(
                        "skipped {}: its repo can't be found (--force removes it)",
                        dir.display()
                    ));
                }
                continue;
            };
            match workspaces_blocking(&root) {
                Ok(listed) if !listed.iter().any(|w| same_path(&w.root, &dir)) => {
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
        clean_prs(root, &mut log);
        worktree_prune_blocking(root);
    }
    for repo_dir in subdirs(base) {
        // Fails unless empty, which is the point.
        std::fs::remove_dir(&repo_dir).ok();
    }
    log
}

/// Entry point for `lgtm gc [--force] [repo]`. Also checks the repo containing the current
/// directory.
pub fn main(args: &[String]) -> i32 {
    let Some(base) = super::ws_base() else {
        eprintln!("lgtm gc: no home directory");
        return 1;
    };
    let cwd = std::env::current_dir().unwrap_or_default();
    let force = args.iter().any(|a| a == "--force");
    let repos: Vec<PathBuf> = args
        .iter()
        .filter(|a| *a != "--force")
        .map(|a| cwd.join(a))
        .chain([cwd.clone()])
        .filter_map(|p| crate::repo::find_root(&p))
        .collect();
    let log = run(&base, &repos, force);
    if log.is_empty() {
        println!("lgtm gc: nothing to clean up in {}", base.display());
    }
    for line in log {
        println!("{line}");
    }
    0
}
