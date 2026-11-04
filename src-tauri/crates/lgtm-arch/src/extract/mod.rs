//! Import extraction: the imports a file's syntax names, before they're resolved to files.
//! Parsing is the slow part, so results are cached by content hash.

mod go;
mod js;
mod py;
mod rust;

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use tree_sitter::Node;

use lgtm_core::syntax;

/// What an import names, in its language's terms.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Spec {
    /// A path starting with `crate`, `super` or `self`, like `["crate", "jj", "diff"]`.
    Rust(Vec<String>),
    /// A relative specifier, like `./views` or `../lib/index.js`.
    Js(String),
    /// `level` leading dots (0 for absolute), the dotted module, and the names it imports.
    Py {
        level: u32,
        module: Vec<String>,
        names: Vec<String>,
    },
    /// The full import path.
    Go(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Raw {
    pub spec: Spec,
    /// 1-based.
    pub line: u32,
}

/// Extensions extraction understands. Only files with these are components' files.
pub const SOURCE_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "py", "go",
];

pub fn is_source(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| SOURCE_EXTS.contains(&ext))
}

/// Past this many entries the cache starts over.
pub const MAX_CACHED: usize = 100_000;

pub type Cache = Mutex<HashMap<[u8; 32], Arc<Vec<Raw>>>>;

pub static CACHE: LazyLock<Cache> = LazyLock::new(Mutex::default);

/// The imports `text` names, cached by its path's extension and content.
pub fn imports(path: &str, text: &str) -> Arc<Vec<Raw>> {
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    let mut h = blake3::Hasher::new();
    h.update(ext.as_bytes());
    h.update(&[0]);
    h.update(text.as_bytes());
    let key = *h.finalize().as_bytes();
    if let Some(hit) = CACHE.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let raw = Arc::new(parse(path, text));
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= MAX_CACHED {
        cache.clear();
    }
    cache.insert(key, raw.clone());
    raw
}

pub fn parse(path: &str, text: &str) -> Vec<Raw> {
    if !is_source(path) {
        return vec![];
    }
    let Some(parsed) = syntax::parse(path, text) else {
        return vec![];
    };
    let root = parsed.root();
    let mut out = Vec::new();
    let ext = path.rsplit_once('.').map_or("", |(_, e)| e);
    match ext {
        "rs" => rust::imports(root, text, &mut out),
        "py" => py::imports(root, text, &mut out),
        "go" => go::imports(root, text, &mut out),
        _ => js::imports(root, text, &mut out),
    }
    out
}

/// Every node under `root`, depth first, without recursion. `enter` says whether to visit a
/// node's children.
pub fn walk<'t>(root: Node<'t>, mut enter: impl FnMut(Node<'t>) -> bool) {
    let mut cursor = root.walk();
    loop {
        let node = cursor.node();
        if enter(node) && cursor.goto_first_child() {
            continue;
        }
        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() || cursor.node() == root {
                return;
            }
        }
    }
}

pub fn text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

pub fn line(node: Node) -> u32 {
    node.start_position().row as u32 + 1
}

/// A string literal's contents, without quotes.
pub fn unquote(s: &str) -> &str {
    let s = s.trim();
    let q = |c: char| matches!(c, '"' | '\'' | '`');
    s.strip_prefix(q)
        .and_then(|s| s.strip_suffix(q))
        .unwrap_or(s)
}
