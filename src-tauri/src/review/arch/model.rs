//! What the architecture diff is computed from, and the component and edge diff itself.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::group::Grouping;
use super::{CompStatus, ComponentDto, EdgeDto, EdgeStatus, FileRefDto, ImportDto, RangeDto};
use crate::diff::FileStatus;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub to: String,
    /// 1-based.
    pub line: u32,
}

/// One side's source files and the files each imports.
pub type Snapshot = BTreeMap<String, Vec<Import>>;

/// A file the diff changes.
#[derive(Clone, Debug)]
pub struct Changed {
    pub path: String,
    /// Where a renamed or copied file came from.
    pub old_path: Option<String>,
    pub status: FileStatus,
    /// The review file, when the diff is the review's own.
    pub file: Option<u32>,
    /// Changed rows' line spans, new side or old side for a removed file.
    pub ranges: Vec<RangeDto>,
}

/// A symbol whose code uses a changed symbol, or (past one hop) a user of one.
#[derive(Clone, Debug)]
pub struct Use {
    pub from_path: String,
    /// The user is itself in a changed file.
    pub from_changed: bool,
    /// 0 when the user is a changed symbol, else how many uses away it is.
    pub hop: u32,
    pub to_path: String,
    pub to_name: String,
    pub to_line: u32,
    /// 0 when the used symbol is a changed one.
    pub to_hop: u32,
}

/// A changed symbol.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub base: Snapshot,
    pub head: Snapshot,
    pub changed: Vec<Changed>,
    pub uses: Vec<Use>,
    pub changed_symbols: Vec<Symbol>,
    /// Symbol names at head, for changed files and files Move intents name.
    pub symbols: HashMap<String, Vec<String>>,
}

pub type EdgeMap = BTreeMap<(String, String), Vec<ImportDto>>;

/// Component-level imports on one side.
pub fn edges(snap: &Snapshot, g: &Grouping) -> EdgeMap {
    let mut out: EdgeMap = BTreeMap::new();
    for (from, imports) in snap {
        let a = g.comp(from);
        for i in imports {
            let b = g.comp(&i.to);
            if a != b {
                out.entry((a.clone(), b)).or_default().push(ImportDto {
                    from_path: from.clone(),
                    to_path: i.to.clone(),
                    line: i.line,
                });
            }
        }
    }
    out
}

pub fn comps(snap: &Snapshot, g: &Grouping) -> BTreeSet<String> {
    snap.keys().map(|p| g.comp(p)).collect()
}

pub struct Diff {
    pub components: Vec<ComponentDto>,
    pub edges: Vec<EdgeDto>,
}

impl Diff {
    pub fn comp(&self, id: &str) -> Option<&ComponentDto> {
        self.components.iter().find(|c| c.id == id)
    }

    pub fn edge(&self, from: &str, to: &str) -> Option<&EdgeDto> {
        self.edges.iter().find(|e| e.from == from && e.to == to)
    }
}

fn is_changed(s: CompStatus) -> bool {
    matches!(
        s,
        CompStatus::Added | CompStatus::Removed | CompStatus::Changed
    )
}

/// Components and edges at base and head, with their statuses.
pub fn diff(m: &Model, g: &Grouping, transitive: bool) -> Diff {
    let file_of: HashMap<&str, Option<u32>> = m
        .changed
        .iter()
        .map(|c| (c.path.as_str(), c.file))
        .collect();
    let mut changed_paths: BTreeSet<&str> = BTreeSet::new();
    for c in &m.changed {
        changed_paths.insert(&c.path);
        if let Some(old) = &c.old_path {
            changed_paths.insert(old);
        }
    }
    let (at_base, at_head) = (comps(&m.base, g), comps(&m.head, g));
    let mut files: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for p in m
        .head
        .keys()
        .chain(m.base.keys().filter(|p| !m.head.contains_key(*p)))
    {
        files.entry(g.comp(p)).or_default().insert(p);
    }
    let mut status: BTreeMap<String, CompStatus> = BTreeMap::new();
    for (id, fs) in &files {
        let s = if !at_base.contains(id) {
            CompStatus::Added
        } else if !at_head.contains(id) {
            CompStatus::Removed
        } else if fs.iter().any(|f| changed_paths.contains(f)) {
            CompStatus::Changed
        } else {
            CompStatus::Unchanged
        };
        status.insert(id.clone(), s);
    }
    let (base_edges, head_edges) = (edges(&m.base, g), edges(&m.head, g));
    for (a, b) in head_edges.keys().chain(base_edges.keys()) {
        if is_changed(status[a]) && status[b] == CompStatus::Unchanged {
            status.insert(b.clone(), CompStatus::Upstream);
        }
    }
    let mut impacted_edges: BTreeSet<(String, String)> = BTreeSet::new();
    let mut uses: Vec<&Use> = m
        .uses
        .iter()
        .filter(|u| !u.from_changed && u.hop >= 1 && (u.hop == 1 || transitive))
        .collect();
    uses.sort_by_key(|u| u.hop);
    for u in uses {
        let (a, b) = (g.comp(&u.from_path), g.comp(&u.to_path));
        let reaches =
            |s: Option<&CompStatus>| s.is_some_and(|&s| is_changed(s) || s == CompStatus::Impacted);
        let user = status.get(&a).copied();
        if a == b
            || !matches!(
                user,
                Some(CompStatus::Unchanged | CompStatus::Upstream | CompStatus::Impacted)
            )
            || !reaches(status.get(&b))
            || !head_edges.contains_key(&(a.clone(), b.clone()))
        {
            continue;
        }
        status.insert(a.clone(), CompStatus::Impacted);
        impacted_edges.insert((a, b));
    }
    let components = files
        .into_iter()
        .map(|(id, fs)| ComponentDto {
            name: g.name(&id),
            container: g.container(&id),
            files: fs
                .into_iter()
                .map(|p| FileRefDto {
                    path: p.to_string(),
                    file: file_of.get(p).copied().flatten(),
                })
                .collect(),
            status: status[&id],
            layer: None,
            risk: false,
            trust: false,
            note: None,
            misplaced: vec![],
            severity: None,
            id,
        })
        .collect();
    let mut edges: Vec<EdgeDto> = Vec::new();
    for (key, imports) in &head_edges {
        edges.push(EdgeDto {
            from: key.0.clone(),
            to: key.1.clone(),
            status: if base_edges.contains_key(key) {
                EdgeStatus::Same
            } else {
                EdgeStatus::Added
            },
            impacted: impacted_edges.contains(key),
            imports: imports.clone(),
            wrong: None,
        });
    }
    for (key, imports) in base_edges
        .iter()
        .filter(|(k, _)| !head_edges.contains_key(*k))
    {
        edges.push(EdgeDto {
            from: key.0.clone(),
            to: key.1.clone(),
            status: EdgeStatus::Removed,
            impacted: false,
            imports: imports.clone(),
            wrong: None,
        });
    }
    Diff { components, edges }
}
