//! The workspace's socket, where `lgtm mcp` asks this app for agents. The first app open in
//! a workspace answers; later ones run without it.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::time::Duration;

use super::load::Msg;
use crate::agent::{Request, Response};

/// Listens until dropped, then removes the socket.
pub struct Server {
    path: PathBuf,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Starts listening at `path`, unless another app already listens there: that one keeps
/// answering agents.
pub fn start(path: &Path, tx: Sender<Msg>) -> anyhow::Result<Option<Server>> {
    if UnixStream::connect(path).is_ok() {
        return Ok(None);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let _ = std::fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || answer(stream, &tx));
        }
    });
    Ok(Some(Server {
        path: path.to_path_buf(),
    }))
}

/// Reads one request, has the UI answer it, and writes the answer back.
fn answer(stream: UnixStream, tx: &Sender<Msg>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut line = String::new();
    if BufReader::new(&stream).read_line(&mut line).is_err() {
        return;
    }
    let response: Response = match serde_json::from_str::<Request>(&line) {
        Ok(req) => {
            let (reply, answer) = mpsc::channel();
            if tx.send(Msg::Agent { req, reply }).is_err() {
                return;
            }
            answer
                .recv_timeout(Duration::from_secs(25))
                .unwrap_or_else(|_| Err("lgtm didn't answer in time".into()))
        }
        Err(e) => Err(format!("unknown request: {e}")),
    };
    if let Ok(mut text) = serde_json::to_string(&response) {
        text.push('\n');
        let _ = (&stream).write_all(text.as_bytes());
    }
}
