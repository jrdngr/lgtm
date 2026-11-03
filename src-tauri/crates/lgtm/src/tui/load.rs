//! Reads from jj off the UI thread. Results come back as messages the event loop drains.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use lgtm_core::anchor::MatchConfig;
use lgtm_core::config::{Config, ConfigTexts};
use lgtm_core::diff::FileStatus;
use lgtm_core::jj::{Change, FileDiff, Jj, LogRow};
use lgtm_core::review::file::{ReviewFile, Sides as Commits, Text};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

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
    /// A jj operation landed: commits may have changed.
    Changed,
    /// Every file's review state, in diff order, for matching marks.
    Built {
        id: u64,
        files: Vec<ReviewFile>,
        cfg: MatchConfig,
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

    /// Watches the workspace at `root` for new operations, and for edits to snapshot while
    /// `snapshots` is set. Watches until the watcher is dropped.
    pub fn watch(
        &self,
        root: &std::path::Path,
        snapshots: Arc<AtomicBool>,
    ) -> anyhow::Result<notify::RecommendedWatcher> {
        let _entered = self.rt.enter();
        super::watch::start(root, self.jj.clone(), self.tx.clone(), snapshots)
    }

    pub fn log(&self) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let _ = tx.send(Msg::Log(jj.log_rows(None).await));
        });
    }

    /// Loads `revset`'s diff, then each file's highlighting, then every file's review state.
    pub fn review(&self, id: u64, revset: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let cached = ConfigTexts::default();
            let result = async {
                let range = jj.resolve(&revset).await?;
                let (changes, files, config) = tokio::try_join!(
                    jj.changes(&revset),
                    jj.diff(&range),
                    Config::at_base(&jj, &cached, range.from.as_deref()),
                )?;
                anyhow::Ok((range, changes, files, config))
            }
            .await;
            let (range, changes, files, config) = match result {
                Ok(loaded) => loaded,
                Err(e) => {
                    let _ = tx.send(Msg::Diff { id, result: Err(e) });
                    return;
                }
            };
            let _ = tx.send(Msg::Diff {
                id,
                result: Ok((changes, files.clone())),
            });
            let config = Arc::new(config);
            let commits = Arc::new(Commits {
                old: range.from.clone(),
                new: range.to.clone(),
            });
            let gate = Arc::new(Semaphore::new(READERS));
            let mut tasks = JoinSet::new();
            for (file, diff) in files.into_iter().enumerate() {
                let (jj, tx, gate) = (jj.clone(), tx.clone(), gate.clone());
                let (config, commits) = (config.clone(), commits.clone());
                tasks.spawn(async move {
                    let _permit = gate.acquire().await.ok();
                    let (old, new) = texts(&jj, &commits, &diff).await;
                    let built = tokio::task::spawn_blocking(move || {
                        let highlight = !diff.binary && !diff.hunks.is_empty();
                        let sides = highlight
                            .then(|| Sides::of(&diff.path, old.as_deref(), new.as_deref()));
                        fn text(t: &Option<String>) -> Text<'_> {
                            Text {
                                text: t.as_deref(),
                                parsed: None,
                            }
                        }
                        let skipped = config.skips(&diff.path);
                        let review_file = ReviewFile::build(
                            &commits,
                            &diff,
                            text(&old),
                            text(&new),
                            config.matching,
                            skipped,
                        );
                        (sides, review_file)
                    })
                    .await;
                    let (sides, review_file) = built.ok()?;
                    if let Some(sides) = sides {
                        let _ = tx.send(Msg::Highlight { id, file, sides });
                    }
                    Some((file, review_file))
                });
            }
            let mut built: Vec<Option<ReviewFile>> = Vec::new();
            while let Some(done) = tasks.join_next().await {
                let Ok(Some((file, review_file))) = done else {
                    return;
                };
                if built.len() <= file {
                    built.resize_with(file + 1, || None);
                }
                built[file] = Some(review_file);
            }
            let files: Option<Vec<ReviewFile>> = built.into_iter().collect();
            if let Some(files) = files {
                let _ = tx.send(Msg::Built {
                    id,
                    files,
                    cfg: config.matching,
                });
            }
        });
    }
}

/// A file's old and new text, `None` for a side it doesn't have.
async fn texts(jj: &Jj, commits: &Commits, diff: &FileDiff) -> (Option<String>, Option<String>) {
    if diff.binary {
        return (None, None);
    }
    let old_path = diff.old_path.as_deref().unwrap_or(&diff.path);
    let old = match diff.status {
        FileStatus::Added => None,
        _ => jj
            .file_if_any(commits.old.as_deref(), old_path)
            .await
            .ok()
            .flatten(),
    };
    let new = match diff.status {
        FileStatus::Removed => None,
        _ => jj
            .file_if_any(Some(&commits.new), &diff.path)
            .await
            .ok()
            .flatten(),
    };
    let text = |b: Option<Vec<u8>>| b.map(|b| String::from_utf8_lossy(&b).into_owned());
    (text(old), text(new))
}
