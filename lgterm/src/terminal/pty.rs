//! A shell on a pseudo-terminal.

use std::io::{Read, Write};
use std::path::Path;

use anyhow::Result;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// The app's control socket, given to every terminal.
pub static SOCKET: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// Variables from a terminal lgterm may itself be running in, which would point its
/// programs at the wrong app.
const HIDDEN_ENV: &[&str] = &[
    "LGTM_SOCKET",
    "LGTM_ROOT",
    "LGTM_TERMINAL",
    "LGTM_APP_BIN",
    "LGTM_WORKSPACE",
    "HC_TERMINAL_ID",
];

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

/// Output from the shell, ending when it exits.
pub type Output = async_channel::Receiver<Vec<u8>>;

impl Pty {
    /// Starts the user's login shell in `cwd`, running `argv` if it's not empty.
    pub fn spawn(
        term_id: u64,
        cwd: &Path,
        argv: &[String],
        cols: u16,
        rows: u16,
    ) -> Result<(Self, Output)> {
        let pair = native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut cmd = if argv.is_empty() {
            CommandBuilder::new_default_prog()
        } else {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
            let mut cmd = CommandBuilder::new(shell);
            cmd.args(["-lc", "exec \"$@\"", "lgterm"]);
            cmd.args(argv);
            cmd
        };
        cmd.cwd(cwd);
        for var in HIDDEN_ENV {
            cmd.env_remove(var);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "lgterm");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        cmd.env("LGTERM_TERMINAL_ID", format!("t{term_id}"));
        if let Some(socket) = SOCKET.get() {
            cmd.env(crate::ipc::SOCKET_ENV, socket);
        }
        if let Ok(exe) = std::env::current_exe() {
            cmd.env("LGTERM_BIN", exe);
        }
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let writer = pair.master.take_writer()?;
        let (tx, rx) = async_channel::unbounded();
        std::thread::Builder::new()
            .name(format!("pty-t{term_id}"))
            .spawn(move || {
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send_blocking(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                    }
                }
            })?;
        Ok((
            Pty {
                master: pair.master,
                writer,
                child,
            },
            rx,
        ))
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if self.writer.write_all(bytes).is_ok() {
            let _ = self.writer.flush();
        }
    }

    pub fn resize(&self, cols: u16, rows: u16, width: u16, height: u16) {
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: width,
            pixel_height: height,
        });
    }

    /// The process in the foreground: the shell, or what it's running.
    pub fn foreground_pid(&self) -> Option<i32> {
        self.master
            .process_group_leader()
            .or_else(|| self.child.process_id().map(|p| p as i32))
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
