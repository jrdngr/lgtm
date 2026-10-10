//! The languages syntax knows: their grammars, highlight queries, and symbol queries.

use std::collections::HashMap;
use std::sync::LazyLock;

use tree_sitter::{Language, Query};
use tree_sitter_highlight::HighlightConfiguration;

use super::HIGHLIGHT_NAMES;

pub(super) struct Lang {
    pub(super) language: Language,
    pub(super) highlight: HighlightConfiguration,
    pub(super) symbols: Query,
    /// Preceding sibling kinds that belong to the symbol after them.
    pub(super) leading: &'static [&'static str],
    /// Parent kinds that wrap a symbol together with its decorators.
    pub(super) wrappers: &'static [&'static str],
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

const SWIFT_SYMBOLS: &str = r#"
(class_declaration name: (_) @name) @symbol
(protocol_declaration name: (_) @name) @symbol
(function_declaration name: (_) @name) @symbol
(protocol_function_declaration name: (_) @name) @symbol
(init_declaration "init" @name) @symbol
(deinit_declaration "deinit" @name) @symbol
(subscript_declaration "subscript" @name) @symbol
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

/// A language with highlighting and no symbols.
fn highlight_only(language: Language, name: &str, highlights: &str) -> Lang {
    lang(language, name, highlights, "", &[], &[])
}

/// Languages by file extension, built once.
pub(super) static LANGS: LazyLock<HashMap<&'static str, Lang>> = LazyLock::new(|| {
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
        "swift",
        lang(
            tree_sitter_swift::LANGUAGE.into(),
            "swift",
            tree_sitter_swift::HIGHLIGHTS_QUERY,
            SWIFT_SYMBOLS,
            &["comment", "multiline_comment"],
            none,
        ),
    );
    m.insert(
        "json",
        highlight_only(
            tree_sitter_json::LANGUAGE.into(),
            "json",
            tree_sitter_json::HIGHLIGHTS_QUERY,
        ),
    );
    m.insert(
        "yaml",
        highlight_only(
            tree_sitter_yaml::LANGUAGE.into(),
            "yaml",
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
        ),
    );
    m.insert(
        "toml",
        highlight_only(
            tree_sitter_toml_ng::LANGUAGE.into(),
            "toml",
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
        ),
    );
    m.insert(
        "sh",
        highlight_only(
            tree_sitter_bash::LANGUAGE.into(),
            "bash",
            tree_sitter_bash::HIGHLIGHT_QUERY,
        ),
    );
    m
});

pub(super) fn lang_key(path: &str) -> Option<&'static str> {
    let ext = path.rsplit_once('.').map(|(_, e)| e)?;
    Some(match ext {
        "rs" => "rs",
        "py" | "pyi" => "py",
        "ts" | "mts" | "cts" => "ts",
        "tsx" => "tsx",
        "js" | "jsx" | "mjs" | "cjs" => "js",
        "go" => "go",
        "swift" => "swift",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" => "sh",
        _ => return None,
    })
}
