//! An issues folder: `ISSUES.md`, and one `<id>-<slug>.md` file per issue in `issues/<status>`,
//! the folder for its status.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};

use crate::guide::GUIDE;
use crate::issue::Issue;
use crate::workflow::{self, Verb, Workflow};

pub const MARKER: &str = "ISSUES.md";

/// An issue's id from its file name, `12-login-loops.md`.
fn file_id(path: &Path) -> Option<u32> {
    let name = path.file_name()?.to_str()?;
    name.strip_suffix(".md")?.split('-').next()?.parse().ok()
}

fn read(path: &Path, id: u32) -> Result<Issue, String> {
    fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|t| Issue::parse(id, &t))
}

/// Writes `issue` to `to` through a temporary file, then removes `from` when it was elsewhere.
fn write(issue: &Issue, from: Option<&Path>, to: &Path) -> Result<()> {
    let tmp = to.with_extension("md.tmp");
    fs::write(&tmp, issue.render())?;
    fs::rename(&tmp, to)?;
    if let Some(from) = from.filter(|f| *f != to) {
        fs::remove_file(from)?;
    }
    Ok(())
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
        folder.upgrade()?;
        folder.place_loose()?;
        Ok(folder)
    }

    pub fn init(dir: &Path) -> Result<Folder> {
        let marker = dir.join(MARKER);
        if !marker.exists() {
            fs::create_dir_all(dir)?;
            fs::write(&marker, GUIDE)?;
        }
        Folder::open(dir)
    }

    pub fn issues_dir(&self) -> PathBuf {
        self.root.join("issues")
    }

    fn status_dir(&self, status: &str) -> PathBuf {
        self.issues_dir().join(status)
    }

    pub fn workflow(&self) -> Result<Workflow> {
        Workflow::load(&self.issues_dir())
    }

    /// Starts the default workflow in a new folder, and in one from before status folders,
    /// where issues sat in `issues/open` and `issues/complete` with a `status:` header: each
    /// moves to its status's folder and loses the header.
    fn upgrade(&self) -> Result<()> {
        let issues = self.issues_dir();
        let has_status_file = workflow::folders(&issues)?
            .iter()
            .any(|s| issues.join(s).join(workflow::STATUS_FILE).is_file());
        let old = issues.join("complete").is_dir()
            && !issues.join(workflow::FILE).is_file()
            && !has_status_file;
        if issues.is_dir() && !old {
            return Ok(());
        }
        workflow::write_default(&issues)?;
        let marker = self.root.join(MARKER);
        if fs::read_to_string(&marker).is_ok_and(|t| t.contains("`issues/complete`")) {
            fs::write(&marker, GUIDE)?;
        }
        let mut files = md_files(&self.root)?;
        for shelf in ["open", "complete"] {
            files.extend(md_files(&issues.join(shelf))?);
        }
        for path in files {
            let Some(id) = file_id(&path) else { continue };
            let Ok(issue) = read(&path, id) else { continue };
            let status = match issue.status.as_str() {
                "needs-input" => "working",
                s if !s.is_empty() && issues.join(s).is_dir() => s,
                _ => "open",
            };
            let to = issues.join(status).join(path.file_name().expect("name"));
            write(&issue, Some(&path), &to)?;
        }
        let _ = fs::remove_dir(issues.join("complete"));
        Ok(())
    }

    /// Moves issue files left right in the folder or in `issues/` into a status folder: the
    /// one their `status:` header names, else where new issues go.
    fn place_loose(&self) -> Result<()> {
        let mut loose = md_files(&self.root)?;
        loose.extend(md_files(&self.issues_dir())?);
        loose.retain(|p| file_id(p).is_some());
        if loose.is_empty() {
            return Ok(());
        }
        let workflow = self.workflow()?;
        for path in loose {
            let id = file_id(&path).expect("checked");
            let Ok(issue) = read(&path, id) else { continue };
            let status = match workflow.status(&issue.status) {
                Some(s) => s,
                None => workflow.verb(Verb::New)?,
            };
            let to = self
                .status_dir(&status.id)
                .join(path.file_name().expect("name"));
            if !to.exists() {
                fs::rename(&path, &to)?;
            }
        }
        Ok(())
    }

    fn files(&self) -> Result<Vec<(u32, PathBuf)>> {
        let mut out = Vec::new();
        for status in workflow::folders(&self.issues_dir())? {
            for path in md_files(&self.status_dir(&status))? {
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
        self.place_loose()?;
        let mut issues = Vec::new();
        let mut bad = Vec::new();
        for (id, path) in self.files()? {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            match read(&path, id) {
                Ok(mut issue) => {
                    issue.status = status_of(&path);
                    issues.push(issue);
                }
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

    /// Issue `id` as it is on disk, with its status.
    pub fn get(&self, id: u32) -> Result<Issue> {
        let path = self.path(id)?;
        let mut issue = read(&path, id).map_err(|e| anyhow::anyhow!("issue #{id}: {e}"))?;
        issue.status = status_of(&path);
        Ok(issue)
    }

    /// Files `issue` in its status's folder under the next free id, which it returns.
    pub fn create(&self, mut issue: Issue) -> Result<u32> {
        let dir = self.status_dir(&issue.status);
        if issue.status.is_empty() || !dir.is_dir() {
            bail!("no status {}", issue.status);
        }
        let mut id = self.files()?.last().map_or(1, |(i, _)| i + 1);
        loop {
            issue.id = id;
            let path = dir.join(issue.file_name());
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

    /// Reads issue `id`, changes it with `change`, and writes it back under its name, in the
    /// folder for its status now.
    pub fn update<T>(&self, id: u32, change: impl FnOnce(&mut Issue) -> Result<T>) -> Result<T> {
        let path = self.path(id)?;
        let mut issue = self.get(id)?;
        let out = change(&mut issue)?;
        let dir = self.status_dir(&issue.status);
        if !dir.is_dir() {
            bail!("no status {}", issue.status);
        }
        write(
            &issue,
            Some(&path),
            &dir.join(path.file_name().expect("name")),
        )?;
        Ok(out)
    }

    pub fn stamp(&self) -> Stamp {
        let issues = self.issues_dir();
        let mut dirs = vec![self.root.clone(), issues.clone()];
        for s in workflow::folders(&issues).unwrap_or_default() {
            dirs.push(issues.join(s));
        }
        let mut out = Stamp::new();
        for dir in dirs {
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

/// The status an issue file is in: its folder's name.
fn status_of(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
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

    fn folder() -> (tempfile::TempDir, Folder) {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        (dir, folder)
    }

    fn issue(title: &str) -> Issue {
        let mut i = Issue::new(0, title, None, "", 0);
        i.status = "open".into();
        i
    }

    #[test]
    fn a_folder_without_the_marker_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let err = Folder::open(dir.path()).err().unwrap().to_string();
        assert!(err.contains("lgtm issues init"), "{err}");
    }

    #[test]
    fn a_new_folder_starts_with_the_default_workflow() {
        let (_dir, folder) = folder();
        let w = folder.workflow().unwrap();
        let ids: Vec<&str> = w.statuses.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["open", "working", "done", "dropped"]);
        assert_eq!(w.verb(Verb::Close).unwrap().id, "done");
    }

    #[test]
    fn new_issues_take_the_next_id_after_the_highest() {
        let (_dir, folder) = folder();
        let a = folder.create(issue("First")).unwrap();
        fs::write(
            folder.root.join("issues/done/9-by-hand.md"),
            "---\ntitle: By hand\n---\n",
        )
        .unwrap();
        let b = folder.create(issue("Second")).unwrap();
        assert_eq!((a, b), (1, 10));
        assert!(folder.root.join("issues/open/10-second.md").is_file());
        let mut nowhere = issue("Nowhere");
        nowhere.status = "later".into();
        assert!(folder.create(nowhere).is_err());
    }

    #[test]
    fn an_issue_moves_with_its_status_and_bad_files_are_reported() {
        let (_dir, folder) = folder();
        let id = folder.create(issue("Old title")).unwrap();
        folder
            .update(id, |i| {
                i.title = "New title".into();
                i.status = "dropped".into();
                Ok(())
            })
            .unwrap();
        assert_eq!(folder.get(id).unwrap().status, "dropped");
        assert!(folder.root.join("issues/dropped/1-old-title.md").is_file());
        assert!(!folder.root.join("issues/open/1-old-title.md").exists());
        let later = folder.update(id, |i| {
            i.status = "later".into();
            Ok(())
        });
        assert!(later.is_err());
        fs::write(folder.root.join("issues/open/2-broken.md"), "no header").unwrap();
        let (issues, bad) = folder.load().unwrap();
        assert_eq!(issues.len(), 1);
        assert!(bad[0].starts_with("2-broken.md"));
    }

    #[test]
    fn a_folder_from_before_status_folders_moves_each_issue_to_its_status() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(
            root.join(MARKER),
            "Issue files go in `issues/complete` when done.",
        )
        .unwrap();
        for (path, status) in [
            ("issues/open/1-a.md", "open"),
            ("issues/open/2-b.md", "needs-input"),
            ("issues/complete/3-c.md", "dropped"),
            ("4-d.md", "done"),
        ] {
            fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            let text = format!("---\ntitle: T\nstatus: {status}\nowner: x\n---\n");
            fs::write(root.join(path), text).unwrap();
        }
        let folder = Folder::open(root).unwrap();
        let (issues, bad) = folder.load().unwrap();
        assert!(bad.is_empty(), "{bad:?}");
        let at: Vec<(u32, &str)> = issues.iter().map(|i| (i.id, i.status.as_str())).collect();
        assert_eq!(
            at,
            [(1, "open"), (2, "working"), (3, "dropped"), (4, "done")]
        );
        let text = fs::read_to_string(root.join("issues/working/2-b.md")).unwrap();
        assert!(!text.contains("status:"), "{text}");
        assert!(text.contains("owner: x"), "{text}");
        assert!(!root.join("issues/complete").exists());
        assert_eq!(fs::read_to_string(root.join(MARKER)).unwrap(), GUIDE);
    }

    #[test]
    fn loose_files_go_to_the_status_they_name_else_where_new_issues_go() {
        let (dir, folder) = folder();
        let file = |path: &str, text: &str| fs::write(folder.root.join(path), text).unwrap();
        file("3-old.md", "---\ntitle: Old\nstatus: done\n---\n");
        file("issues/4-new.md", "---\ntitle: New\n---\n");
        file("notes.md", "not an issue");
        let folder = Folder::open(dir.path()).unwrap();
        assert!(folder.root.join("issues/done/3-old.md").is_file());
        assert!(folder.root.join("issues/open/4-new.md").is_file());
        assert!(folder.root.join("notes.md").is_file());
        assert_eq!(folder.load().unwrap().0.len(), 2);
    }
}
