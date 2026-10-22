//! Where a workspace's files are. jj records a workspace's path when it's added, but not for
//! workspaces older than that record, and not again when its directory moves. A directory
//! that's a workspace names itself in `.jj/working_copy/checkout`, so lgtm checks jj's path
//! against that and looks nearby when it's wrong.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::jj::WorkspaceInfo;

/// The workspace name in a `.jj/working_copy/checkout` file: field 3 of jj's `Checkout`
/// protobuf.
fn checkout_name(bytes: &[u8]) -> Option<String> {
    let mut at = 0;
    while at < bytes.len() {
        let key = varint(bytes, &mut at)?;
        let (field, wire) = (key >> 3, key & 7);
        match wire {
            0 => {
                varint(bytes, &mut at)?;
            }
            1 => at += 8,
            2 => {
                let len = usize::try_from(varint(bytes, &mut at)?).ok()?;
                let value = bytes.get(at..at.checked_add(len)?)?;
                if field == 3 {
                    return String::from_utf8(value.to_vec()).ok();
                }
                at += len;
            }
            5 => at += 4,
            _ => return None,
        }
    }
    None
}

fn varint(bytes: &[u8], at: &mut usize) -> Option<u64> {
    let mut out = 0u64;
    for shift in (0..64).step_by(7) {
        let b = *bytes.get(*at)?;
        *at += 1;
        out |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Some(out);
        }
    }
    None
}

/// The repo store directory a workspace at `root` uses: `.jj/repo` itself in the main
/// workspace, or where the `.jj/repo` file points in the others.
fn repo_dir(root: &Path) -> Option<PathBuf> {
    let link = root.join(".jj/repo");
    if link.is_dir() {
        return link.canonicalize().ok();
    }
    let target = std::fs::read_to_string(&link).ok()?;
    root.join(".jj").join(target.trim()).canonicalize().ok()
}

/// The name of the workspace at `root` when it's one of the repo stored in `repo`.
pub fn workspace_at(root: &Path, repo: &Path) -> Option<String> {
    if repo_dir(root)? != repo {
        return None;
    }
    checkout_name(&std::fs::read(root.join(".jj/working_copy/checkout")).ok()?)
}

/// Directories a workspace of the repo at `main` is likely in: inside it, beside it, in its
/// `.workspaces`, and in lgtm's own `base`.
fn candidates(main: &Path, base: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = vec![main.to_path_buf(), main.join(".workspaces")];
    dirs.extend(main.parent().map(Path::to_path_buf));
    dirs.extend(base.map(Path::to_path_buf));
    let mut out = vec![main.to_path_buf()];
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        out.extend(
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.join(".jj").is_dir()),
        );
    }
    out
}

/// Fixes the roots jj reported for the workspaces of the repo at `main`. A root that's
/// missing or holds another workspace is replaced by `saved`'s path for that name, else by a
/// workspace directory nearby that names itself so. Returns the names still not found.
pub fn fix_roots(
    listed: &mut [WorkspaceInfo],
    main: &Path,
    base: Option<&Path>,
    saved: &HashMap<String, PathBuf>,
) -> Vec<String> {
    let Some(repo) = repo_dir(main) else {
        return vec![];
    };
    let is = |root: &Path, name: &str| workspace_at(root, &repo).as_deref() == Some(name);
    let mut nearby: Option<HashMap<String, PathBuf>> = None;
    let mut missing = Vec::new();
    for w in listed.iter_mut() {
        if is(&w.root, &w.name) {
            continue;
        }
        if let Some(p) = saved.get(&w.name).filter(|p| is(p, &w.name)) {
            w.root = p.clone();
            continue;
        }
        let found = nearby.get_or_insert_with(|| {
            candidates(main, base)
                .into_iter()
                .filter_map(|p| Some((workspace_at(&p, &repo)?, p)))
                .collect()
        });
        match found.get(&w.name) {
            Some(p) => w.root = p.clone(),
            None => missing.push(w.name.clone()),
        }
    }
    missing
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jj(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("jj")
            .current_dir(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    }

    fn listed(main: &Path) -> Vec<WorkspaceInfo> {
        crate::jj::workspace::workspaces_blocking(main).unwrap()
    }

    #[test]
    fn finds_workspaces_jj_has_no_path_for_or_an_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        jj(&top, &["git", "init", "main"]);
        let main = top.join("main");
        jj(&main, &["workspace", "add", "../rework", "--name", "full-impl"]);
        jj(&main, &["workspace", "add", "../gone", "--name", "gone"]);
        std::fs::rename(top.join("rework"), top.join("moved")).unwrap();
        std::fs::remove_dir_all(top.join("gone")).unwrap();

        let mut ws = listed(&main);
        let missing = fix_roots(&mut ws, &main, None, &HashMap::new());
        let root = |name: &str| ws.iter().find(|w| w.name == name).unwrap().root.clone();
        assert_eq!(root("full-impl"), top.join("moved"));
        assert_eq!(root("default"), main);
        assert_eq!(missing, ["gone"]);

        // Without jj's record of paths at all, as for workspaces older than it.
        std::fs::remove_file(main.join(".jj/repo/workspace_store/index")).unwrap();
        let mut ws = listed(&main);
        assert!(ws.iter().all(|w| w.root.as_os_str().is_empty()));
        fix_roots(&mut ws, &main, None, &HashMap::new());
        let root = |name: &str| ws.iter().find(|w| w.name == name).unwrap().root.clone();
        assert_eq!(root("full-impl"), top.join("moved"));
        assert_eq!(root("default"), main);
    }

    #[test]
    fn a_saved_path_counts_only_when_it_holds_that_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().canonicalize().unwrap();
        jj(&top, &["git", "init", "main"]);
        let main = top.join("main");
        let far = top.join("far/away");
        std::fs::create_dir_all(top.join("far")).unwrap();
        jj(&main, &["workspace", "add", "../far/away", "--name", "a"]);
        jj(&main, &["workspace", "add", "../b", "--name", "b"]);
        std::fs::rename(&far, top.join("far/there")).unwrap();

        let saved = HashMap::from([
            ("a".to_string(), top.join("far/there")),
            ("b".to_string(), top.join("far/there")),
        ]);
        let mut ws = listed(&main);
        let missing = fix_roots(&mut ws, &main, None, &saved);
        let root = |name: &str| ws.iter().find(|w| w.name == name).unwrap().root.clone();
        assert_eq!(root("a"), top.join("far/there"));
        assert_eq!(root("b"), top.join("b"));
        assert!(missing.is_empty());
    }

    #[test]
    fn reads_the_name_past_other_fields() {
        // Field 2 (a 3-byte operation id), a varint field 5, then field 3.
        let mut bytes = vec![0x12, 3, 1, 2, 3, 0x28, 0x96, 0x01, 0x1a, 4];
        bytes.extend(b"auth");
        assert_eq!(checkout_name(&bytes).as_deref(), Some("auth"));
        assert_eq!(checkout_name(&[0x12, 9, 1]), None);
    }
}
