//! The notes tools: the markdown notes the human keeps in lgtm, globally, for the repo and for
//! this terminal's workspace.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars::{self, JsonSchema};
use rmcp::{tool, tool_router};
use serde::{Deserialize, Serialize};
use tauri::Manager;
use tauri_specta::Event as _;

use super::server::{Lgtm, Out, failed, json};
use crate::events::NotesChanged;
use crate::notes::{GlobalStore, NoteBook, NoteLevel};

#[derive(Deserialize, JsonSchema)]
pub struct NoteId {
    /// From list_notes.
    pub id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct NoteWrite {
    /// The note to replace the text of. Leave it out to make a new note.
    pub id: Option<String>,
    /// A new note's level: `global` (every repo), `repo` (every workspace of this repo) or
    /// `workspace` (this terminal's workspace only). `workspace` by default.
    pub level: Option<String>,
    /// A new note's folder, `/`-separated. The top of the level by default.
    pub folder: Option<String>,
    /// A new note's name.
    pub name: Option<String>,
    /// The note's whole markdown text.
    pub body: String,
}

#[derive(Serialize)]
struct NoteItem {
    id: String,
    level: NoteLevel,
    folder: String,
    name: String,
}

fn level(name: Option<&str>) -> Result<NoteLevel, String> {
    match name.unwrap_or("workspace") {
        "global" => Ok(NoteLevel::Global),
        "repo" => Ok(NoteLevel::Repo),
        "workspace" => Ok(NoteLevel::Workspace),
        other => Err(format!(
            "`{other}` isn't a level: use global, repo or workspace"
        )),
    }
}

impl Lgtm {
    /// Runs `f` on the notes this terminal's workspace sees.
    fn notes<T>(&self, f: impl FnOnce(&NoteBook) -> anyhow::Result<T>) -> Result<T, String> {
        let (_, ctx) = self.repo().map_err(failed)?;
        let global = self.app().state::<GlobalStore>();
        let book = NoteBook {
            global: &global.0,
            repo: &ctx.store,
            workspace: self.workspace().unwrap_or_default().to_string(),
        };
        f(&book).map_err(failed)
    }
}

#[tool_router(router = notes_router, vis = "pub(super)")]
impl Lgtm {
    #[tool(
        description = "The human's markdown notes in lgtm that this workspace shows: global, \
        the repo's and this workspace's, with their folders."
    )]
    async fn list_notes(&self) -> Out {
        let all = self.notes(|b| b.list())?;
        json(
            &all.notes
                .into_iter()
                .map(|n| NoteItem {
                    id: n.id,
                    level: n.level,
                    folder: n.folder,
                    name: n.name,
                })
                .collect::<Vec<_>>(),
        )
    }

    #[tool(description = "A note's markdown text.")]
    async fn read_note(&self, Parameters(p): Parameters<NoteId>) -> Out {
        Ok(self.notes(|b| b.get(&p.id))?.body)
    }

    #[tool(
        description = "Write a note in lgtm: replace a note's whole text, or with no id make a \
        new note. Only when the human asks for it."
    )]
    async fn write_note(&self, Parameters(p): Parameters<NoteWrite>) -> Out {
        let n = match &p.id {
            Some(id) => self.notes(|b| b.save(id, &p.body))?,
            None => {
                let level = level(p.level.as_deref())?;
                let name = p.name.as_deref().ok_or("a new note needs a name")?;
                let folder = p.folder.as_deref().unwrap_or_default();
                self.notes(|b| b.create(level, folder, name, &p.body))?
            }
        };
        NotesChanged {
            id: Some(n.id.clone()),
        }
        .emit(self.app())
        .ok();
        Ok(format!("Wrote note `{}` with id `{}`.", n.name, n.id))
    }
}
