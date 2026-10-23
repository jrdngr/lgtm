//! Markdown notes at three levels: global ones every repo shows, the repo's, which each of its
//! workspaces shows, and a workspace's own. Each level has folders.

use std::sync::Mutex;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::ctx::now_ms;
use crate::store::{NoteRow, Store};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NoteLevel {
    Global,
    Repo,
    Workspace,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub id: String,
    pub level: NoteLevel,
    /// `/`-separated, empty at the top of the level.
    pub folder: String,
    pub name: String,
    pub body: String,
    pub created_at: f64,
    pub updated_at: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteFolderDto {
    pub level: NoteLevel,
    pub path: String,
}

/// What a workspace shows: every note of its three levels, and every folder, including the
/// ones holding a note.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotesDto {
    pub notes: Vec<NoteDto>,
    pub folders: Vec<NoteFolderDto>,
}

/// lgtm's own store, which holds the global notes.
pub struct GlobalStore(pub Mutex<Store>);

/// The notes one workspace of a repo sees.
pub struct NoteBook<'a> {
    pub global: &'a Mutex<Store>,
    pub repo: &'a Mutex<Store>,
    /// The workspace's name, empty for the main one.
    pub workspace: String,
}

/// `folder` without empty segments or surrounding space. Rejects `.` and `..`.
fn clean_folder(folder: &str) -> anyhow::Result<String> {
    let parts: Vec<&str> = folder
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.iter().any(|p| *p == "." || *p == "..") {
        bail!("`{folder}` isn't a folder name");
    }
    Ok(parts.join("/"))
}

fn clean_name(name: &str, what: &str) -> anyhow::Result<String> {
    let name = name.trim();
    if name.is_empty() {
        bail!("a {what} needs a name");
    }
    if name.contains('/') {
        bail!("a {what} name can't hold `/`");
    }
    Ok(name.to_string())
}

/// Whether `folder` is `path` or inside it.
fn within(folder: &str, path: &str) -> bool {
    folder == path || folder.starts_with(&format!("{path}/"))
}

/// `folder`, which is within `from`, moved to `to`.
fn rebase(folder: &str, from: &str, to: &str) -> String {
    format!("{to}{}", &folder[from.len()..])
}

fn dto(level: NoteLevel, n: NoteRow) -> NoteDto {
    NoteDto {
        id: n.id,
        level,
        folder: n.folder,
        name: n.name,
        body: n.body,
        created_at: n.created_at as f64,
        updated_at: n.updated_at as f64,
    }
}

impl NoteBook<'_> {
    /// The store and workspace column a level's notes live under.
    fn place(&self, level: NoteLevel) -> (&Mutex<Store>, Option<&str>) {
        match level {
            NoteLevel::Global => (self.global, None),
            NoteLevel::Repo => (self.repo, None),
            NoteLevel::Workspace => (self.repo, Some(self.workspace.as_str())),
        }
    }

    fn find(&self, id: &str) -> anyhow::Result<(NoteLevel, NoteRow)> {
        if let Some(n) = self.repo.lock().unwrap().note(id)? {
            let level = match n.workspace {
                Some(_) => NoteLevel::Workspace,
                None => NoteLevel::Repo,
            };
            return Ok((level, n));
        }
        let n = self.global.lock().unwrap().note(id)?;
        Ok((
            NoteLevel::Global,
            n.with_context(|| format!("no note `{id}`"))?,
        ))
    }

    fn put(&self, level: NoteLevel, n: &NoteRow) -> anyhow::Result<()> {
        self.place(level).0.lock().unwrap().put_note(n)?;
        Ok(())
    }

    /// Every note and folder this workspace shows, the last changed note first.
    pub fn list(&self) -> anyhow::Result<NotesDto> {
        let mut out = NotesDto::default();
        for level in [NoteLevel::Global, NoteLevel::Repo, NoteLevel::Workspace] {
            let (store, ws) = self.place(level);
            let store = store.lock().unwrap();
            let mut folders = store.note_folders(ws)?;
            for n in store.notes_of(ws)? {
                folders.push(n.folder.clone());
                out.notes.push(dto(level, n));
            }
            // A folder holding a note shows its parents too.
            let mut all: Vec<String> = Vec::new();
            for f in folders {
                let mut path = String::new();
                for part in f.split('/').filter(|p| !p.is_empty()) {
                    if !path.is_empty() {
                        path.push('/');
                    }
                    path.push_str(part);
                    all.push(path.clone());
                }
            }
            all.sort();
            all.dedup();
            out.folders
                .extend(all.into_iter().map(|path| NoteFolderDto { level, path }));
        }
        out.notes
            .sort_by(|a, b| b.updated_at.total_cmp(&a.updated_at));
        Ok(out)
    }

    pub fn get(&self, id: &str) -> anyhow::Result<NoteDto> {
        let (level, n) = self.find(id)?;
        Ok(dto(level, n))
    }

    pub fn create(
        &self,
        level: NoteLevel,
        folder: &str,
        name: &str,
        body: &str,
    ) -> anyhow::Result<NoteDto> {
        let now = now_ms();
        let mut id = format!("n{now}");
        while self.find(&id).is_ok() {
            id.push('x');
        }
        let n = NoteRow {
            id,
            workspace: self.place(level).1.map(str::to_string),
            folder: clean_folder(folder)?,
            name: clean_name(name, "note")?,
            body: body.to_string(),
            created_at: now,
            updated_at: now,
        };
        self.put(level, &n)?;
        Ok(dto(level, n))
    }

    pub fn save(&self, id: &str, body: &str) -> anyhow::Result<NoteDto> {
        let (level, mut n) = self.find(id)?;
        n.body = body.to_string();
        n.updated_at = now_ms();
        self.put(level, &n)?;
        Ok(dto(level, n))
    }

    pub fn rename(&self, id: &str, name: &str) -> anyhow::Result<NoteDto> {
        let (level, mut n) = self.find(id)?;
        n.name = clean_name(name, "note")?;
        self.put(level, &n)?;
        Ok(dto(level, n))
    }

    /// Moves note `id` into `folder` of `level`, keeping its id.
    pub fn move_to(&self, id: &str, level: NoteLevel, folder: &str) -> anyhow::Result<NoteDto> {
        let (from, mut n) = self.find(id)?;
        n.folder = clean_folder(folder)?;
        n.workspace = self.place(level).1.map(str::to_string);
        if !std::ptr::eq(self.place(from).0, self.place(level).0) {
            self.place(from).0.lock().unwrap().delete_note(id)?;
        }
        self.put(level, &n)?;
        Ok(dto(level, n))
    }

    pub fn delete(&self, id: &str) -> anyhow::Result<()> {
        let (level, _) = self.find(id)?;
        self.place(level).0.lock().unwrap().delete_note(id)?;
        Ok(())
    }

    /// Makes folder `name` in `parent` of `level`. Returns its path.
    pub fn add_folder(&self, level: NoteLevel, parent: &str, name: &str) -> anyhow::Result<String> {
        let name = clean_name(name, "folder")?;
        let parent = clean_folder(parent)?;
        let path = if parent.is_empty() {
            name
        } else {
            format!("{parent}/{name}")
        };
        let (store, ws) = self.place(level);
        store.lock().unwrap().add_note_folder(ws, &path)?;
        Ok(path)
    }

    /// Renames folder `path` of `level`, with what's in it.
    pub fn rename_folder(&self, level: NoteLevel, path: &str, name: &str) -> anyhow::Result<()> {
        let name = clean_name(name, "folder")?;
        let to = match path.rsplit_once('/') {
            Some((parent, _)) => format!("{parent}/{name}"),
            None => name,
        };
        self.refolder(level, path, |folder| Some(rebase(folder, path, &to)))
    }

    /// Deletes folder `path` of `level` with every note and folder in it.
    pub fn delete_folder(&self, level: NoteLevel, path: &str) -> anyhow::Result<()> {
        self.refolder(level, path, |_| None)
    }

    /// Moves each note and folder within `path` to the folder `to` gives, or deletes it.
    fn refolder(
        &self,
        level: NoteLevel,
        path: &str,
        to: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<()> {
        let (store, ws) = self.place(level);
        let store = store.lock().unwrap();
        for mut n in store.notes_of(ws)? {
            if !within(&n.folder, path) {
                continue;
            }
            match to(&n.folder) {
                Some(folder) => {
                    n.folder = folder;
                    store.put_note(&n)?;
                }
                None => store.delete_note(&n.id)?,
            }
        }
        for f in store.note_folders(ws)? {
            if !within(&f, path) {
                continue;
            }
            store.delete_note_folder(ws, &f)?;
            if let Some(moved) = to(&f) {
                store.add_note_folder(ws, &moved)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stores() -> (Mutex<Store>, Mutex<Store>) {
        (
            Mutex::new(Store::in_memory()),
            Mutex::new(Store::in_memory()),
        )
    }

    fn book<'a>(s: &'a (Mutex<Store>, Mutex<Store>), ws: &str) -> NoteBook<'a> {
        NoteBook {
            global: &s.0,
            repo: &s.1,
            workspace: ws.into(),
        }
    }

    #[test]
    fn each_workspace_sees_global_and_repo_notes_but_only_its_own_workspace_notes() {
        let s = stores();
        let a = book(&s, "a");
        a.create(NoteLevel::Global, "", "Everywhere", "").unwrap();
        a.create(NoteLevel::Repo, "", "Repo", "").unwrap();
        a.create(NoteLevel::Workspace, "", "Mine", "").unwrap();

        let seen = |ws| {
            book(&s, ws)
                .list()
                .unwrap()
                .notes
                .into_iter()
                .map(|n| n.name)
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert_eq!(seen("a"), ["Everywhere", "Mine", "Repo"]);
        assert_eq!(seen("b"), ["Everywhere", "Repo"]);
    }

    #[test]
    fn moving_a_note_between_levels_keeps_its_id_and_body() {
        let s = stores();
        let a = book(&s, "a");
        let n = a.create(NoteLevel::Workspace, "", "Idea", "text").unwrap();
        a.move_to(&n.id, NoteLevel::Global, "Later").unwrap();

        let moved = book(&s, "b").get(&n.id).unwrap();
        assert_eq!(
            (moved.level, moved.folder.as_str(), moved.body.as_str()),
            (NoteLevel::Global, "Later", "text")
        );
        assert!(
            s.1.lock().unwrap().note(&n.id).unwrap().is_none(),
            "the repo copy is gone"
        );
    }

    #[test]
    fn folders_list_with_their_parents_and_rename_and_delete_with_their_contents() {
        let s = stores();
        let a = book(&s, "");
        a.add_folder(NoteLevel::Repo, "", "Empty").unwrap();
        let n = a
            .create(NoteLevel::Repo, "Plans/Auth", "Tokens", "")
            .unwrap();
        let folders = |a: &NoteBook| {
            a.list()
                .unwrap()
                .folders
                .into_iter()
                .map(|f| f.path)
                .collect::<Vec<_>>()
        };
        assert_eq!(folders(&a), ["Empty", "Plans", "Plans/Auth"]);

        a.rename_folder(NoteLevel::Repo, "Plans", "Done").unwrap();
        assert_eq!(a.get(&n.id).unwrap().folder, "Done/Auth");

        a.delete_folder(NoteLevel::Repo, "Done").unwrap();
        assert!(a.get(&n.id).is_err());
        assert_eq!(folders(&a), ["Empty"]);
    }

    #[test]
    fn folder_paths_drop_empty_segments_and_refuse_dot_dirs() {
        assert_eq!(clean_folder(" /a//b/ ").unwrap(), "a/b");
        assert!(clean_folder("a/../b").is_err());
        assert!(clean_name("a/b", "note").is_err());
    }
}
