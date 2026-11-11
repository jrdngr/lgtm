//! Colors for the files a change touches, as VS Code colors git status, on their names and
//! their folders' names in the file trees.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use lgtm_core::diff::FileStatus;
use ratatui::style::Color;

use super::theme::rgb;

/// How a change touches a file. A folder takes the first of these among its files, by how
/// much each needs attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Decor {
    Conflict,
    Added,
    Renamed,
    Modified,
}

impl Decor {
    /// Removed files aren't on disk, so they don't color their folders.
    pub fn of(status: FileStatus, conflict: bool) -> Option<Self> {
        match (status, conflict) {
            (_, true) => Some(Self::Conflict),
            (FileStatus::Removed, _) => None,
            (FileStatus::Added | FileStatus::Copied, _) => Some(Self::Added),
            (FileStatus::Renamed, _) => Some(Self::Renamed),
            (FileStatus::Modified, _) => Some(Self::Modified),
        }
    }

    /// VS Code's dark git decoration colors, as the app uses.
    pub fn color(self) -> Color {
        match self {
            Self::Conflict => rgb(0xe4676b),
            Self::Added => rgb(0x81b88b),
            Self::Renamed => rgb(0x73c991),
            Self::Modified => rgb(0xe2c08d),
        }
    }
}

/// The decorations of files and of the folders that hold them, by path.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Decorations {
    files: HashMap<String, Decor>,
    dirs: HashMap<String, Decor>,
}

impl Decorations {
    pub fn new(files: impl IntoIterator<Item = (String, Decor)>) -> Self {
        let mut d = Self::default();
        for (path, decor) in files {
            let mut dir = path.as_str();
            while let Some(i) = dir.rfind('/') {
                dir = &dir[..i];
                let at = d.dirs.entry(dir.to_string()).or_insert(decor);
                *at = (*at).min(decor);
            }
            d.files.insert(path, decor);
        }
        d
    }

    pub fn file(&self, path: &str) -> Option<Decor> {
        self.files.get(path).copied()
    }

    pub fn dir(&self, path: &str) -> Option<Decor> {
        self.dirs.get(path).copied()
    }
}

/// What `git status` says changed in the repo at `root` since `HEAD`, untracked files as
/// added. Empty when it isn't a git repo.
pub fn git(root: &Path) -> Decorations {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output();
    match out {
        Ok(out) if out.status.success() => parse_porcelain(&String::from_utf8_lossy(&out.stdout)),
        _ => Decorations::default(),
    }
}

/// Reads `git status --porcelain=v1 -z`, where a rename's entry is followed by its source.
fn parse_porcelain(out: &str) -> Decorations {
    let mut found = Vec::new();
    let mut entries = out.split('\0');
    while let Some(entry) = entries.next() {
        let (Some(xy), Some(path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        let decor = match xy {
            "DD" | "AU" | "UD" | "UA" | "DU" | "AA" | "UU" => Some(Decor::Conflict),
            _ if xy.starts_with('R') || xy.starts_with('C') => {
                entries.next();
                Some(match xy.starts_with('R') {
                    true => Decor::Renamed,
                    false => Decor::Added,
                })
            }
            "??" => Some(Decor::Added),
            _ if xy.contains('D') => None,
            _ if xy.contains('A') => Some(Decor::Added),
            _ => Some(Decor::Modified),
        };
        if let Some(d) = decor {
            found.push((path.to_string(), d));
        }
    }
    Decorations::new(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_takes_its_most_pressing_files_color() {
        let d = Decorations::new([
            ("src/a/x.rs".to_string(), Decor::Modified),
            ("src/b.rs".to_string(), Decor::Added),
            ("src/a/y.rs".to_string(), Decor::Conflict),
        ]);
        assert_eq!(d.dir("src"), Some(Decor::Conflict));
        assert_eq!(d.dir("src/a"), Some(Decor::Conflict));
        assert_eq!(d.file("src/b.rs"), Some(Decor::Added));
        assert_eq!(d.dir("src/b.rs"), None);
    }

    #[test]
    fn git_status_skips_deletions_and_a_renames_source() {
        let d = parse_porcelain(" M a.rs\0?? new/n.rs\0R  to.rs\0from.rs\0 D gone.rs\0UU c.rs\0");
        assert_eq!(d.file("a.rs"), Some(Decor::Modified));
        assert_eq!(d.file("new/n.rs"), Some(Decor::Added));
        assert_eq!(d.dir("new"), Some(Decor::Added));
        assert_eq!(d.file("to.rs"), Some(Decor::Renamed));
        assert_eq!(d.file("from.rs"), None);
        assert_eq!(d.file("gone.rs"), None);
        assert_eq!(d.file("c.rs"), Some(Decor::Conflict));
    }
}
