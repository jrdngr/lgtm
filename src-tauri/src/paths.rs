//! Where repos and the app's data live on disk.

use std::path::PathBuf;

use tauri::{Manager, Wry};

pub use lgtm_core::paths::{find_root, key, main_root, repo_dir};

pub fn data_dir(app: &impl Manager<Wry>) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}
