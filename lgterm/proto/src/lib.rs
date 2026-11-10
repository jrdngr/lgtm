//! What lgterm and the programs in its terminals say to each other: the variables each
//! terminal gets, and the control socket's requests and replies, one JSON line each way per
//! connection.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The app's control socket.
pub const SOCKET_ENV: &str = "LGTERM_SOCKET";
/// The terminal a program runs in, as `t5`.
pub const TERMINAL_ENV: &str = "LGTERM_TERMINAL_ID";
/// The `lgterm` binary.
pub const BIN_ENV: &str = "LGTERM_BIN";

/// A terminal, written `t5`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TermId(pub u64);

impl TermId {
    /// `t5` or `5`.
    pub fn parse(s: &str) -> Option<TermId> {
        s.strip_prefix('t').unwrap_or(s).parse().ok().map(TermId)
    }
}

impl fmt::Display for TermId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// Where a new terminal goes, relative to the one it's opened from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Tab,
    Right,
    Down,
    SpaceRight,
    SpaceDown,
    Layout,
}

/// Terminals travel as the text the user typed, so the app can say which one doesn't exist.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    /// A hook payload from an agent in `term`.
    Hook {
        term: String,
        event: Value,
    },
    Ls {
        json: bool,
    },
    Open {
        from: Option<String>,
        place: Place,
        cwd: Option<PathBuf>,
        argv: Vec<String>,
        focus: bool,
    },
    /// A message for the agent in `to`, queued until it's ready.
    Send {
        from: Option<String>,
        to: String,
        text: String,
    },
    /// Raw input for `to`.
    Type {
        to: String,
        text: String,
        enter: bool,
    },
    /// Replies once the agent in `term` is done, blocked or gone.
    Wait {
        term: String,
    },
    Read {
        term: String,
        lines: Option<usize>,
    },
    Focus {
        term: String,
    },
    Close {
        term: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reply {
    Ok(String),
    Err(String),
}

/// The terminal and socket of the lgterm this process runs in.
pub fn here() -> Option<(TermId, PathBuf)> {
    let var = |name| std::env::var_os(name).filter(|v| !v.is_empty());
    let term = TermId::parse(var(TERMINAL_ENV)?.to_str()?)?;
    Some((term, PathBuf::from(var(SOCKET_ENV)?)))
}

/// Sends a request to the app at `path` and waits up to `timeout` for the reply.
pub fn call(path: &Path, request: &Request, timeout: Option<Duration>) -> Result<Reply> {
    let mut stream = UnixStream::connect(path).context("lgterm isn't running")?;
    stream.set_read_timeout(timeout)?;
    let mut out = serde_json::to_vec(request)?;
    out.push(b'\n');
    stream.write_all(&out)?;
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line)?;
    Ok(serde_json::from_str(&line)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_parse_with_or_without_the_t() {
        assert_eq!(TermId::parse("t12"), Some(TermId(12)));
        assert_eq!(TermId::parse("7"), Some(TermId(7)));
        assert_eq!(TermId::parse("x"), None);
        assert_eq!(TermId(3).to_string(), "t3");
    }

    #[test]
    fn requests_are_tagged_by_command() {
        let hook = Request::Hook {
            term: "t1".into(),
            event: serde_json::json!({"hook_event_name": "Stop"}),
        };
        let json = serde_json::to_value(&hook).unwrap();
        assert_eq!(json["cmd"], "hook");
        assert_eq!(serde_json::from_value::<Request>(json).unwrap(), hook);
    }
}
