use std::sync::LazyLock;

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
    assert_eq!(LazyLock::force(&LANGS).len(), 19);
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
fn swift_symbols_cover_types_members_and_extensions() {
    let src = "/// Docs.\n@MainActor\nstruct S {\n    init() {}\n    func f() {}\n}\n\nprotocol P {\n    func g()\n}\n\nextension S {\n    deinit {}\n}\n";
    assert_eq!(
        symbols("a.swift", src),
        [
            ("S".into(), 1, 6),
            ("init".into(), 4, 4),
            ("f".into(), 5, 5),
            ("P".into(), 8, 10),
            ("g".into(), 9, 9),
            ("S".into(), 12, 14),
            ("deinit".into(), 13, 13),
        ]
    );
}

#[test]
fn markdown_sections_nest_and_run_to_the_next_heading_of_their_level() {
    let src = "# A\n\ntext\n\n## B\n\nmore\n\n# C\n";
    assert_eq!(
        symbols("a.md", src),
        [("A".into(), 1, 8), ("B".into(), 5, 8), ("C".into(), 9, 9)]
    );
}

#[test]
fn markdown_setext_heading_names_its_section_without_the_newline() {
    let src = "Title\n=====\n\ntext\n\n# B\n";
    assert_eq!(
        symbols("a.md", src),
        [("Title".into(), 1, 5), ("B".into(), 6, 6)]
    );
}

#[test]
fn rust_impls_are_named_by_their_base_type() {
    let src = "impl<T> a::Foo<T> {}\nimpl Bar<u8> {}\nimpl X for &Baz {}\nimpl dyn Any + Send {}\nimpl T for ! {}\n";
    let names: Vec<String> = symbols("a.rs", src).into_iter().map(|s| s.0).collect();
    assert_eq!(names, ["Foo", "Bar", "&Baz", "dyn Any + Send", "!"]);
}

#[test]
fn go_grouped_types_are_separate_symbols() {
    let src = "// Doc.\ntype S struct{}\n\ntype (\n\tA struct{}\n\t// B docs.\n\tB = int\n)\n";
    assert_eq!(
        symbols("a.go", src),
        [("S".into(), 1, 2), ("A".into(), 5, 5), ("B".into(), 6, 7)]
    );
}

#[test]
fn languages_match_extensions_and_fence_tags_in_any_case() {
    assert!(parse("README.MD", "# A\n").is_some());
    let src = "```Rust\nfn f() {}\n```\n";
    let keyword = Some(
        HIGHLIGHT_NAMES
            .iter()
            .position(|h| *h == "keyword")
            .unwrap() as u8,
    );
    assert!(
        highlight_file("a.md", src)[1]
            .iter()
            .any(|s| s.0 == keyword)
    );
}

#[test]
fn config_files_outline_their_nested_keys_and_tables() {
    let json = "{\n  \"name\": \"x\",\n  \"scripts\": {\n    \"a\": \"b\"\n  }\n}\n";
    assert_eq!(symbols("package.json", json), [("scripts".into(), 3, 5)]);
    let yaml = "name: x\njobs:\n  test:\n    runs-on: y\n";
    assert_eq!(
        symbols("ci.yml", yaml),
        [("jobs".into(), 2, 4), ("test".into(), 3, 4)]
    );
    let toml = "a = 1\n\n[deps]\nx = 1\n\n[[bin]]\nname = \"b\"\n";
    assert_eq!(
        symbols("Cargo.toml", toml),
        [("deps".into(), 3, 5), ("bin".into(), 6, 7)]
    );
    let syms = parse("a.toml", toml).unwrap().symbols(toml);
    assert!(syms.iter().all(|s| s.kind == SymbolKind::Section));
}

#[test]
fn shell_make_and_sql_symbols() {
    let sh = "# Builds.\nbuild() {\n  make\n}\n";
    assert_eq!(symbols("a.sh", sh), [("build".into(), 1, 4)]);
    let make = "all: build\n\nbuild:\n\tcargo build\n";
    let names: Vec<String> = symbols("Makefile", make).into_iter().map(|s| s.0).collect();
    assert_eq!(names, ["all", "build"]);
    let sql = "CREATE TABLE users (id int);\nCREATE VIEW names AS SELECT id FROM users;\n";
    let names: Vec<String> = symbols("a.sql", sql).into_iter().map(|s| s.0).collect();
    assert_eq!(names, ["users", "names"]);
}

#[test]
fn stylesheets_outline_rules_and_html_outlines_ids_and_scripts() {
    let css = ".a {\n  color: red;\n}\n@keyframes spin {\n  to { opacity: 1; }\n}\n";
    assert_eq!(
        symbols("a.css", css),
        [(".a".into(), 1, 3), ("spin".into(), 4, 6)]
    );
    let scss = "@mixin m {\n  a: b;\n}\n.x {\n  .y { c: d; }\n}\n";
    assert_eq!(
        symbols("a.scss", scss),
        [("m".into(), 1, 3), (".x".into(), 4, 6), (".y".into(), 5, 5)]
    );
    let html = "<div id=\"app\">\n  <p>hi</p>\n</div>\n<script>\nlet x = 1;\n</script>\n";
    assert_eq!(
        symbols("a.html", html),
        [("app".into(), 1, 3), ("script".into(), 4, 6)]
    );
}

#[test]
fn html_highlights_its_script_and_dockerfiles_are_found_by_name() {
    let lines = highlight_file("a.html", "<script>\nconst x = 1;\n</script>\n");
    let keyword = HIGHLIGHT_NAMES
        .iter()
        .position(|n| *n == "keyword")
        .unwrap() as u8;
    assert!(
        lines[1].iter().any(|s| s.0 == Some(keyword)),
        "{:?}",
        lines[1]
    );
    assert!(!highlight_file("app/Dockerfile", "FROM x\nRUN y\n")[0].is_empty());
    assert!(!highlight_file("Dockerfile.dev", "FROM x\n")[0].is_empty());
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
fn highlight_file_has_a_segment_list_per_line() {
    let lines = highlight_file("a.rs", "fn f() {}\n\nlet x = 1;\n");
    assert_eq!(lines.len(), 4);
    assert!(lines[1].is_empty() && lines[3].is_empty());
    assert!(highlight_file("notes.txt", "hi").is_empty());
}

#[test]
fn markdown_highlights_inline_text_and_fenced_code_by_language() {
    let src = "# Title\n\nSome `code` and **bold**.\n\n```rust\nfn f() {}\n```\n";
    let lines = highlight_file("a.md", src);
    let class = |n: &str| Some(HIGHLIGHT_NAMES.iter().position(|h| *h == n).unwrap() as u8);
    let has = |line: usize, n: &str| lines[line].iter().any(|s| s.0 == class(n));
    assert!(has(0, "tag"), "{:?}", lines[0]);
    assert!(has(2, "string") && has(2, "keyword"), "{:?}", lines[2]);
    assert!(has(5, "keyword"), "{:?}", lines[5]);
}

#[test]
fn terraform_highlights_blocks_attributes_and_references() {
    let src = "resource \"aws_s3_bucket\" \"logs\" {\n  bucket = var.name\n  count  = length(local.ids)\n}\n";
    let lines = highlight_file("main.tf", src);
    let class = |n: &str| Some(HIGHLIGHT_NAMES.iter().position(|h| *h == n).unwrap() as u8);
    let has = |line: usize, n: &str| lines[line].iter().any(|s| s.0 == class(n));
    assert!(has(0, "type") && has(0, "string"), "{:?}", lines[0]);
    assert!(has(1, "property") && has(1, "variable.builtin"), "{:?}", lines[1]);
    assert!(has(2, "function"), "{:?}", lines[2]);
    assert!(!highlight_file("code.terraform", "x = 1")[0].is_empty());
}

#[test]
fn rendered_fence_highlights_by_grammar_name() {
    assert!(!highlight_file("code.rust", "fn main() {}")[0].is_empty());
    assert!(!highlight_file("code.python", "def f(): pass")[0].is_empty());
}

#[test]
fn unknown_extension_has_no_parser() {
    assert!(parse("notes.txt", "hi").is_none());
}
