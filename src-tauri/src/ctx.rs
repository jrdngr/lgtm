//! The repo's service context: its workspaces, store and open reviews, which every feature
//! extends with its own `impl RepoCtx` block.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::canvas::CanvasFocus;
use crate::jj::Jj;
use crate::review::Reviews;
use crate::store::Store;
use crate::watch;
use crate::workspace::Workspace;

/// Reports a workspace's watch events, with the workspace's name.
pub type Emit = Arc<dyn Fn(Option<String>, watch::Event) + Send + Sync>;

pub struct RepoCtx {
    /// The main workspace's root.
    pub root: PathBuf,
    pub main: Arc<Workspace>,
    /// The other workspaces, by name.
    pub(crate) workspaces: Mutex<HashMap<String, Arc<Workspace>>>,
    /// The other workspaces the UI has opened, which are watched.
    pub(crate) opened: Mutex<HashSet<String>>,
    /// Set once the window exists. Workspaces start being watched from then on.
    pub emit: Mutex<Option<Emit>>,
    /// Where new workspaces go.
    pub workspace_base: Option<PathBuf>,
    pub store: Mutex<Store>,
    pub(crate) reviews: Reviews,
    /// The canvas each workspace shows, by workspace name ("" for the main one).
    pub(crate) canvas_focus: Mutex<HashMap<String, CanvasFocus>>,
    /// Agent turns' snapshots from before their tools ran.
    pub(crate) befores: crate::turns::Befores,
}

impl RepoCtx {
    pub fn new(root: PathBuf, jj: Jj, store: Store) -> Self {
        Self {
            main: Arc::new(Workspace::new(None, root.clone(), jj)),
            root,
            workspaces: Mutex::default(),
            opened: Mutex::default(),
            emit: Mutex::default(),
            workspace_base: ws_base(),
            store: Mutex::new(store),
            reviews: Reviews::default(),
            canvas_focus: Mutex::default(),
            befores: Default::default(),
        }
    }
}

/// Where lgtm puts new workspaces: `$XDG_DATA_HOME/lgtm/ws`, else `~/.local/share/lgtm/ws`.
pub fn ws_base() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".local/share")))?;
    Some(data.join("lgtm/ws"))
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
