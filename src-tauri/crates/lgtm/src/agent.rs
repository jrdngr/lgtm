//! What agents ask the terminal app over its workspace's socket. `lgtm mcp` sends one request
//! per connection as a line of JSON and reads one line back.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CheckFiles {
    /// The review's revset, as get_review gives it. lgtm refuses the call once its review
    /// shows another.
    pub revset: String,
    /// The files' paths, as get_review gives them.
    pub paths: Vec<String>,
    /// False unchecks them.
    pub checked: bool,
}

/// An agent's mark: reviewed rows count as done, flagged ones don't.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AgentMark {
    Reviewed,
    Flagged,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MarkLines {
    /// The review's revset, as get_review gives it. lgtm refuses the call once its review
    /// shows another.
    pub revset: String,
    pub path: String,
    /// New-side line numbers or ranges, like `12` or `12-20`.
    #[serde(default)]
    pub lines: Vec<String>,
    /// Old-side line numbers or ranges of removed lines.
    #[serde(default)]
    pub removed_lines: Vec<String>,
    /// The mark to set. Leave it out to clear your marks on those lines.
    pub mark: Option<AgentMark>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct ResolveComment {
    /// The comment's number: 3 for C3.
    pub number: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
pub enum Request {
    GetReview,
    GetComments,
    CheckFiles(CheckFiles),
    MarkLines(MarkLines),
    ResolveComment(ResolveComment),
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

/// Line numbers from `12` and `12-20` style specs.
pub fn parse_lines(specs: &[String]) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for spec in specs {
        let bad = || format!("`{spec}` isn't a line or a range like 12-20");
        let (a, b) = match spec.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (spec.trim(), spec.trim()),
        };
        let (a, b): (u32, u32) = (a.parse().map_err(|_| bad())?, b.parse().map_err(|_| bad())?);
        if a == 0 || b < a {
            return Err(bad());
        }
        out.extend(a..=b);
    }
    Ok(out)
}

/// `lines`, sorted, as ranges like `3`, `5-9`.
pub fn ranges(mut lines: Vec<u32>) -> Vec<String> {
    lines.sort_unstable();
    lines.dedup();
    let mut out: Vec<String> = Vec::new();
    let mut start = None;
    for (i, &l) in lines.iter().enumerate() {
        let s = *start.get_or_insert(l);
        if lines.get(i + 1) != Some(&(l + 1)) {
            out.push(if s == l {
                s.to_string()
            } else {
                format!("{s}-{l}")
            });
            start = None;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_specs_and_ranges_round_trip() {
        let specs = ["3".to_string(), "5-7".into(), "9".into()];
        let lines = parse_lines(&specs).unwrap();
        assert_eq!(lines, [3, 5, 6, 7, 9]);
        assert_eq!(ranges(lines), ["3", "5-7", "9"]);
        assert!(parse_lines(&["7-5".into()]).is_err());
        assert!(parse_lines(&["0".into()]).is_err());
        assert!(parse_lines(&["x".into()]).is_err());
    }

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
