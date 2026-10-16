//! Tree-sitter: languages, highlighting, and symbol ranges.

mod langs;
mod queries;
mod refs;

pub use refs::Ref;

use std::collections::HashMap;

use serde::Serialize;
use specta::Type;
use tree_sitter::{Node, Parser, QueryCursor, StreamingIterator, Tree};
use tree_sitter_highlight::{HighlightEvent, Highlighter};

use langs::{LANGS, Lang, injected, lang_key, lang_named};

/// Capture names the UI styles, as `hl-<index>` classes. Order is part of the UI contract.
pub const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constructor",
    "escape",
    "function",
    "keyword",
    "label",
    "module",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "tag",
    "type",
    "variable.builtin",
    "variable.parameter",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SymbolKind {
    Function,
    Class,
    Module,
    Section,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CodeSymbol {
    pub name: String,
    pub kind: SymbolKind,
    /// 1-based, inclusive. Includes leading decorators, attributes, and doc comments.
    pub start_line: u32,
    pub end_line: u32,
    /// The parse has errors inside the range, so the range can't be trusted.
    pub has_error: bool,
}

/// A run of `len` UTF-16 code units styled as `HIGHLIGHT_NAMES[class]`, or unstyled.
pub type Segment = (Option<u8>, u32);

/// A parsed file side.
pub struct Parsed {
    lang: &'static Lang,
    tree: Tree,
}

/// Whether `path` is in a language with symbols.
pub fn has_symbols(path: &str) -> bool {
    lang_key(path)
        .and_then(|k| LANGS.get(k))
        .is_some_and(|l| l.symbols.pattern_count() > 0)
}

pub fn parse(path: &str, source: &str) -> Option<Parsed> {
    let lang = LANGS.get(lang_key(path)?)?;
    let mut parser = Parser::new();
    parser.set_language(&lang.language).ok()?;
    let tree = parser.parse(source, None)?;
    Some(Parsed { lang, tree })
}

impl Parsed {
    /// The syntax tree's root, for walks the queries here don't cover.
    pub fn root(&self) -> Node<'_> {
        self.tree.root_node()
    }

    pub fn symbols(&self, source: &str) -> Vec<CodeSymbol> {
        let query = &self.lang.symbols;
        let name_idx = query.capture_index_for_name("name");
        let symbol_idx = query.capture_index_for_name("symbol");
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(query, self.tree.root_node(), source.as_bytes());
        let mut out = Vec::new();
        while let Some(m) = matches.next() {
            let node_of = |idx| {
                m.captures()
                    .iter()
                    .find(|c| Some(c.index) == idx)
                    .map(|c| c.node)
            };
            let (Some(node), Some(name)) = (node_of(symbol_idx), node_of(name_idx)) else {
                continue;
            };
            let (start, end) = self.extent(node);
            let has_error = start.has_error() || node.has_error() || end.has_error();
            out.push(CodeSymbol {
                name: source[name.byte_range()].to_string(),
                kind: if self.lang.outline {
                    SymbolKind::Section
                } else {
                    kind_of(node.kind())
                },
                start_line: start.start_position().row as u32 + 1,
                end_line: last_row(end) as u32 + 1,
                has_error,
            });
        }
        out.sort_by_key(|s| (s.start_line, std::cmp::Reverse(s.end_line)));
        out.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);
        out
    }

    /// The first and last nodes of a symbol, widened to its wrapper and to adjacent leading
    /// siblings (decorators, attributes, doc comments) with no blank line between.
    fn extent<'t>(&self, node: Node<'t>) -> (Node<'t>, Node<'t>) {
        let mut node = node;
        while let Some(parent) = node
            .parent()
            .filter(|p| self.lang.wrappers.contains(&p.kind()))
        {
            node = parent;
        }
        let mut start = node;
        while let Some(prev) = start.prev_sibling() {
            let adjacent = prev.end_position().row + 1 >= start.start_position().row;
            if !adjacent || !self.lang.leading.contains(&prev.kind()) {
                break;
            }
            start = prev;
        }
        (start, node)
    }

    /// Highlight segments for the given 1-based lines.
    pub fn highlight(&self, source: &str, lines: &[u32]) -> HashMap<u32, Vec<Segment>> {
        let wanted: std::collections::HashSet<u32> = lines.iter().copied().collect();
        highlight_lines(self.lang, source, |l| wanted.contains(&l))
    }
}

/// Highlight segments for every line of `source`, empty when `path` has no grammar.
pub fn highlight_file(path: &str, source: &str) -> Vec<Vec<Segment>> {
    // A rendered fence passes `code.<lang>`, where the language may be a grammar name.
    let named = || lang_named(path.rsplit_once('.')?.1);
    let Some(lang) = lang_key(path).and_then(|k| LANGS.get(k)).or_else(named) else {
        return vec![];
    };
    let mut by_line = highlight_lines(lang, source, |_| true);
    let count = source.split('\n').count() as u32;
    (1..=count)
        .map(|l| by_line.remove(&l).unwrap_or_default())
        .collect()
}

fn highlight_lines(
    lang: &Lang,
    source: &str,
    wanted: impl Fn(u32) -> bool,
) -> HashMap<u32, Vec<Segment>> {
    let mut out: HashMap<u32, Vec<Segment>> = HashMap::new();
    let mut highlighter = Highlighter::new();
    let Ok(events) = highlighter.highlight(&lang.highlight, source.as_bytes(), None, None, |n| {
        injected(n)
    }) else {
        return out;
    };
    let mut stack: Vec<u8> = Vec::new();
    let mut line = 1u32;
    for event in events {
        match event {
            Ok(HighlightEvent::HighlightStart(h)) => stack.push(h.0 as u8),
            Ok(HighlightEvent::HighlightEnd) => {
                stack.pop();
            }
            Ok(HighlightEvent::Source { start, end }) => {
                for (i, piece) in source[start..end].split('\n').enumerate() {
                    if i > 0 {
                        line += 1;
                    }
                    let piece = piece.strip_suffix('\r').unwrap_or(piece);
                    if piece.is_empty() || !wanted(line) {
                        continue;
                    }
                    let len = piece.encode_utf16().count() as u32;
                    let segs = out.entry(line).or_default();
                    let class = stack.last().copied();
                    match segs.last_mut() {
                        Some((c, l)) if *c == class => *l += len,
                        _ => segs.push((class, len)),
                    }
                }
            }
            Err(_) => break,
        }
    }
    out
}

/// The node's last row. Block grammars like Markdown and YAML end a node at the start of
/// the next line, which isn't part of it.
fn last_row(node: Node) -> usize {
    let end = node.end_position();
    if end.column == 0 && end.row > node.start_position().row {
        end.row - 1
    } else {
        end.row
    }
}

fn kind_of(node_kind: &str) -> SymbolKind {
    match node_kind {
        "mod_item" | "internal_module" => SymbolKind::Module,
        k if k.contains("function") || k.contains("method") || k == "lexical_declaration" => {
            SymbolKind::Function
        }
        "macro_definition"
        | "init_declaration"
        | "deinit_declaration"
        | "subscript_declaration" => SymbolKind::Function,
        _ => SymbolKind::Class,
    }
}

#[cfg(test)]
mod tests;
