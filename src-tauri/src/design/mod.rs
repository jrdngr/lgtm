//! Designs: working out a design with an agent. Each design is a folder in the repo
//! (`docs/design/<slug>/` by default) holding the brief, the design, the question queue, and
//! the docs, diagrams and HTML made from them. What isn't worth committing (the agent's
//! session, comments on the docs, what the user last saw) stays in the store.

pub mod agent;
pub mod queue;

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::review::RepoCtx;
use crate::workspace::Workspace;
use queue::Queue;

pub const BRIEF: &str = "brief.md";
pub const DESIGN: &str = "design.md";
pub const QUESTIONS: &str = "questions.json";

/// Held while a design's files are read, changed and written back.
static FILES: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CommentKind {
    /// Asks the agent to change the doc.
    Change,
    /// A question about the passage; nothing changes.
    Ask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CommentState {
    Queued,
    Applied,
    Answered,
}

/// A comment on a passage of one of the design's markdown files.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DocComment {
    pub id: u32,
    /// Relative to the design's folder.
    pub path: String,
    /// The selected text, so the comment still finds its place after edits.
    pub quote: String,
    /// First and last source lines of the selection, from 1.
    pub start: u32,
    pub end: u32,
    pub body: String,
    pub kind: CommentKind,
    pub state: CommentState,
    /// The agent's answer to an ask, or what it did for a change.
    pub reply: Option<String>,
}

/// The markdown an HTML page was made from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HtmlSource {
    pub source: String,
    pub hash: String,
}

/// What lgtm keeps about a design outside its folder.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Local {
    /// The agent's conversation, resumed by each step.
    pub session: Option<String>,
    pub comments: Vec<DocComment>,
    pub next_comment: u32,
    /// Each file's text when the user last looked at it.
    pub seen: BTreeMap<String, String>,
    /// HTML pages by path, and the markdown each was made from.
    pub html: BTreeMap<String, HtmlSource>,
    /// Paths the agent must read before drafting, relative to the workspace root.
    pub attach: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HtmlFile {
    pub path: String,
    /// The markdown it was made from, when lgtm made it from one.
    pub source: Option<String>,
    /// The markdown changed since.
    pub stale: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignDto {
    pub slug: String,
    pub name: String,
    /// The folder, relative to the workspace root.
    pub dir: String,
    pub brief: String,
    pub has_design: bool,
    pub queue: Queue,
    /// Why `questions.json` couldn't be read, when it couldn't.
    pub queue_error: Option<String>,
    /// Markdown files, relative to the folder: the design, then the docs.
    pub docs: Vec<String>,
    /// Standalone Mermaid files.
    pub diagrams: Vec<String>,
    pub html: Vec<HtmlFile>,
    pub comments: Vec<DocComment>,
    pub attach: Vec<String>,
}

/// Where designs go in a repo, relative to its root: the repo's config, else the setting.
pub fn base(root: &Path) -> String {
    crate::config::Config::load(root)
        .ok()
        .and_then(|c| c.design_dir)
        .unwrap_or_else(|| crate::settings::get().design_dir)
        .trim_matches('/')
        .to_string()
}

/// A folder name for a design called `name`.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

/// `rel` inside `dir`, refusing paths that would leave it.
pub fn inside(dir: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(rel);
    anyhow::ensure!(
        !rel.is_empty() && path.components().all(|c| matches!(c, Component::Normal(_))),
        "{rel} isn't a path inside the design"
    );
    Ok(dir.join(path))
}

/// The design's name: the brief's first heading, else its folder's name.
fn name(slug: &str, brief: &str) -> String {
    brief
        .lines()
        .find_map(|l| l.strip_prefix("# "))
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| slug.replace('-', " "))
}

/// A short hash of `text`, to tell when a page's markdown changed.
pub fn hash(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn read_queue(dir: &Path) -> anyhow::Result<Queue> {
    match std::fs::read_to_string(dir.join(QUESTIONS)) {
        Ok(text) => serde_json::from_str(&text).with_context(|| format!("can't read {QUESTIONS}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Queue::default()),
        Err(e) => Err(e).with_context(|| format!("can't read {QUESTIONS}")),
    }
}

fn write_queue(dir: &Path, queue: &Queue) -> anyhow::Result<()> {
    let text = serde_json::to_string_pretty(queue)? + "\n";
    std::fs::write(dir.join(QUESTIONS), text).with_context(|| format!("can't write {QUESTIONS}"))
}

/// Changes the queue on disk. Answers and agent updates both go through here.
pub fn edit_queue<T>(
    dir: &Path,
    f: impl FnOnce(&mut Queue) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let _held = FILES.lock().unwrap();
    let mut queue = read_queue(dir)?;
    let out = f(&mut queue)?;
    write_queue(dir, &queue)?;
    Ok(out)
}

/// Files under `dir/sub` with extension `ext`, relative to `dir`, sorted.
fn files(dir: &Path, sub: &str, ext: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir.join(sub)) else {
        return vec![];
    };
    let mut out: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(ext))
        .map(|n| {
            if sub.is_empty() {
                n
            } else {
                format!("{sub}/{n}")
            }
        })
        .collect();
    out.sort();
    out
}

fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// Reads a design's folder.
fn dto(dir: &Path, rel: &str, slug: &str, local: Local) -> DesignDto {
    let brief = read_text(&dir.join(BRIEF));
    let (queue, queue_error) = match read_queue(dir) {
        Ok(q) => (q, None),
        Err(e) => (Queue::default(), Some(format!("{e:#}"))),
    };
    let has_design = dir.join(DESIGN).is_file();
    let mut docs: Vec<String> = [DESIGN, BRIEF]
        .into_iter()
        .filter(|f| dir.join(f).is_file())
        .map(String::from)
        .collect();
    docs.extend(files(dir, "docs", ".md"));
    let mut diagrams = files(dir, "docs", ".mmd");
    diagrams.extend(files(dir, "diagrams", ".mmd"));
    let html = files(dir, "html", ".html")
        .into_iter()
        .map(|path| {
            let src = local.html.get(&path);
            let stale = src.is_some_and(|s| hash(&read_text(&dir.join(&s.source))) != s.hash);
            HtmlFile {
                source: src.map(|s| s.source.clone()),
                stale,
                path,
            }
        })
        .collect();
    DesignDto {
        name: name(slug, &brief),
        slug: slug.to_string(),
        dir: rel.to_string(),
        brief,
        has_design,
        queue,
        queue_error,
        docs,
        diagrams,
        html,
        comments: local.comments,
        attach: local.attach,
    }
}

/// The store's key for a workspace.
fn ws_key(ws: &Workspace) -> &str {
    ws.name.as_deref().unwrap_or("")
}

impl RepoCtx {
    /// The design folder `slug` in `ws`, relative to its root and absolute.
    pub fn design_dir(&self, ws: &Workspace, slug: &str) -> anyhow::Result<(String, PathBuf)> {
        anyhow::ensure!(
            slugify(slug) == slug && !slug.is_empty(),
            "bad design name {slug}"
        );
        let rel = format!("{}/{slug}", base(&ws.root));
        let abs = ws.root.join(&rel);
        Ok((rel, abs))
    }

    pub fn design_local(&self, ws: &Workspace, slug: &str) -> anyhow::Result<Local> {
        let data = self.store.lock().unwrap().design_state(ws_key(ws), slug)?;
        Ok(data
            .map(|d| serde_json::from_str(&d))
            .transpose()?
            .unwrap_or_default())
    }

    /// Changes what the store keeps about a design.
    pub fn edit_design_local<T>(
        &self,
        ws: &Workspace,
        slug: &str,
        f: impl FnOnce(&mut Local) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let store = self.store.lock().unwrap();
        let mut local: Local = store
            .design_state(ws_key(ws), slug)?
            .map(|d| serde_json::from_str(&d))
            .transpose()?
            .unwrap_or_default();
        let out = f(&mut local)?;
        store.save_design_state(ws_key(ws), slug, &serde_json::to_string(&local)?)?;
        Ok(out)
    }

    pub fn design(&self, ws: &Workspace, slug: &str) -> anyhow::Result<DesignDto> {
        let (rel, dir) = self.design_dir(ws, slug)?;
        anyhow::ensure!(dir.is_dir(), "there's no design {slug} in {rel}");
        Ok(dto(&dir, &rel, slug, self.design_local(ws, slug)?))
    }

    /// The workspace's designs: folders under the design dir with a brief or questions.
    pub fn designs(&self, ws: &Workspace) -> anyhow::Result<Vec<DesignDto>> {
        let base_dir = ws.root.join(base(&ws.root));
        let Ok(entries) = std::fs::read_dir(&base_dir) else {
            return Ok(vec![]);
        };
        let mut slugs: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join(BRIEF).is_file() || e.path().join(QUESTIONS).is_file())
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|s| slugify(s) == *s)
            .collect();
        slugs.sort();
        slugs.iter().map(|s| self.design(ws, s)).collect()
    }

    /// Starts a design called `name` with an empty brief.
    pub fn create_design(&self, ws: &Workspace, name: &str) -> anyhow::Result<DesignDto> {
        let slug = slugify(name);
        anyhow::ensure!(
            !slug.is_empty(),
            "give the design a name with letters or digits"
        );
        let (rel, dir) = self.design_dir(ws, &slug)?;
        anyhow::ensure!(!dir.exists(), "{rel} already exists");
        std::fs::create_dir_all(&dir).with_context(|| format!("can't create {rel}"))?;
        std::fs::write(dir.join(BRIEF), format!("# {}\n\n", name.trim()))?;
        write_queue(&dir, &Queue::default())?;
        self.design(ws, &slug)
    }

    pub fn save_brief(
        &self,
        ws: &Workspace,
        slug: &str,
        brief: &str,
        attach: Vec<String>,
    ) -> anyhow::Result<DesignDto> {
        let (_, dir) = self.design_dir(ws, slug)?;
        std::fs::write(dir.join(BRIEF), brief).context("can't write the brief")?;
        self.edit_design_local(ws, slug, |l| {
            l.attach = attach;
            Ok(())
        })?;
        self.design(ws, slug)
    }

    /// A file in the design's folder.
    pub fn design_file(&self, ws: &Workspace, slug: &str, path: &str) -> anyhow::Result<String> {
        let (_, dir) = self.design_dir(ws, slug)?;
        let file = inside(&dir, path)?;
        std::fs::read_to_string(&file).with_context(|| format!("can't read {path}"))
    }

    /// What `path` said when the user last looked, and what it says now.
    pub fn design_seen(
        &self,
        ws: &Workspace,
        slug: &str,
        path: &str,
    ) -> anyhow::Result<Option<String>> {
        Ok(self.design_local(ws, slug)?.seen.get(path).cloned())
    }

    /// Records `text` as what the user has seen of `path`.
    pub fn mark_design_seen(
        &self,
        ws: &Workspace,
        slug: &str,
        path: &str,
        text: String,
    ) -> anyhow::Result<()> {
        self.edit_design_local(ws, slug, |l| {
            l.seen.insert(path.to_string(), text);
            Ok(())
        })
    }

    pub fn add_design_comment(
        &self,
        ws: &Workspace,
        slug: &str,
        mut comment: DocComment,
    ) -> anyhow::Result<DesignDto> {
        let (_, dir) = self.design_dir(ws, slug)?;
        inside(&dir, &comment.path)?;
        self.edit_design_local(ws, slug, |l| {
            l.next_comment += 1;
            comment.id = l.next_comment;
            comment.state = CommentState::Queued;
            comment.reply = None;
            l.comments.push(comment);
            Ok(())
        })?;
        self.design(ws, slug)
    }

    pub fn edit_design_comment(
        &self,
        ws: &Workspace,
        slug: &str,
        id: u32,
        body: Option<&str>,
    ) -> anyhow::Result<DesignDto> {
        self.edit_design_local(ws, slug, |l| {
            match body {
                Some(body) => {
                    let c = l
                        .comments
                        .iter_mut()
                        .find(|c| c.id == id)
                        .context("no such comment")?;
                    c.body = body.to_string();
                }
                None => l.comments.retain(|c| c.id != id),
            }
            Ok(())
        })?;
        self.design(ws, slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_folder_names() {
        assert_eq!(
            slugify("  Workspace templates! v2 "),
            "workspace-templates-v2"
        );
        assert_eq!(slugify("--"), "");
    }

    #[test]
    fn paths_stay_inside_the_design() {
        let dir = Path::new("/r/docs/design/x");
        assert_eq!(inside(dir, "docs/a.md").unwrap(), dir.join("docs/a.md"));
        for bad in ["../y/a.md", "/etc/passwd", "docs/../../a", ""] {
            assert!(inside(dir, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_folder_reads_into_its_parts() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join("docs")).unwrap();
        std::fs::create_dir_all(dir.join("html")).unwrap();
        std::fs::write(dir.join(BRIEF), "Intro\n# Templates\n").unwrap();
        std::fs::write(dir.join(DESIGN), "d").unwrap();
        std::fs::write(dir.join("docs/b.md"), "").unwrap();
        std::fs::write(dir.join("docs/a.md"), "overview").unwrap();
        std::fs::write(dir.join("docs/flow.mmd"), "").unwrap();
        std::fs::write(dir.join("html/a.html"), "").unwrap();
        std::fs::write(dir.join("html/raw.html"), "").unwrap();
        std::fs::write(dir.join(QUESTIONS), "{").unwrap();
        let mut local = Local::default();
        local.html.insert(
            "html/a.html".into(),
            HtmlSource {
                source: "docs/a.md".into(),
                hash: hash("old overview"),
            },
        );
        let d = dto(dir, "docs/design/t", "t", local);
        assert_eq!(d.name, "Templates");
        assert_eq!(d.docs, ["design.md", "brief.md", "docs/a.md", "docs/b.md"]);
        assert_eq!(d.diagrams, ["docs/flow.mmd"]);
        assert!(d.queue_error.is_some());
        let stale: Vec<_> = d.html.iter().map(|h| (h.path.as_str(), h.stale)).collect();
        assert_eq!(stale, [("html/a.html", true), ("html/raw.html", false)]);
    }
}
