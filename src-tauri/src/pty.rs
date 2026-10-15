use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde::Serialize;
use specta::Type;

#[derive(Clone, Serialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PtyEvent {
    Data { data: String },
    Exit { code: u32 },
}

/// Which window and workspace a PTY belongs to. `workspace` is `None` for the main workspace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Owner {
    pub window: String,
    pub workspace: Option<String>,
}

struct Session {
    owner: Owner,
    master: Box<dyn MasterPty + Send>,
    /// Feeds a writer thread, so a PTY that isn't reading input never blocks a caller.
    input: Sender<Vec<u8>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    /// The child's process id, which leads its process group.
    pid: Option<u32>,
}

#[derive(Default)]
pub struct Ptys {
    next_id: AtomicU32,
    sessions: Arc<Mutex<HashMap<u32, Session>>>,
}

impl Ptys {
    /// Starts `command` (or the login shell) in a new PTY. `on_event` runs on background
    /// threads and gets output followed by one `Exit`.
    pub fn spawn(
        &self,
        owner: Owner,
        cwd: &Path,
        size: (u16, u16),
        command: Option<Vec<String>>,
        env: &[(&str, &str)],
        on_event: impl Fn(PtyEvent) + Send + 'static,
    ) -> anyhow::Result<u32> {
        let (cols, rows) = size;
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open pty")?;

        let mut cmd = match command {
            Some(argv) => {
                let mut cmd = CommandBuilder::new(argv.first().context("empty command")?);
                cmd.args(&argv[1..]);
                cmd
            }
            None => {
                let mut cmd = CommandBuilder::new(default_shell());
                cmd.arg("-l");
                cmd
            }
        };
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = pair.slave.spawn_command(cmd).context("spawn")?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader()?;
        let input = spawn_writer(pair.master.take_writer()?);
        let killer = child.clone_killer();
        let pid = child.process_id();

        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.sessions.lock().unwrap().insert(
            id,
            Session {
                owner,
                master: pair.master,
                input,
                killer,
                pid,
            },
        );

        let sessions = Arc::clone(&self.sessions);
        // Output and the exit go through here. The exit takes it, so no output follows it.
        let on_event = Arc::new(Mutex::new(Some(on_event)));
        let emit = Arc::clone(&on_event);
        let (drained_tx, drained) = channel::<()>();
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 64 * 1024];
            let mut carry = Vec::new();
            // Linux reports EIO instead of EOF once the child exits.
            while let Ok(n @ 1..) = reader.read(&mut buf) {
                let data = decode_utf8(&mut carry, &buf[..n]);
                match emit.lock().unwrap().as_ref() {
                    Some(on) => on(PtyEvent::Data { data }),
                    None => break,
                }
            }
            drained_tx.send(()).ok();
        });
        // Reads only end once every process holding the terminal closes it, and a background
        // job can outlive the child. So the exit comes from the child, after a moment for the
        // rest of its output.
        std::thread::spawn(move || {
            let code = child.wait().map(|s| s.exit_code()).unwrap_or(1);
            drained.recv_timeout(Duration::from_millis(500)).ok();
            sessions.lock().unwrap().remove(&id);
            if let Some(on) = on_event.lock().unwrap().take() {
                on(PtyEvent::Exit { code });
            }
        });
        Ok(id)
    }

    pub fn write(&self, id: u32, data: &[u8]) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(&id).context("no such pty")?;
        session.input.send(data.to_vec()).ok().context("pty closed")
    }

    pub fn resize(&self, id: u32, cols: u16, rows: u16) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().unwrap();
        let session = sessions.get(&id).context("no such pty")?;
        session.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    /// The name of the program in the foreground of PTY `id`, when it isn't the PTY's own
    /// child, as when the shell runs a command.
    pub fn running(&self, id: u32) -> Option<String> {
        let (pid, leader) = {
            let sessions = self.sessions.lock().unwrap();
            let session = sessions.get(&id)?;
            (session.pid?, session.master.process_group_leader()?)
        };
        if leader as u32 == pid {
            return None;
        }
        let out = std::process::Command::new("ps")
            .args(["-o", "comm=", "-p", &leader.to_string()])
            .output()
            .ok()?;
        let comm = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let name = comm.rsplit('/').next().unwrap_or_default();
        Some(if name.is_empty() { "a program".into() } else { name.to_string() })
    }

    /// The reader thread removes the session once the child is gone.
    pub fn kill(&self, id: u32) {
        if let Some(session) = self.sessions.lock().unwrap().get_mut(&id) {
            session.killer.kill().ok();
        }
    }

    pub fn kill_window(&self, window: &str) {
        self.kill_where(|o| o.window == window);
    }

    pub fn kill_owner(&self, owner: &Owner) {
        self.kill_where(|o| o == owner);
    }

    fn kill_where(&self, pick: impl Fn(&Owner) -> bool) {
        for session in self.sessions.lock().unwrap().values_mut() {
            if pick(&session.owner) {
                session.killer.kill().ok();
            }
        }
    }
}

/// Writes queued input in order. The thread ends when the session is dropped or a write fails.
fn spawn_writer(mut writer: Box<dyn Write + Send>) -> Sender<Vec<u8>> {
    let (tx, rx) = channel::<Vec<u8>>();
    std::thread::spawn(move || {
        while let Ok(data) = rx.recv() {
            if writer.write_all(&data).is_err() {
                break;
            }
        }
    });
    tx
}

pub fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into())
}

/// Decodes a chunk of PTY output. A multi-byte character split across reads is held in
/// `carry` until the rest arrives. Invalid bytes become U+FFFD.
fn decode_utf8(carry: &mut Vec<u8>, chunk: &[u8]) -> String {
    carry.extend_from_slice(chunk);
    let mut out = String::with_capacity(carry.len());
    let mut rest: &[u8] = carry;
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                rest = &[];
                break;
            }
            Err(e) => {
                let (valid, after) = rest.split_at(e.valid_up_to());
                out.push_str(std::str::from_utf8(valid).unwrap());
                match e.error_len() {
                    Some(len) => {
                        out.push(char::REPLACEMENT_CHARACTER);
                        rest = &after[len..];
                    }
                    None => {
                        rest = after;
                        break;
                    }
                }
            }
        }
    }
    *carry = rest.to_vec();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn decode_holds_split_character_until_complete() {
        let mut carry = Vec::new();
        let bytes = "a→b".as_bytes(); // → is 3 bytes
        assert_eq!(decode_utf8(&mut carry, &bytes[..2]), "a");
        assert_eq!(decode_utf8(&mut carry, &bytes[2..3]), "");
        assert_eq!(decode_utf8(&mut carry, &bytes[3..]), "→b");
        assert!(carry.is_empty());
    }

    #[test]
    fn decode_replaces_invalid_bytes_and_continues() {
        let mut carry = Vec::new();
        assert_eq!(decode_utf8(&mut carry, b"a\xffb"), "a\u{FFFD}b");
        assert!(carry.is_empty());
    }

    fn owner(window: &str) -> Owner {
        Owner {
            window: window.into(),
            workspace: None,
        }
    }

    fn run(ptys: &Ptys, argv: &[&str]) -> (String, u32) {
        let (tx, rx) = mpsc::channel();
        let argv = argv.iter().map(|s| s.to_string()).collect();
        ptys.spawn(
            owner("w"),
            Path::new("/"),
            (80, 24),
            Some(argv),
            &[],
            move |e| tx.send(e).unwrap(),
        )
        .unwrap();
        let mut out = String::new();
        loop {
            match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
                PtyEvent::Data { data } => out.push_str(&data),
                PtyEvent::Exit { code } => return (out, code),
            }
        }
    }

    #[test]
    fn spawn_streams_output_then_exit_code_and_drops_session() {
        let ptys = Ptys::default();
        let (out, code) = run(&ptys, &["sh", "-c", "printf hello; exit 3"]);
        assert_eq!(out, "hello");
        assert_eq!(code, 3);
        assert!(ptys.sessions.lock().unwrap().is_empty());
    }

    #[test]
    fn exit_arrives_while_a_background_job_holds_the_terminal() {
        let ptys = Ptys::default();
        let (_, code) = run(&ptys, &["sh", "-c", "sleep 20 & exit 0"]);
        assert_eq!(code, 0);
    }

    #[test]
    fn running_names_the_foreground_program_but_not_the_shell() {
        let ptys = Ptys::default();
        let id = ptys
            .spawn(owner("w"), Path::new("/"), (80, 24), Some(vec!["sh".into()]), &[], |_| {})
            .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(ptys.running(id), None);
        ptys.write(id, b"sleep 5\n").unwrap();
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(ptys.running(id).as_deref(), Some("sleep"));
        ptys.kill(id);
    }

    #[test]
    fn write_reaches_child_stdin() {
        let ptys = Ptys::default();
        let (tx, rx) = mpsc::channel();
        let argv = ["sh", "-c", "read x; printf \"got:$x\""]
            .map(String::from)
            .to_vec();
        let id = ptys
            .spawn(
                owner("w"),
                Path::new("/"),
                (80, 24),
                Some(argv),
                &[],
                move |e| tx.send(e).unwrap(),
            )
            .unwrap();
        ptys.write(id, b"ping\n").unwrap();
        let mut out = String::new();
        while let PtyEvent::Data { data } = rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            out.push_str(&data);
        }
        assert!(out.ends_with("got:ping"), "{out:?}");
    }

    #[test]
    fn write_to_a_pty_not_reading_never_blocks() {
        let ptys = Ptys::default();
        let (tx, rx) = mpsc::channel();
        let argv = ["sleep", "30"].map(String::from).to_vec();
        let id = ptys
            .spawn(
                owner("w"),
                Path::new("/"),
                (80, 24),
                Some(argv),
                &[],
                move |e| {
                    if let PtyEvent::Exit { .. } = e {
                        tx.send(()).unwrap();
                    }
                },
            )
            .unwrap();
        ptys.write(id, &vec![b'x'; 1 << 20]).unwrap();
        ptys.write(id, b"more").unwrap();
        ptys.kill_window("w");
        rx.recv_timeout(Duration::from_secs(10)).unwrap();
    }

    #[test]
    fn kill_window_ends_only_that_windows_ptys() {
        let ptys = Ptys::default();
        let (tx, rx) = mpsc::channel();
        for window in ["a", "b"] {
            let tx = tx.clone();
            let argv = ["sleep", "30"].map(String::from).to_vec();
            ptys.spawn(
                owner(window),
                Path::new("/"),
                (80, 24),
                Some(argv),
                &[],
                move |e| {
                    if let PtyEvent::Exit { .. } = e {
                        tx.send(window).unwrap();
                    }
                },
            )
            .unwrap();
        }
        ptys.kill_window("a");
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), "a");
        assert!(rx.recv_timeout(Duration::from_millis(300)).is_err());
        ptys.kill_window("b");
    }
}
