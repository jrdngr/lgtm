use std::fs;

use super::*;

fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, text) in files {
        let p = dir.path().join(path);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, text).unwrap();
    }
    dir
}

fn query(pattern: &str) -> TextQuery {
    TextQuery {
        pattern: pattern.into(),
        ..TextQuery::default()
    }
}

#[test]
fn text_search_honors_case_word_regex_and_globs() {
    let dir = tree(&[
        ("src/a.rs", "let Act = act();\nlet acting = 1;\n"),
        ("src/a.test.ts", "act()\n"),
        ("docs/b.md", "ACT now\n"),
        (".gitignore", "out/\n"),
        ("out/c.rs", "act\n"),
    ]);
    let r = dir.path();
    let lines = |q: &TextQuery| -> Vec<(String, u32)> {
        search(r, q)
            .unwrap()
            .hits
            .into_iter()
            .map(|h| (h.path, h.line))
            .collect()
    };
    let p = |s: &str, l: u32| (s.to_string(), l);

    assert_eq!(
        lines(&query("act")),
        [p("docs/b.md", 1), p("src/a.rs", 1), p("src/a.rs", 2), p("src/a.test.ts", 1)]
    );
    let exact = TextQuery { case_sensitive: true, whole_word: true, ..query("act") };
    assert_eq!(lines(&exact), [p("src/a.rs", 1), p("src/a.test.ts", 1)]);
    let re = TextQuery { regex: true, ..query(r"act\w+") };
    assert_eq!(lines(&re), [p("src/a.rs", 2)]);
    let globbed = TextQuery { include: "src".into(), exclude: "*.test.ts".into(), ..query("act") };
    assert_eq!(lines(&globbed), [p("src/a.rs", 1), p("src/a.rs", 2)]);
    let only = TextQuery { paths: Some(vec!["docs/b.md".into()]), ..query("act") };
    assert_eq!(lines(&only), [p("docs/b.md", 1)]);
}

#[test]
fn text_hits_span_each_match_in_utf16() {
    let dir = tree(&[("a.txt", "é act, act\n")]);
    let hit = &search(dir.path(), &query("act")).unwrap().hits[0];
    assert_eq!(hit.spans, [(2, 5), (7, 10)]);
}

#[test]
fn replace_rewrites_matches_with_regex_groups_or_literally() {
    let dir = tree(&[("a.rs", "foo(1); foo(2);\n"), ("b.rs", "bar\n")]);
    let r = dir.path();
    let re = TextQuery { regex: true, ..query(r"foo\((\d)\)") };
    assert_eq!(replace(r, &re, "baz($1)").unwrap(), 1);
    assert_eq!(fs::read_to_string(r.join("a.rs")).unwrap(), "baz(1); baz(2);\n");
    assert_eq!(replace(r, &query("bar"), "$x").unwrap(), 1);
    assert_eq!(fs::read_to_string(r.join("b.rs")).unwrap(), "$x\n");
}

#[test]
fn references_are_identifiers_only_with_the_definition_first() {
    let dir = tree(&[
        ("b.rs", "fn caller() {\n    act();\n    // act in a comment\n}\n"),
        ("a.rs", "/// Docs.\nfn act() {}\nfn other() { let acting = 1; }\n"),
    ]);
    let hits = references(dir.path(), "act").hits;
    let got: Vec<_> = hits.iter().map(|h| (h.path.as_str(), h.line, h.def)).collect();
    assert_eq!(got, [("a.rs", 2, true), ("b.rs", 2, false)]);
    assert_eq!(hits[1].spans, [(4, 7)]);
}

#[test]
fn symbols_match_as_subsequences_exact_and_prefix_first() {
    let dir = tree(&[("a.rs", "fn near() {}\nfn nearest_bookmarks() {}\nfn unclear() {}\n")]);
    let names: Vec<_> = symbols(dir.path(), "near", 10).into_iter().map(|s| s.name).collect();
    assert_eq!(names, ["near", "nearest_bookmarks", "unclear"]);
}
