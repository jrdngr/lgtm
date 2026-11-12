//! An issues folder: `ISSUES.md` and one `<id>-<slug>.md` file per issue.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

use crate::guide::GUIDE;
use crate::issue::Issue;

pub const MARKER: &str = "ISSUES.md";

pub struct Folder {
    pub root: PathBuf,
}

/// What a folder held at one moment: its files' names, sizes and times.
pub type Stamp = Vec<(String, u64, Option<SystemTime>)>;

impl Folder {
    pub fn open(dir: &Path) -> Result<Folder> {
        let root = dir
            .canonicalize()
            .with_context(|| format!("{} doesn't exist", dir.display()))?;
        if !root.join(MARKER).is_file() {
            bail!(
                "{} has no {MARKER}. Run `lgtm-issues init` there to start an issues folder.",
                root.display()
            );
        }
        Ok(Folder { root })
    }

    pub fn init(dir: &Path) -> Result<Folder> {
        let marker = dir.join(MARKER);
        if !marker.exists() {
            fs::create_dir_all(dir)?;
            fs::write(&marker, GUIDE)?;
        }
        Folder::open(dir)
    }

    fn files(&self) -> Result<Vec<(u32, PathBuf)>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let path = entry?.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let Some(stem) = name.strip_suffix(".md") else {
                continue;
            };
            let digits = stem.split('-').next().unwrap_or_default();
            if let Ok(id) = digits.parse::<u32>() {
                out.push((id, path));
            }
        }
        out.sort();
        Ok(out)
    }

    /// Every issue, and a line for each file that didn't read.
    pub fn load(&self) -> Result<(Vec<Issue>, Vec<String>)> {
        let mut issues = Vec::new();
        let mut bad = Vec::new();
        for (id, path) in self.files()? {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            match fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| Issue::parse(id, &t))
            {
                Ok(issue) => issues.push(issue),
                Err(e) => bad.push(format!("{name}: {e}")),
            }
        }
        Ok((issues, bad))
    }

    pub fn path(&self, id: u32) -> Result<PathBuf> {
        let mut found = self.files()?.into_iter().filter(|(i, _)| *i == id);
        match (found.next(), found.next()) {
            (Some((_, path)), None) => Ok(path),
            (None, _) => bail!("no issue #{id}"),
            (Some(_), Some(_)) => bail!("more than one file is issue #{id}"),
        }
    }

    #[cfg(test)]
    pub fn get(&self, id: u32) -> Result<Issue> {
        let path = self.path(id)?;
        let text = fs::read_to_string(&path)?;
        Issue::parse(id, &text).map_err(|e| anyhow::anyhow!("issue #{id}: {e}"))
    }

    /// Files `issue` under the next free id, which it returns.
    pub fn create(&self, mut issue: Issue) -> Result<u32> {
        let mut id = self.files()?.last().map_or(1, |(i, _)| i + 1);
        loop {
            issue.id = id;
            let path = self.root.join(issue.file_name());
            match fs::File::create_new(&path) {
                Ok(mut f) => {
                    f.write_all(issue.render().as_bytes())?;
                    return Ok(id);
                }
                Err(e) if e.kind() == ErrorKind::AlreadyExists => id += 1,
                Err(e) => return Err(e.into()),
            }
            if self.files()?.iter().any(|(i, _)| *i == id) {
                id += 1;
            }
        }
    }

    /// Reads issue `id`, changes it with `change`, and writes it back in place.
    pub fn update<T>(&self, id: u32, change: impl FnOnce(&mut Issue) -> Result<T>) -> Result<T> {
        let path = self.path(id)?;
        let mut issue = Issue::parse(id, &fs::read_to_string(&path)?)
            .map_err(|e| anyhow::anyhow!("issue #{id}: {e}"))?;
        let out = change(&mut issue)?;
        let tmp = path.with_extension("md.tmp");
        fs::write(&tmp, issue.render())?;
        fs::rename(&tmp, &path)?;
        Ok(out)
    }

    pub fn stamp(&self) -> Stamp {
        let Ok(dir) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out: Stamp = dir
            .flatten()
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                Some((
                    e.file_name().to_string_lossy().into_owned(),
                    meta.len(),
                    meta.modified().ok(),
                ))
            })
            .collect();
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::Status;

    fn folder() -> (tempfile::TempDir, Folder) {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        (dir, folder)
    }

    #[test]
    fn a_folder_without_the_marker_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let err = Folder::open(dir.path()).err().unwrap().to_string();
        assert!(err.contains("lgtm-issues init"), "{err}");
    }

    #[test]
    fn new_issues_take_the_next_id_after_the_highest() {
        let (_dir, folder) = folder();
        let a = folder.create(Issue::new(0, "First", None, "", 0)).unwrap();
        fs::write(
            folder.root.join("9-by-hand.md"),
            "---\ntitle: By hand\nstatus: open\n---\n",
        )
        .unwrap();
        let b = folder.create(Issue::new(0, "Second", None, "", 0)).unwrap();
        assert_eq!((a, b), (1, 10));
        assert!(folder.root.join("10-second.md").is_file());
    }

    #[test]
    fn updates_keep_the_file_name_and_bad_files_are_reported() {
        let (_dir, folder) = folder();
        let id = folder
            .create(Issue::new(0, "Old title", None, "", 0))
            .unwrap();
        folder
            .update(id, |i| {
                i.title = "New title".into();
                i.status = Status::Working;
                Ok(())
            })
            .unwrap();
        assert_eq!(folder.get(id).unwrap().status, Status::Working);
        assert!(folder.root.join("1-old-title.md").is_file());
        fs::write(folder.root.join("2-broken.md"), "no header").unwrap();
        let (issues, bad) = folder.load().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].starts_with("2-broken.md"));
    }
}
