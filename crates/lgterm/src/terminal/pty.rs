//! A shell on a pseudo-terminal.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use lgterm_proto::{BIN_ENV, SOCKET_ENV, TERMINAL_ENV, TermId};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

/// What every terminal is told about the app it runs in.
#[derive(Clone, Debug, Default)]
pub struct SpawnEnv {
    /// The control socket.
    pub socket: Option<PathBuf>,
    /// The `lgterm` binary.
    pub bin: Option<PathBuf>,
}

/// Variables from a terminal lgterm may itself be running in, which would point its
/// programs at the wrong app.
const HIDDEN_ENV: &[&str] = &["HC_TERMINAL_ID"];

/// Reads from the shell held in the channel before the reader waits.
const OUTPUT_CHUNKS: usize = 16;

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    /// Input for the writer thread, in order.
    input: std::sync::mpsc::Sender<Vec<u8>>,
    /// Taken on drop, to be killed and reaped off the main thread.
    child: Option<Box<dyn Child + Send + Sync>>,
}

/// Output from the shell, ending when it exits.
pub type Output = async_channel::Receiver<Vec<u8>>;

impl Pty {
    /// Starts the user's login shell in `cwd`, running `argv` if it's not empty.
    pub fn spawn(
        term_id: TermId,
        cwd: &Path,
        argv: &[String],
        app: &SpawnEnv,
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
            // fish passes extra arguments as $argv; other shells that aren't POSIX run it
            // through zsh.
            let shell = std::env::var("SHELL").unwrap_or_default();
            let name = Path::new(&shell).file_name().and_then(|n| n.to_str());
            let fish = name == Some("fish");
            let posix = matches!(name, Some("sh" | "bash" | "zsh" | "dash" | "ksh"));
            let mut cmd = CommandBuilder::new(if fish || posix { &shell } else { "/bin/zsh" });
            if fish {
                cmd.args(["-l", "-c", "exec $argv"]);
            } else {
                cmd.args(["-lc", "exec \"$@\"", "lgterm"]);
            }
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
        cmd.env(TERMINAL_ENV, term_id.to_string());
        if let Some(socket) = &app.socket {
            cmd.env(SOCKET_ENV, socket);
        }
        if let Some(bin) = &app.bin {
            cmd.env(BIN_ENV, bin);
        }
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        let (input, inputs) = std::sync::mpsc::channel::<Vec<u8>>();
        std::thread::Builder::new()
            .name(format!("pty-{term_id}-in"))
            .spawn(move || {
                for bytes in inputs {
                    if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                        break;
                    }
                }
            })?;
        // Bounded, so a flood of output waits in the pty instead of piling up in memory.
        let (tx, rx) = async_channel::bounded(OUTPUT_CHUNKS);
        std::thread::Builder::new()
            .name(format!("pty-{term_id}"))
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
                input,
                child: Some(child),
            },
            rx,
        ))
    }

    pub fn write(&mut self, bytes: &[u8]) {
        let _ = self.input.send(bytes.to_vec());
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
            .or_else(|| self.shell_pid())
    }

    pub fn shell_pid(&self) -> Option<i32> {
        self.child.as_ref()?.process_id().map(|p| p as i32)
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            std::thread::spawn(move || {
                let _ = child.kill();
                let _ = child.wait();
            });
        }
    }
}
