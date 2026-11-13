//! Undo and redo for changes made on the issues screen: each is an issue file as it was
//! before and after, so undoing puts back the exact file, where it was.

use std::fs;
use std::path::PathBuf;

use anyhow::{Result, bail};

use crate::folder::Folder;

/// An issue file at one moment: where it was and what it said, or None when it didn't exist.
type File = Option<(PathBuf, String)>;

struct Change {
    id: u32,
    before: File,
    after: File,
}

#[derive(Default)]
pub struct History {
    done: Vec<Change>,
    undone: Vec<Change>,
}

fn read(folder: &Folder, id: u32) -> File {
    let path = folder.path(id).ok()?;
    let text = fs::read_to_string(&path).ok()?;
    Some((path, text))
}

/// Puts back `to` in place of `from`, unless the file has changed since it was `from`.
fn swap(folder: &Folder, id: u32, from: &File, to: &File) -> Result<()> {
    if read(folder, id) != *from {
        bail!("#{id} changed since");
    }
    if let Some((path, _)) = from {
        fs::remove_file(path)?;
    }
    if let Some((path, text)) = to {
        fs::write(path, text)?;
    }
    Ok(())
}

impl History {
    /// Runs `change`, which makes or changes issue `id`, keeping it for undo. `id` is None for
    /// a new issue, which `change` returns.
    pub fn record(
        &mut self,
        folder: &Folder,
        id: Option<u32>,
        change: impl FnOnce() -> Result<u32>,
    ) -> Result<u32> {
        let before = id.and_then(|id| read(folder, id));
        let id = change()?;
        let after = read(folder, id);
        if before != after {
            self.done.push(Change { id, before, after });
            self.undone.clear();
        }
        Ok(id)
    }

    /// Undoes the last change; says which issue it was.
    pub fn undo(&mut self, folder: &Folder) -> Result<Option<u32>> {
        let Some(c) = self.done.pop() else {
            return Ok(None);
        };
        if let Err(e) = swap(folder, c.id, &c.after, &c.before) {
            self.done.push(c);
            return Err(e);
        }
        let id = c.id;
        self.undone.push(c);
        Ok(Some(id))
    }

    /// Redoes the last undone change; says which issue it was.
    pub fn redo(&mut self, folder: &Folder) -> Result<Option<u32>> {
        let Some(c) = self.undone.pop() else {
            return Ok(None);
        };
        if let Err(e) = swap(folder, c.id, &c.before, &c.after) {
            self.undone.push(c);
            return Err(e);
        }
        let id = c.id;
        self.done.push(c);
        Ok(Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue::Issue;

    fn open(title: &str) -> Issue {
        let mut i = Issue::new(0, title, None, "", 0);
        i.status = "open".into();
        i
    }

    fn set(folder: &Folder, id: u32, status: &str) -> Result<u32> {
        folder.update(id, |i| {
            i.status = status.into();
            Ok(id)
        })
    }

    #[test]
    fn undo_puts_back_the_exact_file_where_it_was_and_redo_repeats_it() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        let mut h = History::default();
        let id = h
            .record(&folder, None, || folder.create(open("Loops")))
            .unwrap();
        let open = read(&folder, id);
        h.record(&folder, Some(id), || set(&folder, id, "done"))
            .unwrap();
        assert!(folder.root.join("issues/done/1-loops.md").is_file());

        assert_eq!(h.undo(&folder).unwrap(), Some(id));
        assert_eq!(read(&folder, id), open);
        assert!(!folder.root.join("issues/done/1-loops.md").exists());
        h.undo(&folder).unwrap();
        assert!(folder.path(id).is_err());
        assert_eq!(h.undo(&folder).unwrap(), None);

        h.redo(&folder).unwrap();
        h.redo(&folder).unwrap();
        assert_eq!(folder.get(id).unwrap().status, "done");
        assert_eq!(h.redo(&folder).unwrap(), None);
    }

    #[test]
    fn undo_refuses_when_the_file_changed_since() {
        let dir = tempfile::tempdir().unwrap();
        let folder = Folder::init(dir.path()).unwrap();
        let id = folder.create(open("Loops")).unwrap();
        let mut h = History::default();
        h.record(&folder, Some(id), || set(&folder, id, "working"))
            .unwrap();
        folder
            .update(id, |i| {
                i.title = "Edited by an agent".into();
                Ok(())
            })
            .unwrap();
        assert!(h.undo(&folder).is_err());
        assert_eq!(folder.get(id).unwrap().title, "Edited by an agent");
    }
}
