//! Layouts saved on quit and restored on launch.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::model::Model;
use crate::status::Kind;
use crate::ui::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedTerm {
    pub cwd: PathBuf,
    /// The agent running in it and its session, to resume.
    pub agent: Option<(Kind, String)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub model: Model,
    pub terms: BTreeMap<u64, SavedTerm>,
    pub messages_tab: Option<u64>,
    pub messages: Vec<Message>,
    pub collapsed: Vec<u64>,
    pub sidebar: bool,
}

fn path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("lgterm").join("state.json"))
}

impl State {
    pub fn load() -> Option<State> {
        let bytes = std::fs::read(path()?).ok()?;
        let state: State = serde_json::from_slice(&bytes).ok()?;
        state.model.is_valid().then_some(state)
    }

    pub fn to_json(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }

    pub fn write(json: &[u8]) {
        let Some(path) = path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}
