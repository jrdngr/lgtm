//! The languages syntax knows: their grammars, highlight queries, and symbol queries.

use std::collections::HashMap;
use std::sync::LazyLock;

use tree_sitter::{Language, Query};
use tree_sitter_highlight::HighlightConfiguration;

use super::HIGHLIGHT_NAMES;
use super::queries::*;

pub(super) struct Lang {
    pub(super) language: Language,
    pub(super) highlight: HighlightConfiguration,
    pub(super) symbols: Query,
    /// Preceding sibling kinds that belong to the symbol after them.
    pub(super) leading: &'static [&'static str],
    /// Parent kinds that wrap a symbol together with its decorators.
    pub(super) wrappers: &'static [&'static str],
    /// Symbols are an outline (sections, keys, rules), not code, so they are all sections and
    /// the graph never links uses to them.
    pub(super) outline: bool,
}

impl Lang {
    /// A language with highlighting and no symbols.
    fn new(language: Language, name: &str, highlights: &str, injections: &str) -> Lang {
        let mut highlight =
            HighlightConfiguration::new(language.clone(), name, highlights, injections, "")
                .unwrap_or_else(|e| panic!("{name} highlights query: {e}"));
        highlight.configure(HIGHLIGHT_NAMES);
        Lang {
            symbols: Query::new(&language, "").expect("empty query"),
            language,
            highlight,
            leading: &[],
            wrappers: &[],
            outline: false,
        }
    }

    fn symbols(mut self, query: &str) -> Lang {
        self.symbols = Query::new(&self.language, query).unwrap_or_else(|e| {
            let name = &self.highlight.language_name;
            panic!("{name} symbols query: {e}")
        });
        self
    }

    fn outline(self, query: &str) -> Lang {
        Lang {
            outline: true,
            ..self.symbols(query)
        }
    }

    fn leading(self, leading: &'static [&'static str]) -> Lang {
        Lang { leading, ..self }
    }

    fn wrappers(self, wrappers: &'static [&'static str]) -> Lang {
        Lang { wrappers, ..self }
    }
}

unsafe extern "C" {
    fn tree_sitter_gorp() -> *const ();
}

/// Gorp's grammar, compiled from `grammars/gorp` by the build script.
// SAFETY: `tree_sitter_gorp` is the generated parser's entry point and returns its static
// `TSLanguage`.
const GORP: tree_sitter_language::LanguageFn =
    unsafe { tree_sitter_language::LanguageFn::from_raw(tree_sitter_gorp) };

/// Markdown's inline grammar, which only runs injected into the block grammar.
static MD_INLINE: LazyLock<HighlightConfiguration> = LazyLock::new(|| {
    Lang::new(
        tree_sitter_md::INLINE_LANGUAGE.into(),
        "markdown_inline",
        MD_INLINE_HIGHLIGHTS,
        "",
    )
    .highlight
});

/// The highlighting for a language injected into another by name, like a fenced code
/// block's `rust` or `ts`.
pub(super) fn injected(name: &str) -> Option<&'static HighlightConfiguration> {
    if name == "markdown_inline" {
        return Some(&MD_INLINE);
    }
    lang_named(name).map(|l| &l.highlight)
}

/// The language a name stands for, in any case, as an extension (`rs`) or a grammar name
/// (`rust`).
pub(super) fn lang_named(name: &str) -> Option<&'static Lang> {
    let name = name.to_ascii_lowercase();
    let by_ext = lang_key(&format!("x.{name}")).and_then(|k| LANGS.get(k));
    by_ext.or_else(|| {
        LANGS
            .values()
            .find(|l| l.highlight.language_name == name.as_str())
    })
}

/// Languages by key, built once.
pub(super) static LANGS: LazyLock<HashMap<&'static str, Lang>> = LazyLock::new(|| {
    let js_highlights = tree_sitter_javascript::HIGHLIGHT_QUERY;
    let ts_highlights = [js_highlights, tree_sitter_typescript::HIGHLIGHTS_QUERY].concat();
    let jsx_highlights = tree_sitter_javascript::JSX_HIGHLIGHT_QUERY;
    let ts_symbols = [JS_SYMBOLS, TS_SYMBOLS].concat();
    let js_leading: &[&str] = &["decorator", "comment"];
    let js_wrappers: &[&str] = &["export_statement"];

    HashMap::from([
        (
            "rs",
            Lang::new(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                tree_sitter_rust::HIGHLIGHTS_QUERY,
                "",
            )
            .symbols(RUST_SYMBOLS)
            .leading(&["attribute_item", "line_comment", "block_comment"]),
        ),
        (
            "py",
            Lang::new(
                tree_sitter_python::LANGUAGE.into(),
                "python",
                tree_sitter_python::HIGHLIGHTS_QUERY,
                "",
            )
            .symbols(PYTHON_SYMBOLS)
            .leading(&["comment"])
            .wrappers(&["decorated_definition"]),
        ),
        (
            "ts",
            Lang::new(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                "typescript",
                &ts_highlights,
                "",
            )
            .symbols(&ts_symbols)
            .leading(js_leading)
            .wrappers(js_wrappers),
        ),
        (
            "tsx",
            Lang::new(
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                "tsx",
                &[ts_highlights.as_str(), jsx_highlights].concat(),
                "",
            )
            .symbols(&ts_symbols)
            .leading(js_leading)
            .wrappers(js_wrappers),
        ),
        (
            "js",
            Lang::new(
                tree_sitter_javascript::LANGUAGE.into(),
                "javascript",
                &[js_highlights, jsx_highlights].concat(),
                "",
            )
            .symbols(JS_SYMBOLS)
            .leading(js_leading)
            .wrappers(js_wrappers),
        ),
        (
            "go",
            Lang::new(
                tree_sitter_go::LANGUAGE.into(),
                "go",
                tree_sitter_go::HIGHLIGHTS_QUERY,
                "",
            )
            .symbols(GO_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "swift",
            Lang::new(
                tree_sitter_swift::LANGUAGE.into(),
                "swift",
                tree_sitter_swift::HIGHLIGHTS_QUERY,
                "",
            )
            .symbols(SWIFT_SYMBOLS)
            .leading(&["comment", "multiline_comment"]),
        ),
        (
            "sh",
            Lang::new(
                tree_sitter_bash::LANGUAGE.into(),
                "bash",
                tree_sitter_bash::HIGHLIGHT_QUERY,
                "",
            )
            .symbols(SHELL_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "md",
            Lang::new(
                tree_sitter_md::LANGUAGE.into(),
                "markdown",
                MD_BLOCK_HIGHLIGHTS,
                MD_INJECTIONS,
            )
            .outline(MD_SYMBOLS),
        ),
        (
            "json",
            Lang::new(
                tree_sitter_json::LANGUAGE.into(),
                "json",
                tree_sitter_json::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(JSON_SYMBOLS),
        ),
        (
            "yaml",
            Lang::new(
                tree_sitter_yaml::LANGUAGE.into(),
                "yaml",
                tree_sitter_yaml::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(YAML_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "toml",
            Lang::new(
                tree_sitter_toml_ng::LANGUAGE.into(),
                "toml",
                tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(TOML_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "css",
            Lang::new(
                tree_sitter_css::LANGUAGE.into(),
                "css",
                tree_sitter_css::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(CSS_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "scss",
            Lang::new(
                tree_sitter_scss::language(),
                "scss",
                tree_sitter_scss::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(SCSS_SYMBOLS)
            .leading(&["comment", "single_line_comment"]),
        ),
        (
            "html",
            Lang::new(
                tree_sitter_html::LANGUAGE.into(),
                "html",
                tree_sitter_html::HIGHLIGHTS_QUERY,
                tree_sitter_html::INJECTIONS_QUERY,
            )
            .outline(HTML_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "sql",
            Lang::new(
                tree_sitter_sequel::LANGUAGE.into(),
                "sql",
                tree_sitter_sequel::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(SQL_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "make",
            Lang::new(
                tree_sitter_make::LANGUAGE.into(),
                "make",
                tree_sitter_make::HIGHLIGHTS_QUERY,
                "",
            )
            .outline(MAKE_SYMBOLS)
            .leading(&["comment"]),
        ),
        (
            "hcl",
            Lang::new(tree_sitter_hcl::LANGUAGE.into(), "hcl", HCL_HIGHLIGHTS, ""),
        ),
        (
            "gorp",
            Lang::new(GORP.into(), "gorp", GORP_HIGHLIGHTS, "")
                .symbols(GORP_SYMBOLS)
                .leading(&["comment"]),
        ),
        (
            "dockerfile",
            Lang::new(
                tree_sitter_containerfile::LANGUAGE.into(),
                "dockerfile",
                tree_sitter_containerfile::HIGHLIGHTS_QUERY,
                tree_sitter_containerfile::INJECTIONS_QUERY,
            ),
        ),
    ])
});

/// The language of `path`, by extension or, for files like `Makefile`, by name.
pub(super) fn lang_key(path: &str) -> Option<&'static str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name {
        "Makefile" | "makefile" | "GNUmakefile" => return Some("make"),
        "Dockerfile" | "Containerfile" => return Some("dockerfile"),
        _ if name.starts_with("Dockerfile.") || name.starts_with("Containerfile.") => {
            return Some("dockerfile");
        }
        _ if name.ends_with(".gorp.lock") => return Some("gorp"),
        _ => {}
    }
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match ext.as_str() {
        "rs" => "rs",
        "py" | "pyi" => "py",
        "ts" | "mts" | "cts" => "ts",
        "tsx" => "tsx",
        "js" | "jsx" | "mjs" | "cjs" => "js",
        "go" => "go",
        "swift" => "swift",
        "json" | "jsonc" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "sh" | "bash" | "zsh" | "shell" => "sh",
        "md" | "markdown" => "md",
        "css" => "css",
        "scss" => "scss",
        "html" | "htm" => "html",
        "sql" => "sql",
        "mk" => "make",
        "hcl" | "tf" | "tfvars" | "terraform" => "hcl",
        "dockerfile" | "containerfile" => "dockerfile",
        "gorp" => "gorp",
        _ => return None,
    })
}
