//! Resolving extracted imports to the repo-relative files they name. Imports that resolve to
//! nothing, like third-party ones, are dropped.

use std::collections::{BTreeMap, BTreeSet};

use super::extract::{Raw, Spec};
use super::roots::{Manifests, dir_of, join, under};

const JS_EXTS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

/// One side's files and what's needed to resolve imports among them.
pub struct Resolver<'a> {
    paths: &'a BTreeSet<String>,
    /// Folders with a `Cargo.toml` and a `src/lib.rs` or `src/main.rs`.
    crates: Vec<String>,
    /// go.mod folders and their module paths.
    go_mods: Vec<(String, String)>,
    /// Folders absolute Python imports are relative to.
    py_roots: Vec<String>,
    /// Go files per folder, sorted.
    go_dirs: BTreeMap<&'a str, Vec<&'a str>>,
}

impl<'a> Resolver<'a> {
    pub fn new(paths: &'a BTreeSet<String>, manifests: &Manifests) -> Self {
        let mut crates = Vec::new();
        let mut go_mods = Vec::new();
        let mut py_roots = vec![String::new()];
        let mut go_dirs: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for p in paths {
            let (dir, name) = split(p);
            match name {
                "Cargo.toml" => {
                    let src = join(dir, "src");
                    if ["lib.rs", "main.rs"]
                        .iter()
                        .any(|f| paths.contains(&join(&src, f)))
                    {
                        crates.push(dir.to_string());
                    }
                }
                "go.mod" => {
                    if let Some(m) = manifests.go_module(p) {
                        go_mods.push((dir.to_string(), m));
                    }
                }
                "pyproject.toml" | "setup.py" | "setup.cfg" => {
                    py_roots.push(dir.to_string());
                    py_roots.push(join(dir, "src"));
                }
                _ if name.ends_with(".go") && !name.ends_with("_test.go") => {
                    go_dirs.entry(dir).or_default().push(p);
                }
                _ => {}
            }
        }
        // Deepest first, so a nested crate or module wins.
        crates.sort_by_key(|c| std::cmp::Reverse(c.len()));
        go_mods.sort_by_key(|(d, _)| std::cmp::Reverse(d.len()));
        py_roots.sort();
        py_roots.dedup();
        Resolver {
            paths,
            crates,
            go_mods,
            py_roots,
            go_dirs,
        }
    }

    /// The files `raw`, imported from `from`, names. Never `from` itself.
    pub fn resolve(&self, from: &str, raw: &Raw) -> Vec<String> {
        let mut out = match &raw.spec {
            Spec::Rust(segs) => self.rust(from, segs).into_iter().collect(),
            Spec::Js(spec) => self.js(from, spec).into_iter().collect(),
            Spec::Py {
                level,
                module,
                names,
            } => self.py(from, *level, module, names),
            Spec::Go(path) => self.go(path).into_iter().collect(),
        };
        out.retain(|p| p != from);
        out
    }

    fn has(&self, p: &str) -> bool {
        self.paths.contains(p)
    }

    fn rust(&self, from: &str, segs: &[String]) -> Option<String> {
        let krate = self.crates.iter().find(|c| under(from, &join(c, "src")))?;
        let src = join(krate, "src");
        let rel = &from[src.len() + 1..];
        let mut module: Vec<&str> = rel.trim_end_matches(".rs").split('/').collect();
        if matches!(rel, "lib.rs" | "main.rs") || module.last() == Some(&"mod") {
            module.pop();
        }
        let mut rest = segs;
        let mut base: Vec<&str> = match rest.first().map(String::as_str) {
            Some("crate") => vec![],
            _ => module,
        };
        while let Some(first) = rest.first() {
            match first.as_str() {
                "crate" | "self" => {}
                "super" => {
                    base.pop();
                }
                _ => break,
            }
            rest = &rest[1..];
        }
        let root = if from == join(&src, "main.rs") || !self.has(&join(&src, "lib.rs")) {
            join(&src, "main.rs")
        } else {
            join(&src, "lib.rs")
        };
        let module_file = |m: &[&str]| -> Option<String> {
            if m.is_empty() {
                return Some(root.clone());
            }
            let p = join(&src, &m.join("/"));
            [format!("{p}.rs"), format!("{p}/mod.rs")]
                .into_iter()
                .find(|f| self.has(f))
        };
        let mut best = module_file(&base)?;
        for seg in rest {
            base.push(seg);
            match module_file(&base) {
                Some(f) => best = f,
                None => break,
            }
        }
        Some(best)
    }

    fn js(&self, from: &str, spec: &str) -> Option<String> {
        let p = normalize(&join(dir_of(from), spec))?;
        let stem = ["js", "jsx", "mjs", "cjs"]
            .iter()
            .find_map(|e| p.strip_suffix(&format!(".{e}")));
        let mut candidates = vec![p.clone()];
        candidates.extend(JS_EXTS.iter().map(|e| format!("{p}.{e}")));
        if let Some(stem) = stem {
            candidates.extend(JS_EXTS.iter().map(|e| format!("{stem}.{e}")));
        }
        candidates.extend(JS_EXTS.iter().map(|e| join(&p, &format!("index.{e}"))));
        candidates
            .into_iter()
            .find(|c| self.has(c) && super::extract::is_source(c))
    }

    fn py(&self, from: &str, level: u32, module: &[String], names: &[String]) -> Vec<String> {
        if level > 0 {
            let mut dir = dir_of(from).to_string();
            for _ in 1..level {
                dir = dir_of(&dir).to_string();
            }
            return self.py_in(&dir, module, names);
        }
        self.py_roots
            .iter()
            .map(|r| self.py_in(r, module, names))
            .find(|found| !found.is_empty())
            .unwrap_or_default()
    }

    /// The deepest module of `module` under `dir`, or the submodules `names` of it.
    fn py_in(&self, dir: &str, module: &[String], names: &[String]) -> Vec<String> {
        let file = |m: &str| -> Option<String> {
            let p = join(dir, m);
            [format!("{p}.py"), join(&p, "__init__.py")]
                .into_iter()
                .find(|f| self.has(f))
        };
        let full = module.join("/");
        let subs: Vec<String> = names.iter().filter_map(|n| file(&join(&full, n))).collect();
        if !subs.is_empty() {
            return subs;
        }
        (1..=module.len())
            .rev()
            .find_map(|n| file(&module[..n].join("/")))
            .or_else(|| {
                (module.is_empty() && !dir.is_empty())
                    .then(|| file(""))
                    .flatten()
            })
            .into_iter()
            .collect()
    }

    fn go(&self, path: &str) -> Option<String> {
        let (dir, rest) = self.go_mods.iter().find_map(|(dir, m)| {
            let rest = path.strip_prefix(m.as_str())?;
            (rest.is_empty() || rest.starts_with('/')).then(|| (dir, rest.trim_start_matches('/')))
        })?;
        let target = join(dir, rest);
        self.go_dirs
            .get(target.as_str())?
            .first()
            .map(|s| s.to_string())
    }
}

fn split(p: &str) -> (&str, &str) {
    p.rsplit_once('/').unwrap_or(("", p))
}

/// Resolves `.` and `..` segments. `None` when it leaves the repo.
fn normalize(p: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s),
        }
    }
    Some(out.join("/"))
}
