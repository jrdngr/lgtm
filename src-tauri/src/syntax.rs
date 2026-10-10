//! Tree-sitter: languages, highlighting, and symbol ranges.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde::Serialize;
use specta::Type;
use tree_sitter::{Language, Node, Parser, Query, QueryCursor, StreamingIterator, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

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

struct Lang {
    language: Language,
    highlight: HighlightConfiguration,
    symbols: Query,
    /// Preceding sibling kinds that belong to the symbol after them.
    leading: &'static [&'static str],
    /// Parent kinds that wrap a symbol together with its decorators.
    wrappers: &'static [&'static str],
}

const RUST_SYMBOLS: &str = r#"
(function_item name: (identifier) @name) @symbol
(struct_item name: (type_identifier) @name) @symbol
(enum_item name: (type_identifier) @name) @symbol
(union_item name: (type_identifier) @name) @symbol
(trait_item name: (type_identifier) @name) @symbol
(impl_item type: (_) @name) @symbol
(mod_item name: (identifier) @name) @symbol
(macro_definition name: (identifier) @name) @symbol
"#;

const PYTHON_SYMBOLS: &str = r#"
(function_definition name: (identifier) @name) @symbol
(class_definition name: (identifier) @name) @symbol
"#;

const JS_SYMBOLS: &str = r#"
(function_declaration name: (identifier) @name) @symbol
(generator_function_declaration name: (identifier) @name) @symbol
(class_declaration name: (_) @name) @symbol
(method_definition name: (_) @name) @symbol
(lexical_declaration
  (variable_declarator name: (identifier) @name value: [(arrow_function) (function_expression)])) @symbol
"#;

const TS_SYMBOLS: &str = r#"
(interface_declaration name: (_) @name) @symbol
(abstract_class_declaration name: (_) @name) @symbol
(enum_declaration name: (_) @name) @symbol
(type_alias_declaration name: (_) @name) @symbol
(internal_module name: (_) @name) @symbol
"#;

const GO_SYMBOLS: &str = r#"
(function_declaration name: (identifier) @name) @symbol
(method_declaration name: (field_identifier) @name) @symbol
(type_declaration (type_spec name: (type_identifier) @name)) @symbol
"#;

fn lang(
    language: Language,
    name: &str,
    highlights: &str,
    symbols: &str,
    leading: &'static [&'static str],
    wrappers: &'static [&'static str],
) -> Lang {
    let mut highlight = HighlightConfiguration::new(language.clone(), name, highlights, "", "")
        .unwrap_or_else(|e| panic!("{name} highlights query: {e}"));
    highlight.configure(HIGHLIGHT_NAMES);
    let symbols =
        Query::new(&language, symbols).unwrap_or_else(|e| panic!("{name} symbols query: {e}"));
    Lang {
        language,
        highlight,
        symbols,
        leading,
        wrappers,
    }
}

/// Languages by file extension, built once.
static LANGS: LazyLock<HashMap<&'static str, Lang>> = LazyLock::new(|| {
    let js_highlights = tree_sitter_javascript::HIGHLIGHT_QUERY;
    let ts_highlights = [js_highlights, tree_sitter_typescript::HIGHLIGHTS_QUERY].concat();
    let ts_symbols = [JS_SYMBOLS, TS_SYMBOLS].concat();
    let js_leading: &[&str] = &["decorator", "comment"];
    let none: &[&str] = &[];

    let mut m = HashMap::new();
    m.insert(
        "rs",
        lang(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            RUST_SYMBOLS,
            &["attribute_item", "line_comment", "block_comment"],
            none,
        ),
    );
    m.insert(
        "py",
        lang(
            tree_sitter_python::LANGUAGE.into(),
            "python",
            tree_sitter_python::HIGHLIGHTS_QUERY,
            PYTHON_SYMBOLS,
            &["comment"],
            &["decorated_definition"],
        ),
    );
    m.insert(
        "ts",
        lang(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
            &ts_highlights,
            &ts_symbols,
            js_leading,
            &["export_statement"],
        ),
    );
    m.insert(
        "tsx",
        lang(
            tree_sitter_typescript::LANGUAGE_TSX.into(),
            "tsx",
            &[
                ts_highlights.as_str(),
                tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
            ]
            .concat(),
            &ts_symbols,
            js_leading,
            &["export_statement"],
        ),
    );
    m.insert(
        "js",
        lang(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
            &[js_highlights, tree_sitter_javascript::JSX_HIGHLIGHT_QUERY].concat(),
            JS_SYMBOLS,
            js_leading,
            &["export_statement"],
        ),
    );
    m.insert(
        "go",
        lang(
            tree_sitter_go::LANGUAGE.into(),
            "go",
            tree_sitter_go::HIGHLIGHTS_QUERY,
            GO_SYMBOLS,
            &["comment"],
            none,
        ),
    );
    m.insert(
        "json",
        lang(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
            "",
            none,
            none,
        ),
    );
    m.insert(
        "yaml",
        lang(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            "",
            none,
            none,
        ),
    );
    m.insert(
        "toml",
        lang(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
            "",
            none,
            none,
        ),
    );
    m.insert(
        "sh",
        lang(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
            "",
            none,
            none,
        ),
    );
    m
});

fn lang_key(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.').map(|(_, e)| e)?;
    Some(match ext {
        "rs" => "rs",
        "py" | "pyi" => "py",
        "ts" | "mts" | "cts" => "ts",
        "tsx" => "tsx",
        "js" | "jsx" | "mjs" | "cjs" => "js",
        "go" => "go",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" => "sh",
        _ => return None,
    })
}

/// A parsed file side.
pub struct Parsed {
    lang: &'static Lang,
    tree: Tree,
}

pub fn parse(path: &str, source: &str) -> Option<Parsed> {
    let lang = LANGS.get(lang_key(path)?)?;
    let mut parser = Parser::new();
    parser.set_language(&lang.language).ok()?;
    let tree = parser.parse(source, None)?;
    Some(Parsed { lang, tree })
}

impl Parsed {
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
                kind: kind_of(node.kind()),
                start_line: start.start_position().row as u32 + 1,
                end_line: end.end_position().row as u32 + 1,
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
        let mut out: HashMap<u32, Vec<Segment>> = HashMap::new();
        let mut highlighter = Highlighter::new();
        let Ok(events) =
            highlighter.highlight(&self.lang.highlight, source.as_bytes(), None, None, |_| {
                None
            })
        else {
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
                        if piece.is_empty() || !wanted.contains(&line) {
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
}

fn kind_of(node_kind: &str) -> SymbolKind {
    match node_kind {
        "mod_item" | "internal_module" => SymbolKind::Module,
        k if k.contains("function") || k.contains("method") || k == "lexical_declaration" => {
            SymbolKind::Function
        }
        "macro_definition" => SymbolKind::Function,
        _ => SymbolKind::Class,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols(path: &str, src: &str) -> Vec<(String, u32, u32)> {
        parse(path, src)
            .unwrap()
            .symbols(src)
            .into_iter()
            .map(|s| (s.name, s.start_line, s.end_line))
            .collect()
    }

    #[test]
    fn every_language_builds() {
        assert_eq!(LazyLock::force(&LANGS).len(), 10);
    }

    #[test]
    fn python_symbol_includes_decorators() {
        let src = "import x\n\n@app.get('/')\n@cached\ndef get_user():\n    return 1\n";
        assert_eq!(symbols("a.py", src), [("get_user".into(), 3, 6)]);
    }

    #[test]
    fn rust_symbol_includes_attributes_and_doc_comments_but_not_detached_comments() {
        let src = "// unrelated\n\n/// Docs.\n#[inline]\nfn f() {}\n\nimpl S {\n    fn g() {}\n}\n";
        assert_eq!(
            symbols("a.rs", src),
            [("f".into(), 3, 5), ("S".into(), 7, 9), ("g".into(), 8, 8)]
        );
    }

    #[test]
    fn typescript_method_includes_decorator_and_exported_function_includes_export() {
        let src = "class A {\n  @dec()\n  m() {}\n}\nexport function f() {\n}\n";
        assert_eq!(
            symbols("a.ts", src),
            [("A".into(), 1, 4), ("m".into(), 2, 3), ("f".into(), 5, 6)]
        );
    }

    #[test]
    fn symbol_with_parse_error_is_flagged() {
        let src = "fn ok() {}\nfn broken( {\n}\n";
        let syms = parse("a.rs", src).unwrap().symbols(src);
        assert!(!syms[0].has_error);
        assert!(syms.iter().skip(1).all(|s| s.has_error), "{syms:?}");
    }

    #[test]
    fn highlight_segments_cover_requested_lines_in_utf16_units() {
        let src = "fn main() {\n    let s = \"é\";\n}\n";
        let lines = parse("a.rs", src).unwrap().highlight(src, &[2]);
        assert_eq!(lines.keys().collect::<Vec<_>>(), [&2]);
        let segs = &lines[&2];
        assert_eq!(
            segs.iter().map(|s| s.1).sum::<u32>(),
            "    let s = \"é\";".encode_utf16().count() as u32
        );
        let keyword = HIGHLIGHT_NAMES
            .iter()
            .position(|n| *n == "keyword")
            .unwrap() as u8;
        assert!(segs.iter().any(|s| s.0 == Some(keyword)));
    }

    #[test]
    fn unknown_extension_has_no_parser() {
        assert!(parse("notes.txt", "hi").is_none());
    }
}
