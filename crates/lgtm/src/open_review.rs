//! Which review each open lgtm shows, so `lgtm review` can default to it. Each app keeps a
//! file named for its pid in its workspace's folder, locked while it runs.

use std::fs::{File, TryLockError};
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A review an app shows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shown {
    /// The review's name: its revset, or a change's versions.
    pub revset: String,
    /// For a change's versions, the commits the review runs between.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<(String, String)>,
    /// The PR whose threads it shows, as `(owner/name, number)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<(String, u32)>,
}

#[derive(Serialize, Deserialize)]
struct Record {
    shown: Shown,
    /// When it started showing, in ms.
    at: u128,
}

/// The folder of the workspace at `root`'s records.
pub fn folder(root: &Path) -> Option<PathBuf> {
    let data = lgtm_core::paths::data_dir()?;
    Some(data.join("tui/open").join(lgtm_core::paths::key(root)))
}

/// This app's record. Removed when dropped.
pub struct Mine {
    path: PathBuf,
    file: File,
    shown: Option<Shown>,
}

impl Mine {
    /// Starts this app's record in `folder`, showing nothing yet.
    pub fn start(folder: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(folder)?;
        let path = folder.join(format!("{}.json", std::process::id()));
        let file = File::create(&path)?;
        file.lock()?;
        Ok(Self {
            path,
            file,
            shown: None,
        })
    }

    /// Records that the app shows `shown` now, when that's news.
    pub fn show(&mut self, shown: Option<Shown>) {
        if shown == self.shown {
            return;
        }
        let text = match &shown {
            Some(s) => serde_json::to_string(&Record {
                shown: s.clone(),
                at: now_ms(),
            })
            .unwrap_or_default(),
            None => String::new(),
        };
        let written = self.file.set_len(0).and_then(|()| {
            self.file.rewind()?;
            self.file.write_all(text.as_bytes())
        });
        if written.is_ok() {
            self.shown = shown;
        }
    }
}

impl Drop for Mine {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The review the open app in `folder` that most lately changed its review shows. Removes
/// records whose app is gone.
pub fn latest(folder: &Path) -> Option<Shown> {
    let entries = std::fs::read_dir(folder).ok()?;
    let mut best: Option<Record> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file) = File::open(&path) else {
            continue;
        };
        match file.try_lock_shared() {
            // Nobody holds it, so its app is gone.
            Ok(()) => {
                let _ = std::fs::remove_file(&path);
                continue;
            }
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(_)) => continue,
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(record) = serde_json::from_str::<Record>(&text) else {
            continue;
        };
        if best.as_ref().is_none_or(|b| record.at > b.at) {
            best = Some(record);
        }
    }
    best.map(|r| r.shown)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(revset: &str) -> Option<Shown> {
        Some(Shown {
            revset: revset.into(),
            between: None,
            pr: None,
        })
    }

    #[test]
    fn the_latest_open_review_wins_and_closed_apps_drop_out() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(latest(dir.path()), None);
        let mut first = Mine::start(&dir.path().join("a")).unwrap();
        let folder = dir.path().join("a");
        first.show(shown("@-"));
        assert_eq!(latest(&folder), shown("@-"));

        // A second app in the same folder, as another process would be.
        let path = folder.join("2.json");
        let file = File::create(&path).unwrap();
        file.lock().unwrap();
        let mut second = Mine {
            path,
            file,
            shown: None,
        };
        std::thread::sleep(std::time::Duration::from_millis(2));
        second.show(shown("main"));
        assert_eq!(latest(&folder), shown("main"));

        second.show(None);
        assert_eq!(latest(&folder), shown("@-"));
        drop(second);
        drop(first);
        assert_eq!(latest(&folder), None);
    }

    #[test]
    fn a_record_left_by_an_app_that_died_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir.path().join("99.json");
        let record = Record {
            shown: shown("@").unwrap(),
            at: 1,
        };
        std::fs::write(&stale, serde_json::to_string(&record).unwrap()).unwrap();
        assert_eq!(latest(dir.path()), None);
        assert!(!stale.exists());
    }
}
