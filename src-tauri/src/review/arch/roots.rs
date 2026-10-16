//! Source roots: crate `src/` folders, package roots, Go modules and a top-level `src/`.
//! Each is a container whose subfolders are components.

use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    /// Repo-relative, "" for the repo root.
    pub path: String,
    pub name: String,
}

/// Manifest texts by path: `Cargo.toml`, `package.json`, `go.mod` and `pyproject.toml`.
#[derive(Clone, Debug, Default)]
pub struct Manifests(pub HashMap<String, String>);

pub fn is_manifest(path: &str) -> bool {
    matches!(
        path.rsplit('/').next(),
        Some("Cargo.toml" | "package.json" | "go.mod" | "pyproject.toml")
    )
}

impl Manifests {
    pub fn go_module(&self, path: &str) -> Option<String> {
        let text = self.0.get(path)?;
        let line = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("module "))?;
        Some(line.trim().trim_matches('"').to_string())
    }

    /// The package name a manifest declares.
    fn name(&self, path: &str) -> Option<String> {
        let text = self.0.get(path)?;
        let name = match path.rsplit('/').next()? {
            "Cargo.toml" | "pyproject.toml" => {
                let doc: toml::Table = text.parse().ok()?;
                let table = doc.get("package").or_else(|| doc.get("project"))?;
                table.get("name")?.as_str()?.to_string()
            }
            "package.json" => {
                let doc: serde_json::Value = serde_json::from_str(text).ok()?;
                doc.get("name")?.as_str()?.to_string()
            }
            "go.mod" => self.go_module(path)?.rsplit('/').next()?.to_string(),
            _ => return None,
        };
        Some(name)
    }
}

/// The source roots among `paths`, named after their package or folder. `repo` names the
/// repo root.
pub fn detect(paths: &BTreeSet<String>, manifests: &Manifests, repo: &str) -> Vec<Root> {
    let mut roots: Vec<Root> = Vec::new();
    let has_under = |dir: &str| {
        let prefix = format!("{dir}/");
        paths
            .range(prefix.clone()..)
            .next()
            .is_some_and(|p| p.starts_with(&prefix))
    };
    let folder = |dir: &str| match dir.rsplit('/').next() {
        Some("") | None => repo.to_string(),
        Some(n) => n.to_string(),
    };
    for p in paths {
        let (dir, file) = p.rsplit_once('/').unwrap_or(("", p));
        if dir.split('/').any(|s| s == "node_modules") {
            continue;
        }
        let name = || manifests.name(p).unwrap_or_else(|| folder(dir));
        let src = join(dir, "src");
        match file {
            "Cargo.toml" if has_under(&src) => add(&mut roots, src, name()),
            "package.json" | "pyproject.toml" | "setup.py" if has_under(&src) => {
                add(&mut roots, src, name())
            }
            "package.json" | "pyproject.toml" | "setup.py" | "go.mod" => {
                add(&mut roots, dir.to_string(), name())
            }
            _ => {}
        }
    }
    // Top-level Python packages outside any root.
    for p in paths {
        let Some(dir) = p
            .strip_suffix("/__init__.py")
            .or((p == "__init__.py").then_some(""))
        else {
            continue;
        };
        let parent_is_pkg = !dir.is_empty() && paths.contains(&join(dir_of(dir), "__init__.py"));
        if !dir.is_empty() && !parent_is_pkg && root_of(&roots, p).is_none() {
            add(&mut roots, dir.to_string(), folder(dir));
        }
    }
    if has_under("src") {
        add(&mut roots, "src".into(), "src".into());
    }
    roots.sort_by(|a, b| a.path.cmp(&b.path));
    roots
}

fn add(roots: &mut Vec<Root>, path: String, name: String) {
    if !roots.iter().any(|r| r.path == path) {
        roots.push(Root { path, name });
    }
}

/// The deepest root holding `path`.
pub fn root_of<'r>(roots: &'r [Root], path: &str) -> Option<&'r Root> {
    roots
        .iter()
        .filter(|r| r.path.is_empty() || path == r.path || under(path, &r.path))
        .max_by_key(|r| r.path.len())
}

/// Whether `path` is inside folder `dir`.
pub fn under(path: &str, dir: &str) -> bool {
    dir.is_empty() || path.strip_prefix(dir).is_some_and(|r| r.starts_with('/'))
}

pub fn join(dir: &str, name: &str) -> String {
    match (dir.is_empty(), name.is_empty()) {
        (true, _) => name.to_string(),
        (_, true) => dir.to_string(),
        _ => format!("{dir}/{name}"),
    }
}

pub fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}
