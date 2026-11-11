//! What agents ask the terminal app over its workspace's socket. `lgtm mcp` sends one request
//! per connection as a line of JSON and reads one line back.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use lgtm_core::review::agent::{AddComment, CheckFiles, MarkLines, ResolveComment};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum Request {
    GetReview,
    GetComments,
    CheckFiles(CheckFiles),
    MarkLines(MarkLines),
    ResolveComment(ResolveComment),
    UnresolveComment(ResolveComment),
    AddComment(AddComment),
}

pub type Response = Result<String, String>;

/// The jj workspace `dir` is in: the nearest ancestor holding a `.jj` directory.
pub fn workspace(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|p| p.join(".jj").is_dir())
        .map(Path::to_path_buf)
}

/// Where the terminal app open in the workspace at `root` listens.
pub fn socket_path(root: &Path) -> Option<PathBuf> {
    let dir = lgtm_core::paths::data_dir()?.join("tui");
    Some(dir.join(format!("{}.sock", lgtm_core::paths::key(root))))
}

/// Sends `req` to the terminal app listening at `socket`.
pub fn ask(socket: &Path, req: &Request) -> Response {
    let stream = UnixStream::connect(socket)
        .map_err(|_| "lgtm isn't open in this workspace: run `lgtm` there.".to_string())?;
    let talk = || -> std::io::Result<String> {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        let mut line = serde_json::to_string(req)?;
        line.push('\n');
        (&stream).write_all(line.as_bytes())?;
        let mut reply = String::new();
        BufReader::new(&stream).read_line(&mut reply)?;
        Ok(reply)
    };
    let reply = talk().map_err(|e| format!("lgtm didn't answer: {e}"))?;
    serde_json::from_str::<Response>(&reply)
        .map_err(|e| format!("lgtm's answer didn't parse: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_name_their_tool() {
        let req = Request::ResolveComment(ResolveComment { number: 3 });
        let text = serde_json::to_string(&req).unwrap();
        assert_eq!(text, r#"{"tool":"resolve_comment","number":3}"#);
        assert!(matches!(
            serde_json::from_str::<Request>(r#"{"tool":"get_review"}"#).unwrap(),
            Request::GetReview
        ));
    }
}
