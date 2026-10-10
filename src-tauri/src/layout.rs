use std::fs;
use std::path::{Path, PathBuf};

/// Dock layouts are opaque JSON owned by the UI, stored one file per repo.
pub fn path(data_dir: &Path, repo_key: &str) -> PathBuf {
    data_dir.join("repos").join(repo_key).join("layout.json")
}

pub fn load(path: &Path) -> std::io::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Writes through a temp file so a crash mid-write never leaves a truncated layout.
pub fn save(path: &Path, layout: &str) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().expect("layout path has a parent"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, layout)?;
    fs::rename(tmp, path)
}
