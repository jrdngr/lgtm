//! An issues folder: `ISSUES.md`, and one `<id>-<slug>.md` file per issue in `issues/open`
//! or `issues/complete` by its status.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

use crate::guide::GUIDE;
use crate::issue::{Issue, Status};

pub const MARKER: &str = "ISSUES.md";

const SHELVES: [&str; 2] = ["open", "complete"];

/// The folder under `issues` that an issue with `status` goes in.
fn shelf(status: Status) -> &'static str {
    match status {
        Status::Done | Status::Dropped => "complete",
        _ => "open",
    }
}

/// An issue's id from its file name, `12-login-loops.md`.
fn file_id(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    name.strip_suffix(".md")?.split('-').next()?.parse().ok()
}

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
                "{} has no {MARKER}. Run `lgtm issues init` there to start an issues folder.",
                root.display()
            );
        }
        let folder = Folder { root };
        folder.shelve()?;
        Ok(folder)
    }

    fn shelf_dir(&self, name: &str) -> PathBuf {
        self.root.join("issues").join(name)
    }

    /// Makes the shelves, and moves each issue file that isn't on its status's shelf there:
    /// ones from before the shelves, and ones whose status was changed by hand.
    fn shelve(&self) -> Result<()> {
        for name in SHELVES {
            fs::create_dir_all(self.shelf_dir(name))?;
        }
        let mut loose: Vec<PathBuf> = md_files(&self.root)?;
        for name in SHELVES {
            loose.extend(md_files(&self.shelf_dir(name))?);
        }
        for path in loose {
            let Some(id) = file_id(&path) else { continue };
            let Ok(issue) = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| Issue::parse(id, &t))
            else {
                continue;
            };
            let to = self
                .shelf_dir(shelf(issue.status))
                .join(path.file_name().expect("name"));
            if to != path && !to.exists() {
                fs::rename(&path, &to)?;
            }
        }
        Ok(())
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
        for name in SHELVES {
            for path in md_files(&self.shelf_dir(name))? {
                if let Some(id) = file_id(&path) {
                    out.push((id, path));
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// Every issue, and a line for each file that didn't read.
    pub fn load(&self) -> Result<(Vec<Issue>, Vec<String>)> {
        self.shelve()?;
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
            let path = self.shelf_dir(shelf(issue.status)).join(issue.file_name());
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

    /// Reads issue `id`, changes it with `change`, and writes it back under its name, on the
    /// shelf for its status now.
    pub fn update<T>(&self, id: u32, change: impl FnOnce(&mut Issue) -> Result<T>) -> Result<T> {
        let path = self.path(id)?;
        let mut issue = Issue::parse(id, &fs::read_to_string(&path)?)
            .map_err(|e| anyhow::anyhow!("issue #{id}: {e}"))?;
        let out = change(&mut issue)?;
        let to = self
            .shelf_dir(shelf(issue.status))
            .join(path.file_name().expect("name"));
        let tmp = to.with_extension("md.tmp");
        fs::write(&tmp, issue.render())?;
        fs::rename(&tmp, &to)?;
        if to != path {
            fs::remove_file(&path)?;
        }
        Ok(out)
    }

    pub fn stamp(&self) -> Stamp {
        let mut out = Stamp::new();
        for dir in [self.root.clone()]
            .into_iter()
            .chain(SHELVES.map(|s| self.shelf_dir(s)))
        {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            out.extend(entries.flatten().filter_map(|e| {
                let meta = e.metadata().ok()?;
                Some((
                    e.path().to_string_lossy().into_owned(),
                    meta.len(),
                    meta.modified().ok(),
                ))
            }));
        }
        out.sort();
        out
    }
}

/// The `.md` files right in `dir`, none when it's missing.
fn md_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.is_file() && path.extension().is_some_and(|x| x == "md") {
            out.push(path);
        }
    }
    Ok(out)
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
        assert!(err.contains("lgtm issues init"), "{err}");
    }

    #[test]
    fn new_issues_take_the_next_id_after_the_highest() {
        let (_dir, folder) = folder();
        let a = folder.create(Issue::new(0, "First", None, "", 0)).unwrap();
        fs::write(
            folder.root.join("issues/complete/9-by-hand.md"),
            "---\ntitle: By hand\nstatus: done\n---\n",
        )
        .unwrap();
        let b = folder.create(Issue::new(0, "Second", None, "", 0)).unwrap();
        assert_eq!((a, b), (1, 10));
        assert!(folder.root.join("issues/open/10-second.md").is_file());
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
        assert!(folder.root.join("issues/open/1-old-title.md").is_file());
        fs::write(folder.root.join("issues/open/2-broken.md"), "no header").unwrap();
        let (issues, bad) = folder.load().unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].starts_with("2-broken.md"));
    }

    #[test]
    fn issues_move_between_open_and_complete_with_their_status() {
        let (_dir, folder) = folder();
        let id = folder.create(Issue::new(0, "Loops", None, "", 0)).unwrap();
        let set = |status| {
            folder
                .update(id, |i| {
                    i.status = status;
                    Ok(())
                })
                .unwrap()
        };
        set(Status::Dropped);
        assert!(folder.root.join("issues/complete/1-loops.md").is_file());
        assert!(!folder.root.join("issues/open/1-loops.md").exists());
        set(Status::NeedsInput);
        assert!(folder.root.join("issues/open/1-loops.md").is_file());
        assert!(!folder.root.join("issues/complete/1-loops.md").exists());
    }

    #[test]
    fn loose_and_misplaced_files_go_on_their_shelf_when_the_folder_opens() {
        let (dir, folder) = folder();
        fs::write(
            folder.root.join("3-old.md"),
            "---\ntitle: Old\nstatus: done\n---\n",
        )
        .unwrap();
        fs::write(
            folder.root.join("issues/complete/4-edited.md"),
            "---\ntitle: Edited\nstatus: working\n---\n",
        )
        .unwrap();
        fs::write(folder.root.join("notes.md"), "not an issue").unwrap();
        let folder = Folder::open(dir.path()).unwrap();
        assert!(folder.root.join("issues/complete/3-old.md").is_file());
        assert!(folder.root.join("issues/open/4-edited.md").is_file());
        assert!(folder.root.join("notes.md").is_file());
        assert_eq!(folder.load().unwrap().0.len(), 2);
    }
}
