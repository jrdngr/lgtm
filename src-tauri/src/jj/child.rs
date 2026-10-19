//! Runs jj commands that take locks. Each logs its lock steps to stderr
//! (`JJ_LOG=jj_lib::lock=info`), which tells a jj waiting on a lock, safe to end because it
//! hasn't written anything, from one that holds a lock and may be writing, which is never
//! ended early. Running ones are listed so quitting can end them.

use std::collections::HashMap;
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::Context;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::process::{Child, Command};

use super::holders;

/// How long a background command may wait on a lock before it's ended.
#[cfg(not(test))]
pub(super) const BACKGROUND_WAIT: Duration = Duration::from_secs(15);
#[cfg(test)]
pub(super) const BACKGROUND_WAIT: Duration = Duration::from_secs(1);
/// How long a foreground command waits on a lock before what holds it is reported.
#[cfg(not(test))]
const REPORT_AFTER: Duration = Duration::from_secs(10);
#[cfg(test)]
const REPORT_AFTER: Duration = Duration::from_secs(1);
/// How long an ended jj gets to clean up after SIGTERM before SIGKILL.
const END_GRACE: Duration = Duration::from_secs(2);
const TICK: Duration = Duration::from_millis(200);

/// A background command ended while it waited on `lock`.
#[derive(Debug)]
pub struct LockWait {
    pub lock: String,
}

impl std::fmt::Display for LockWait {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "gave up waiting on {}", self.lock)
    }
}

impl std::error::Error for LockWait {}

pub(super) enum Mode<'a> {
    /// Ended once it has waited on a lock for `BACKGROUND_WAIT` while holding none.
    Background,
    /// Never ended early. Once it has waited on a lock for `REPORT_AFTER`, gets the processes
    /// that have the lock open, and `None` once it stops waiting.
    Foreground(&'a (dyn Fn(Option<String>) + Send + Sync)),
}

pub(super) struct Output {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    /// Without the lock log lines.
    pub stderr: String,
}

/// What jj's lock log says it's doing.
#[derive(Debug, Default)]
struct Locks {
    /// The lock it's waiting on, and since when.
    waiting: Option<(String, Instant)>,
    /// Whether it has taken a lock. From then on it may write.
    held: bool,
}

impl Locks {
    /// Takes in a stderr line. Returns false when it isn't a lock log line.
    fn note(&mut self, line: &str, now: Instant) -> bool {
        let line = strip_ansi(line);
        if let Some(path) = quoted_after(&line, "Attempting to lock ") {
            self.waiting = Some((path, now));
        } else if quoted_after(&line, "Locked ").is_some() {
            self.waiting = None;
            self.held = true;
        } else {
            return false;
        }
        true
    }

    /// The lock it's waiting on while it holds none, and since when.
    fn blocked(&self) -> Option<(String, Instant)> {
        self.waiting.clone().filter(|_| !self.held)
    }
}

/// Removes terminal color codes, which jj's log has even when piped.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // `ESC [` then parameters, ended by a letter.
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// The double-quoted path after `marker`, which jj writes with Rust's debug escapes.
fn quoted_after(line: &str, marker: &str) -> Option<String> {
    let rest = &line[line.find(marker)? + marker.len()..];
    let inner = rest.strip_prefix('"')?.strip_suffix('"')?;
    Some(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// A running jj.
struct Live {
    child: Mutex<Child>,
    locks: Mutex<Locks>,
}

impl Live {
    /// Waits for it to exit. Polls rather than awaiting `Child::wait`, so the child stays
    /// reachable for signals until it's reaped.
    async fn wait(&self) -> std::io::Result<ExitStatus> {
        let mut pause = Duration::from_millis(2);
        loop {
            if let Some(status) = self.child.lock().unwrap().try_wait()? {
                return Ok(status);
            }
            tokio::time::sleep(pause).await;
            pause = (pause * 2).min(Duration::from_millis(50));
        }
    }

    /// Sends `signal` unless it has exited. A child that hasn't been reaped keeps its PID, so
    /// the signal can't reach another process.
    fn signal(&self, signal: rustix::process::Signal) {
        let mut child = self.child.lock().unwrap();
        if !matches!(child.try_wait(), Ok(None)) {
            return;
        }
        let pid = child
            .id()
            .and_then(|p| rustix::process::Pid::from_raw(p as i32));
        if let Some(pid) = pid {
            rustix::process::kill_process(pid, signal).ok();
        }
    }

    fn running(&self) -> bool {
        matches!(self.child.lock().unwrap().try_wait(), Ok(None))
    }

    /// Asks it to stop, which lets jj clean up, then kills it if it hasn't.
    async fn end(&self) {
        self.signal(rustix::process::Signal::TERM);
        if tokio::time::timeout(END_GRACE, self.wait()).await.is_err() {
            self.signal(rustix::process::Signal::KILL);
            self.wait().await.ok();
        }
    }
}

static NEXT: AtomicU64 = AtomicU64::new(0);
static LIVE: LazyLock<Mutex<HashMap<u64, Arc<Live>>>> = LazyLock::new(Mutex::default);

/// Lists a child while it runs. Dropped before it exits, as when the command's future is
/// dropped, it kills a child that holds no lock and lets one that does finish.
struct Listed {
    id: u64,
    live: Arc<Live>,
}

impl Listed {
    fn new(child: Child) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let live = Arc::new(Live {
            child: Mutex::new(child),
            locks: Mutex::default(),
        });
        LIVE.lock().unwrap().insert(id, live.clone());
        Self { id, live }
    }
}

impl Drop for Listed {
    fn drop(&mut self) {
        if self.live.running() {
            if !self.live.locks.lock().unwrap().held {
                self.live.signal(rustix::process::Signal::KILL);
            } else if let Ok(rt) = tokio::runtime::Handle::try_current() {
                // Listed until it exits, so a quit still ends it.
                let (id, live) = (self.id, self.live.clone());
                rt.spawn(async move {
                    live.wait().await.ok();
                    LIVE.lock().unwrap().remove(&id);
                });
                return;
            }
        }
        LIVE.lock().unwrap().remove(&self.id);
    }
}

/// PIDs of the app's own jj processes that are waiting on `lock`, so not holding it.
pub(super) fn own_waiters(lock: &str) -> Vec<u32> {
    let live: Vec<Arc<Live>> = LIVE.lock().unwrap().values().cloned().collect();
    live.iter()
        .filter(|l| {
            let locks = l.locks.lock().unwrap();
            locks.waiting.as_ref().is_some_and(|(p, _)| p == lock)
        })
        .filter_map(|l| l.child.lock().unwrap().id())
        .collect()
}

/// Ends every jj the app started, for when the app quits: SIGTERM, then SIGKILL for any still
/// running a second later.
pub fn end_all() {
    let live: Vec<Arc<Live>> = LIVE.lock().unwrap().values().cloned().collect();
    for l in &live {
        l.signal(rustix::process::Signal::TERM);
    }
    let until = Instant::now() + Duration::from_secs(1);
    while Instant::now() < until && live.iter().any(|l| l.running()) {
        std::thread::sleep(Duration::from_millis(20));
    }
    for l in &live {
        l.signal(rustix::process::Signal::KILL);
    }
}

async fn read_all(mut r: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    r.read_to_end(&mut buf).await?;
    Ok(buf)
}

/// Reads stderr, noting lock log lines and keeping the rest.
async fn read_stderr(r: impl AsyncRead + Unpin, live: &Live) -> std::io::Result<String> {
    let mut r = BufReader::new(r);
    let (mut line, mut kept) = (Vec::new(), String::new());
    while r.read_until(b'\n', &mut line).await? > 0 {
        let text = String::from_utf8_lossy(&line);
        if !live
            .locks
            .lock()
            .unwrap()
            .note(text.trim_end(), Instant::now())
        {
            kept.push_str(&text);
        }
        line.clear();
    }
    Ok(kept)
}

pub(super) async fn run(mut cmd: Command, mode: Mode<'_>) -> anyhow::Result<Output> {
    cmd.env("JJ_LOG", "jj_lib::lock=info")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .context("run jj (is it installed and on PATH?)")?;
    let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
    let listed = Listed::new(child);
    let live = &*listed.live;
    let work = async {
        tokio::try_join!(
            read_all(stdout.context("no stdout")?),
            read_stderr(stderr.context("no stderr")?, live),
            live.wait(),
        )
        .map_err(anyhow::Error::from)
    };
    tokio::pin!(work);
    let mut tick = tokio::time::interval(TICK);
    let mut reported = false;
    let done = loop {
        tokio::select! {
            done = &mut work => break done,
            _ = tick.tick() => match mode {
                Mode::Background => {
                    let blocked = live.locks.lock().unwrap().blocked();
                    if let Some((lock, since)) = blocked
                        && since.elapsed() >= BACKGROUND_WAIT
                    {
                        live.end().await;
                        return Err(LockWait { lock }.into());
                    }
                }
                Mode::Foreground(report) => {
                    let waiting = live.locks.lock().unwrap().waiting.clone();
                    match waiting {
                        Some((lock, since)) if !reported && since.elapsed() >= REPORT_AFTER => {
                            reported = true;
                            report(Some(holders::describe(&lock).await));
                        }
                        None if reported => {
                            reported = false;
                            report(None);
                        }
                        _ => {}
                    }
                }
            },
        }
    };
    if let (true, Mode::Foreground(report)) = (reported, &mode) {
        report(None);
    }
    let (stdout, stderr, status) = done?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jj::Notice;
    use crate::jj::test_support::{git_lock, hold, openers, repo, write};

    #[tokio::test]
    async fn a_blocked_write_reports_what_has_the_lock_and_still_finishes() {
        let (dir, j) = repo();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        j.set_notify(Arc::new(move |n| sink.lock().unwrap().push(n)));
        write(dir.path(), "a.txt", "a\n");
        let lock = git_lock(dir.path());
        let held = hold(&lock);
        let j = Arc::new(j);
        let write = tokio::spawn({
            let j = j.clone();
            async move { j.snapshot().await }
        });

        let reported = async {
            loop {
                if let Some(Notice::Blocked(Some(holders))) = seen.lock().unwrap().first() {
                    return holders.clone();
                }
                tokio::time::sleep(TICK).await;
            }
        };
        let holders = tokio::time::timeout(REPORT_AFTER * 5, reported)
            .await
            .unwrap();
        // The test holds the lock. The waiting jj, the app's own, isn't listed.
        let lines: Vec<&str> = holders.lines().collect();
        assert_eq!(lines.len(), 1, "{holders}");
        assert!(lines[0].starts_with(&format!("{} ", std::process::id())));
        let jj_pid = openers(&lock)
            .into_iter()
            .find(|p| *p != std::process::id());
        assert!(jj_pid.is_some());

        tokio::time::sleep(REPORT_AFTER).await;
        assert!(!write.is_finished());
        drop(held);
        write.await.unwrap().unwrap();
        assert_eq!(seen.lock().unwrap()[1..], [Notice::Blocked(None)]);
    }

    #[test]
    fn reads_jjs_lock_log() {
        let now = Instant::now();
        let mut locks = Locks::default();
        let attempt = "\x1b[2m2026-10-19T19:36:27Z\x1b[0m \x1b[32m INFO\x1b[0m \
            \x1b[2mjj_lib::lock::unix\x1b[0m\x1b[2m:\x1b[0m Attempting to lock \"/r/.jj/repo/a \\\"b\\\".lock\"";
        assert!(locks.note(attempt, now));
        assert_eq!(
            locks.blocked(),
            Some(("/r/.jj/repo/a \"b\".lock".into(), now))
        );
        assert!(locks.note(
            " INFO jj_lib::lock::unix: Locked \"/r/.jj/repo/a.lock\"",
            now
        ));
        assert_eq!(locks.blocked(), None);
        // Waiting on a second lock while holding the first isn't safe to end.
        assert!(locks.note(
            " INFO jj_lib::lock::unix: Attempting to lock \"/r/wc.lock\"",
            now
        ));
        assert!(locks.waiting.is_some());
        assert_eq!(locks.blocked(), None);
        assert!(!locks.note("Error: Refusing to undo operation", now));
    }
}
