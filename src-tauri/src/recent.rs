//! Recently opened repos, most recent first.

use std::path::{Path, PathBuf};

const MAX: usize = 10;

pub fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("recent.json")
}

/// Saved roots that are still jj repos. A missing or unreadable file is an empty list.
pub fn load(file: &Path) -> Vec<PathBuf> {
    std::fs::read(file)
        .ok()
        .and_then(|b| serde_json::from_slice::<Vec<PathBuf>>(&b).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.join(".jj").is_dir())
        .collect()
}

/// Moves `root` to the front of the list.
pub fn add(file: &Path, root: &Path) -> std::io::Result<()> {
    let mut roots = load(file);
    roots.retain(|p| p != root);
    roots.insert(0, root.to_path_buf());
    roots.truncate(MAX);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(file, serde_json::to_vec(&roots)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_moves_to_front_dedups_caps_and_drops_missing_repos() {
        let dir = tempfile::tempdir().unwrap();
        let file = path(dir.path());
        let repos: Vec<PathBuf> = (0..=MAX).map(|i| dir.path().join(i.to_string())).collect();
        for r in &repos {
            std::fs::create_dir_all(r.join(".jj")).unwrap();
            add(&file, r).unwrap();
        }
        add(&file, &repos[MAX - 1]).unwrap();
        std::fs::remove_dir_all(&repos[MAX - 2]).unwrap();

        let got = load(&file);
        assert_eq!(got.len(), MAX - 1);
        assert_eq!(got[..2], [repos[MAX - 1].clone(), repos[MAX].clone()]);
        assert!(!got.contains(&repos[0]) && !got.contains(&repos[MAX - 2]));
    }
}
