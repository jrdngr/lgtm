//! The app's control socket: one JSON request and one JSON reply per connection.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

/// A request waiting for the app's reply.
pub struct Incoming {
    pub request: Request,
    pub reply: async_channel::Sender<Reply>,
}

pub const SOCKET_ENV: &str = "LGTERM_SOCKET";

/// A socket path for this app instance.
pub fn socket_path() -> PathBuf {
    let dir = std::env::temp_dir();
    dir.join(format!("lgterm-{}.sock", std::process::id()))
}

pub struct Server {
    path: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Listens on `path`, handing each request to the receiver.
pub fn serve(path: &Path) -> Result<(Server, async_channel::Receiver<Incoming>)> {
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path).with_context(|| format!("binding {path:?}"))?;
    let (tx, rx) = async_channel::unbounded();
    std::thread::Builder::new()
        .name("lgterm-socket".into())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = tx.clone();
                let _ = std::thread::Builder::new()
                    .name("lgterm-conn".into())
                    .spawn(move || handle(stream, tx));
            }
        })?;
    Ok((
        Server {
            path: path.to_path_buf(),
        },
        rx,
    ))
}

fn handle(stream: UnixStream, tx: async_channel::Sender<Incoming>) {
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    let reply = match serde_json::from_str::<Request>(&line) {
        Ok(request) => {
            let (reply, answer) = async_channel::bounded(1);
            if tx.send_blocking(Incoming { request, reply }).is_err() {
                return;
            }
            watch_hangup(&stream, answer.clone());
            match answer.recv_blocking() {
                Ok(r) => r,
                Err(_) => Reply::Err("lgterm closed".into()),
            }
        }
        Err(e) => Reply::Err(format!("bad request: {e}")),
    };
    let mut stream = stream;
    if let Ok(mut out) = serde_json::to_vec(&reply) {
        out.push(b'\n');
        let _ = stream.write_all(&out);
    }
    let _ = stream.shutdown(Shutdown::Both);
}

/// Closes `answer` once the client hangs up, so a reply nobody will read isn't waited for.
fn watch_hangup(stream: &UnixStream, answer: async_channel::Receiver<Reply>) {
    let Ok(mut peer) = stream.try_clone() else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("lgterm-hangup".into())
        .spawn(move || {
            let mut buf = [0u8; 64];
            while matches!(peer.read(&mut buf), Ok(n) if n > 0) {}
            answer.close();
        });
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

/// `t5` or `5` as a terminal id.
pub fn parse_term(s: &str) -> Option<u64> {
    s.strip_prefix('t').unwrap_or(s).parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip_through_the_socket() {
        let dir = std::env::temp_dir().join(format!("lgterm-ipc-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        let (_server, rx) = serve(&path).unwrap();
        std::thread::spawn(move || {
            let inc = rx.recv_blocking().unwrap();
            let Request::Focus { term } = inc.request else {
                panic!("wrong request")
            };
            inc.reply.send_blocking(Reply::Ok(term)).unwrap();
        });
        let reply = call(&path, &Request::Focus { term: "t3".into() }, None).unwrap();
        assert_eq!(reply, Reply::Ok("t3".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hung_up_client_closes_its_pending_reply() {
        let dir = std::env::temp_dir().join(format!("lgterm-hangup-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.sock");
        let (_server, rx) = serve(&path).unwrap();
        let wait = Request::Wait { term: "t1".into() };
        let err = call(&path, &wait, Some(Duration::from_millis(50))).unwrap_err();
        assert!(err.downcast_ref::<std::io::Error>().is_some());
        let inc = rx.recv_blocking().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !inc.reply.is_closed() {
            assert!(std::time::Instant::now() < deadline, "reply never closed");
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn terms_parse_with_or_without_the_t() {
        assert_eq!(parse_term("t12"), Some(12));
        assert_eq!(parse_term("7"), Some(7));
        assert_eq!(parse_term("x"), None);
    }
}
