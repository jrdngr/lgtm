//! Snapshots the watcher takes on its own. They give way to other jj processes: one that
//! waits on a lock too long is ended and retried later, and after two in a row they pause
//! until the user acts in the workspace.

use std::time::Duration;

use super::child::{self, LockWait, Mode};
use super::holders;
use super::{Jj, Notice, is_stale};

/// Timeouts since the last resume that pause background snapshots.
const PAUSE_AFTER: u32 = 2;
const RETRY_FIRST: Duration = Duration::from_secs(5);
const RETRY_MAX: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
pub enum Snapshot {
    /// Taken. Holds jj's message when the working copy is stale.
    Done(Option<String>),
    /// Gave up waiting on a lock. Try again after this long.
    Retry(Duration),
    /// Not taken: background snapshots are paused.
    Paused,
}

#[derive(Debug, Default)]
pub(super) struct Strikes {
    /// Timeouts in a row, which set how long to wait before the next try.
    count: u32,
    /// `count` at the last resume.
    resumed_at: u32,
    paused: bool,
}

/// Doubles from `RETRY_FIRST` with each timeout in a row, up to `RETRY_MAX`.
fn retry_after(count: u32) -> Duration {
    RETRY_FIRST
        .saturating_mul(1 << count.saturating_sub(1).min(16))
        .min(RETRY_MAX)
}

impl Jj {
    pub async fn background_snapshot(&self) -> anyhow::Result<Snapshot> {
        if self.background.lock().unwrap().paused {
            return Ok(Snapshot::Paused);
        }
        let _guard = self.writer.lock().await;
        let out = child::run(
            self.command(&["util", "snapshot"], false, "never"),
            Mode::Background,
        )
        .await
        .and_then(|out| self.check(&["util", "snapshot"], out));
        let lock_wait = match out {
            Ok(_) => None,
            Err(e) => match e.downcast::<LockWait>() {
                Ok(wait) => Some(wait),
                Err(e) if is_stale(&e) => {
                    *self.background.lock().unwrap() = Strikes::default();
                    return Ok(Snapshot::Done(Some(format!("{e:#}"))));
                }
                Err(e) => return Err(e),
            },
        };
        let Some(wait) = lock_wait else {
            *self.background.lock().unwrap() = Strikes::default();
            return Ok(Snapshot::Done(None));
        };
        let held_by = holders::describe(&wait.lock).await;
        eprintln!(
            "lgtm: background snapshot in {}: {wait}{}",
            self.root.display(),
            if held_by.is_empty() {
                String::new()
            } else {
                format!(", open in:\n{held_by}")
            }
        );
        let mut strikes = self.background.lock().unwrap();
        strikes.count += 1;
        if strikes.count - strikes.resumed_at < PAUSE_AFTER {
            return Ok(Snapshot::Retry(retry_after(strikes.count)));
        }
        strikes.paused = true;
        drop(strikes);
        self.notify(Notice::Paused(true));
        Ok(Snapshot::Paused)
    }

    /// The user acted in the workspace, so background snapshots start again.
    pub fn resume(&self) {
        let mut strikes = self.background.lock().unwrap();
        strikes.resumed_at = strikes.count;
        if std::mem::take(&mut strikes.paused) {
            drop(strikes);
            self.notify(Notice::Paused(false));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    use super::*;
    use crate::jj::test_support::{git_lock, hold, openers, repo, write};

    fn notices(j: &Jj) -> Arc<Mutex<Vec<Notice>>> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        j.set_notify(Arc::new(move |n| sink.lock().unwrap().push(n)));
        seen
    }

    #[tokio::test]
    async fn a_snapshot_blocked_on_a_lock_is_ended_then_pauses_until_resumed() {
        let (dir, j) = repo();
        let seen = notices(&j);
        write(dir.path(), "a.txt", "a\n");
        let lock = git_lock(dir.path());
        let held = hold(&lock);

        let started = Instant::now();
        assert_eq!(
            j.background_snapshot().await.unwrap(),
            Snapshot::Retry(RETRY_FIRST)
        );
        assert!(started.elapsed() < child::BACKGROUND_WAIT * 4);
        // The waiting jj is gone, and so is its hold on the writer.
        assert_eq!(openers(&lock), [std::process::id()]);
        assert!(j.writer.try_lock().is_ok());

        assert_eq!(j.background_snapshot().await.unwrap(), Snapshot::Paused);
        assert_eq!(*seen.lock().unwrap(), [Notice::Paused(true)]);
        // Paused, it doesn't run jj at all.
        let started = Instant::now();
        assert_eq!(j.background_snapshot().await.unwrap(), Snapshot::Paused);
        assert!(started.elapsed() < child::BACKGROUND_WAIT);

        drop(held);
        j.resume();
        assert_eq!(
            *seen.lock().unwrap(),
            [Notice::Paused(true), Notice::Paused(false)]
        );
        assert_eq!(j.background_snapshot().await.unwrap(), Snapshot::Done(None));
        let at = j.commit_ids("@").await.unwrap();
        assert_eq!(j.file(Some(&at[0]), "a.txt").await.unwrap(), b"a\n");
    }

    #[tokio::test]
    async fn a_snapshot_that_holds_a_lock_is_never_ended() {
        let (dir, j) = repo();
        write(dir.path(), "a.txt", "a\n");
        // It takes the repo-wide lock, then waits on the working copy's.
        let held = hold(&dir.path().join(".jj/working_copy/working_copy.lock"));
        let j = Arc::new(j);
        let snap = tokio::spawn({
            let j = j.clone();
            async move { j.background_snapshot().await.unwrap() }
        });
        tokio::time::sleep(child::BACKGROUND_WAIT * 3).await;
        assert!(!snap.is_finished());
        drop(held);
        assert_eq!(snap.await.unwrap(), Snapshot::Done(None));
    }

    #[test]
    fn retries_back_off_up_to_a_minute() {
        let waits: Vec<u64> = (1..=6).map(|n| retry_after(n).as_secs()).collect();
        assert_eq!(waits, [5, 10, 20, 40, 60, 60]);
        assert_eq!(retry_after(u32::MAX), RETRY_MAX);
    }
}
