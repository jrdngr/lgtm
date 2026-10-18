//! Where repos and the app's data live on disk.

use std::path::{Path, PathBuf};

use tauri::{Manager, Wry};

/// The main workspace root of the repo containing `path`: the nearest ancestor (inclusive)
/// holding a `.jj` directory or, when that's a secondary workspace, the main workspace its
/// `.jj/repo` file points to.
pub fn find_root(path: &Path) -> Option<PathBuf> {
    let root = path.ancestors().find(|p| p.join(".jj").is_dir())?;
    Some(main_root(root).unwrap_or_else(|| root.to_path_buf()))
}

/// The main workspace root a secondary workspace at `root` belongs to, from its `.jj/repo` file.
/// `None` for a main workspace, or when that file's path doesn't resolve.
pub fn main_root(root: &Path) -> Option<PathBuf> {
    let link = std::fs::read_to_string(root.join(".jj/repo")).ok()?;
    let store = root.join(".jj").join(link.trim()).canonicalize().ok()?;
    Some(store.parent()?.parent()?.to_path_buf())
}

/// Stable per-repo key, used for window labels and storage directories.
pub fn key(root: &Path) -> String {
    blake3::hash(root.as_os_str().as_encoded_bytes()).to_hex()[..16].to_string()
}

pub fn data_dir(app: &impl Manager<Wry>) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// Where the repo's mark store and layouts live.
pub fn repo_dir(data_dir: &Path, root: &Path) -> PathBuf {
    data_dir.join("repos").join(key(root))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_root_walks_up_to_nearest_jj_dir() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path();
        let inner = outer.join("vendor/inner");
        std::fs::create_dir_all(outer.join(".jj")).unwrap();
        std::fs::create_dir_all(inner.join(".jj")).unwrap();
        std::fs::create_dir_all(inner.join("src/deep")).unwrap();

        assert_eq!(find_root(&inner.join("src/deep")), Some(inner.clone()));
        assert_eq!(find_root(&outer.join("vendor")), Some(outer.to_path_buf()));
    }

    #[test]
    fn find_root_resolves_a_secondary_workspace_to_its_main_repo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let (main, ws) = (root.join("main"), root.join("ws"));
        std::fs::create_dir_all(main.join(".jj/repo")).unwrap();
        std::fs::create_dir_all(ws.join(".jj")).unwrap();
        std::fs::create_dir_all(ws.join("src")).unwrap();
        std::fs::write(ws.join(".jj/repo"), "../../main/.jj/repo").unwrap();
        assert_eq!(find_root(&ws.join("src")), Some(main));
    }

    #[test]
    fn find_root_ignores_jj_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".jj"), "").unwrap();
        assert_eq!(find_root(dir.path()), None);
    }
}
