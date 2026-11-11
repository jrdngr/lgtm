use std::fs;
use std::path::{Path, PathBuf};

/// Dock layouts are opaque JSON owned by the UI, one file per workspace: `layout.json` for
/// the main one, `layouts/<name>.json` for the others.
pub fn path(repo_dir: &Path, workspace: Option<&str>) -> PathBuf {
    match workspace {
        None => repo_dir.join("layout.json"),
        Some(name) => repo_dir.join("layouts").join(format!("{name}.json")),
    }
}

pub fn remove(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
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
