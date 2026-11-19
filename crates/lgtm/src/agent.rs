//! What `lgtm review` asks of a review it loaded.

use std::path::{Path, PathBuf};

pub use lgtm_core::review::agent::{AddComment, CheckFiles, MarkLines};

#[derive(Debug)]
pub enum Request {
    GetReview,
    GetComments,
    CheckFiles(CheckFiles),
    MarkLines(MarkLines),
    AddComment(AddComment),
}

pub type Response = Result<String, String>;

/// The jj workspace `dir` is in: the nearest ancestor holding a `.jj` directory.
pub fn workspace(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|p| p.join(".jj").is_dir())
        .map(Path::to_path_buf)
}
