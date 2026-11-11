//! Reads from jj off the UI thread. Results come back as messages the event loop drains.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use lgtm_core::anchor::MatchConfig;
use lgtm_core::config::{Config, ConfigTexts};
use lgtm_core::jj::{Change, FileDiff, Jj, LogRow, Range, Version};
use lgtm_core::review::contents;
use lgtm_core::review::file::{ReviewFile, Sides as Commits, Text};
use tokio::runtime::Runtime;
use tokio::task::JoinSet;

use super::highlight::Sides;

/// What a review shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Revset(String),
    /// From one version of a change to a later one. `span` names the versions.
    Between {
        from: String,
        to: String,
        change: String,
        span: String,
    },
}

impl Target {
    /// What the review is called.
    pub fn name(&self) -> String {
        match self {
            Target::Revset(r) => r.clone(),
            Target::Between { change, span, .. } => format!("{change} {span}"),
        }
    }
}

pub enum Msg {
    Log(anyhow::Result<Vec<LogRow>>),
    /// The workspace's files.
    Files(super::walk::Listing),
    /// How the working copy changes files.
    Decor(super::decor::Decorations),
    /// What jj knows about the sides of the conflicts in file `name`, read as `text`.
    Resolve {
        name: String,
        text: String,
        session: Box<lgtm_core::resolve::Session>,
    },
    /// A file at `@-`, empty when it isn't there, for the editor's gutter.
    Base {
        name: String,
        text: String,
    },
    /// The versions of the change `rev`, newest first.
    Versions {
        rev: String,
        result: anyhow::Result<Vec<Version>>,
    },
    /// A review's changes, oldest first, and its diff. `id` tells a stale load apart.
    Diff {
        id: u64,
        result: anyhow::Result<(Vec<Change>, Vec<FileDiff>)>,
    },
    /// The bookmarks and workspaces nearest above review `id`'s newest commit.
    Containing {
        id: u64,
        found: lgtm_core::jj::Containing,
    },
    Highlight {
        id: u64,
        file: usize,
        sides: Sides,
    },
    /// A jj operation landed: commits may have changed.
    Changed,
    /// Files were written, for the trees' heat.
    Written(Vec<String>),
    /// An agent's request over MCP, and where its answer goes.
    Agent {
        req: crate::agent::Request,
        reply: std::sync::mpsc::Sender<crate::agent::Response>,
    },
    /// The review's PR threads, pulled from GitHub.
    Pulled {
        id: u64,
        result: anyhow::Result<super::github::Pulled>,
    },
    /// Every file's review state, in diff order, for matching marks.
    Built {
        id: u64,
        files: Vec<ReviewFile>,
        cfg: MatchConfig,
    },
    /// A jj menu motion ran.
    Acted(anyhow::Result<u32>),
    /// The bookmarks nearest a commit, for Set bookmark.
    Bookmarks(anyhow::Result<Vec<String>>),
    /// What a push would do.
    PushPlan(anyhow::Result<lgtm_core::jj::PushPlan>),
}

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

    /// Runs a jj menu motion.
    pub fn act(&self, action: lgtm_core::jj::JjAction) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let _ = tx.send(Msg::Acted(jj.act(&action).await));
        });
    }

    /// Finds what a push of `revs`' bookmarks would do.
    pub fn push_plan(&self, revs: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let _ = tx.send(Msg::PushPlan(jj.push_plan(Some(&revs)).await));
        });
    }

    /// Lists the bookmarks nearest `rev`.
    pub fn bookmarks(&self, rev: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let found = jj.nearest_bookmarks(&rev).await;
            let _ = tx.send(Msg::Bookmarks(
                found.map(|spots| spots.into_iter().map(|s| s.name).collect()),
            ));
        });
    }

    /// Reads the file `name` at `@-`.
    pub fn base(&self, name: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            // Without a readable base the gutter stays empty.
            if let Ok(bytes) = jj.file_if_any(Some("@-"), &name).await {
                let text = String::from_utf8_lossy(&bytes.unwrap_or_default()).into_owned();
                let _ = tx.send(Msg::Base { name, text });
            }
        });
    }

    /// Fills in the sides of the conflicts in `text`, the file `name`'s, from jj.
    pub fn resolve(&self, name: String, text: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let mut session = lgtm_core::resolve::Session::new(&name, &text);
            lgtm_core::resolve::fill(&mut session, &jj).await;
            let session = Box::new(session);
            let _ = tx.send(Msg::Resolve {
                name,
                text,
                session,
            });
        });
    }

    /// Snapshots the working copy, then finds how `@` changes files, for the file trees. A
    /// snapshot that changed something lands as an operation, which reloads this again.
    pub fn decor(&self) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            use super::decor::{Decor, Decorations};
            let _ = jj.background_snapshot().await;
            // Without a diff the trees keep their colors.
            let Ok(range) = jj.resolve("@").await else {
                return;
            };
            let Ok(paths) = jj.diff_paths(&range).await else {
                return;
            };
            let found = paths
                .into_iter()
                .filter_map(|p| Some((p.path, Decor::of(p.status, p.conflict)?)));
            let _ = tx.send(Msg::Decor(Decorations::new(found)));
        });
    }

    /// The files with conflicts in the working copy, snapshotted first.
    pub fn conflicted(&self) -> anyhow::Result<Vec<String>> {
        self.rt.block_on(async {
            let _ = self.jj.snapshot().await;
            self.jj.conflicted_files("@").await
        })
    }

    /// Lists the files under `root`.
    pub fn files(&self, root: &std::path::Path) {
        let (root, tx) = (root.to_path_buf(), self.tx.clone());
        self.rt.spawn_blocking(move || {
            let _ = tx.send(Msg::Files(super::walk::list(&root)));
        });
    }

    /// Pulls the threads of the PR `pr`, or else of the open PR from one of `bookmarks`.
    pub fn pull(&self, id: u64, pr: Option<String>, bookmarks: Vec<String>) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = super::github::pull(&jj, pr.as_deref(), &bookmarks).await;
            let _ = tx.send(Msg::Pulled { id, result });
        });
    }

    /// Loads the versions of the change `rev`.
    pub fn versions(&self, rev: String) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let result = jj.versions(&rev).await;
            let _ = tx.send(Msg::Versions { rev, result });
        });
    }

    /// Loads `target`'s diff, then each file's highlighting, then every file's review state.
    pub fn review(&self, id: u64, target: Target) {
        let (jj, tx) = (self.jj.clone(), self.tx.clone());
        self.rt.spawn(async move {
            let cached = ConfigTexts::default();
            let result = async {
                let (range, revset) = match target {
                    Target::Revset(revset) => {
                        // Files the editor saved since the last snapshot show too.
                        let _ = jj.background_snapshot().await;
                        (jj.resolve(&revset).await?, revset)
                    }
                    Target::Between { from, to, .. } => (
                        Range {
                            from: Some(from),
                            to: to.clone(),
                        },
                        to,
                    ),
                };
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
            if let Ok(found) = jj.containing(&range.to).await {
                let _ = tx.send(Msg::Containing { id, found });
            }
            let config = Arc::new(config);
            let commits = Arc::new(Commits {
                old: range.from.clone(),
                new: range.to.clone(),
            });
            let diffs: Vec<&FileDiff> = files.iter().collect();
            let texts = contents::read(&jj, &commits, &diffs)
                .await
                .unwrap_or_else(|_| vec![(None, None); files.len()]);
            let mut tasks = JoinSet::new();
            for (file, (diff, (old, new))) in files.into_iter().zip(texts).enumerate() {
                let tx = tx.clone();
                let (config, commits) = (config.clone(), commits.clone());
                tasks.spawn(async move {
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
