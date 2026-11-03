//! Reads from jj off the UI thread. Results come back as messages the event loop drains.

use std::sync::Arc;
use std::sync::mpsc::Sender;

use lgtm_core::diff::FileStatus;
use lgtm_core::jj::{Change, FileDiff, Jj, LogRow};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

use super::highlight::Sides;

pub enum Msg {
    Log(anyhow::Result<Vec<LogRow>>),
    /// A review's changes, oldest first, and its diff. `id` tells a stale load apart.
    Diff {
        id: u64,
        result: anyhow::Result<(Vec<Change>, Vec<FileDiff>)>,
    },
    Highlight {
        id: u64,
        file: usize,
        sides: Sides,
    },
}

/// How many files are read and highlighted at once.
const READERS: usize = 8;

pub struct Loader {
    rt: Runtime,
    jj: Arc<Jj>,
    tx: Sender<Msg>,
}

impl Loader {
    pub fn new(jj: Jj, tx: Sender<Msg>) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        Ok(Self {
            rt,
            jj: Arc::new(jj),
            tx,
        })
    }

    pub fn log(&self) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let _ = tx.send(Msg::Log(jj.log_rows(None).await));
        });
    }

    /// Loads `revset`'s diff, then each file's highlighting.
    pub fn review(&self, id: u64, revset: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = async {
                let range = jj.resolve(&revset).await?;
                let (changes, files) = tokio::try_join!(jj.changes(&revset), jj.diff(&range))?;
                anyhow::Ok((range, changes, files))
            }
            .await;
            let (range, changes, files) = match result {
                Ok(loaded) => loaded,
                Err(e) => {
                    let _ = tx.send(Msg::Diff { id, result: Err(e) });
                    return;
                }
            };
            let wanted: Vec<_> = files
                .iter()
                .enumerate()
                .filter(|(_, f)| !f.binary && !f.hunks.is_empty())
                .map(|(i, f)| (i, f.clone()))
                .collect();
            let _ = tx.send(Msg::Diff {
                id,
                result: Ok((changes, files)),
            });
            let gate = Arc::new(Semaphore::new(READERS));
            for (file, diff) in wanted {
                let (jj, tx, gate, range) = (jj.clone(), tx.clone(), gate.clone(), range.clone());
                tokio::spawn(async move {
                    let Ok(_permit) = gate.acquire().await else {
                        return;
                    };
                    let old_path = diff.old_path.as_deref().unwrap_or(&diff.path);
                    let old = match diff.status {
                        FileStatus::Added => None,
                        _ => jj
                            .file_if_any(range.from.as_deref(), old_path)
                            .await
                            .ok()
                            .flatten(),
                    };
                    let new = match diff.status {
                        FileStatus::Removed => None,
                        _ => jj
                            .file_if_any(Some(&range.to), &diff.path)
                            .await
                            .ok()
                            .flatten(),
                    };
                    let text =
                        |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned());
                    let (old, new) = (text(old), text(new));
                    let path = diff.path.clone();
                    let sides = tokio::task::spawn_blocking(move || {
                        Sides::of(&path, old.as_deref(), new.as_deref())
                    })
                    .await
                    .unwrap_or_default();
                    let _ = tx.send(Msg::Highlight { id, file, sides });
                });
            }
        });
    }
}
