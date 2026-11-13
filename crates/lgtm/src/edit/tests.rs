use super::fold;
use super::keys::{Keys, View};
use super::spec::{buffer_named, check, keys, marked};

/// Key cases for the modal editor, by group: start, keys, expected.
const KEY_CASES: &str = include_str!("key_cases.json");

#[test]
fn key_cases() {
    let groups: std::collections::BTreeMap<String, Vec<(String, String, String)>> =
        serde_json::from_str(KEY_CASES).expect("key cases parse");
    for cases in groups.values() {
        let cases: Vec<(&str, &str, &str)> = cases
            .iter()
            .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
            .collect();
        check(&cases);
    }
}

fn run_rs(start: &str, spec: &str) -> String {
    super::spec::run_in(buffer_named(start, "t.rs"), spec)
}

#[test]
fn mif_and_maf_select_the_function_around_the_cursor() {
    let src = "fn a() {\n    let [x] = 1;\n}\n";
    assert_eq!(run_rs(src, "mif"), "fn a() {[\n    let x = 1;\n]}\n");
    assert_eq!(run_rs(src, "maf"), "[fn a() {\n    let x = 1;\n}]\n");
}

#[test]
fn alt_o_grows_to_syntax_nodes_and_alt_i_steps_back() {
    let src = "fn a() { f([x] + 1); }";
    assert_eq!(run_rs(src, "<A-o>"), "fn a() { f([x + 1]); }");
    assert_eq!(run_rs(src, "<A-o><A-o>"), "fn a() { f[(x + 1)]; }");
    assert_eq!(run_rs(src, "<A-o><A-o><A-i>"), "fn a() { f([x + 1]); }");
    assert_eq!(run_rs(src, "<A-o><A-o><A-i><A-i>"), src);
}

fn spans(name: &str, text: &str) -> Vec<(usize, usize)> {
    let b = buffer_named(text, name);
    fold::ranges(&b).iter().map(|r| (r.start, r.end)).collect()
}

#[test]
fn rust_folds_cover_items_bodies_and_use_runs() {
    let src = "use a;\nuse b;\n\nfn f() {\n    if x {\n        y();\n    }\n}\n";
    assert_eq!(spans("t.rs", src), vec![(0, 1), (3, 7), (4, 6)]);
}

#[test]
fn typescript_and_markdown_fold_by_syntax() {
    let ts = "import a from \"a\";\nimport b from \"b\";\nfunction f() {\n  return 1;\n}\n";
    assert_eq!(spans("t.ts", ts), vec![(0, 1), (2, 4)]);
    let md = "# A\ntext\n\n## B\nmore\n";
    let md_spans = spans("t.md", md);
    assert!(md_spans.contains(&(0, 4)), "{md_spans:?}");
    assert!(md_spans.contains(&(3, 4)), "{md_spans:?}");
}

#[test]
fn files_without_a_grammar_fold_by_indent() {
    let txt = "a\n  b\n\n  c\n    d\ne\n";
    assert_eq!(spans("t.txt", txt), vec![(0, 4), (3, 4)]);
}

#[test]
fn folds_toggle_hide_lines_and_reveal_the_cursor() {
    let b = buffer_named("a\n  b\n  c\n    d\ne\n", "t.txt");
    let mut f = fold::Folds::default();
    f.update(&b);
    f.apply(fold::Fold::Toggle, 0);
    assert_eq!(f.visible(b.last_line()), vec![0, 4]);
    f.apply(fold::Fold::Toggle, 0);
    assert_eq!(f.visible(b.last_line()), vec![0, 1, 2, 3, 4]);
    f.apply(fold::Fold::CloseAll, 0);
    assert_eq!(f.hidden(), vec![(0, 3)]);
    f.reveal(3);
    assert_eq!(f.visible(b.last_line()), vec![0, 1, 2, 3, 4]);
}

#[test]
fn j_steps_over_a_closed_fold() {
    let mut b = buffer_named("[a]\n  b\n  c\nd", "t.txt");
    let mut k = Keys::default();
    for key in keys("j") {
        k.press(
            &mut b,
            key,
            View {
                height: 10,
                hidden: &[(0, 2)],
            },
        );
    }
    assert_eq!(marked(&b, k.mode), "a\n  b\n  c\n[d]");
}

/// Presses `spec` on a buffer named `name` holding `text` with the cursor at `at`, and gives
/// its text and cursor.
fn md(name: &str, text: &str, at: usize, spec: &str) -> (String, usize) {
    use super::buffer::{Buffer, Sel};
    use super::keys::{Keys, View};
    let mut b = Buffer::with_text(std::path::Path::new(name), name, text);
    b.sel = Sel::point(at);
    let mut k = Keys::default();
    let view = View {
        height: 10,
        hidden: &[],
    };
    for key in keys(spec) {
        k.press(&mut b, key, view);
    }
    (b.text.to_string(), b.sel.head)
}

#[test]
fn markdown_enter_continues_lists_only_in_markdown_files() {
    assert_eq!(md("a.md", "1. a\n", 0, "Ab<ret>c<esc>").0, "1. ab\n2. c\n");
    assert_eq!(md("a.txt", "1. a\n", 0, "Ab<ret>c<esc>").0, "1. ab\nc\n");
}

#[test]
fn markdown_space_x_toggles_a_checkbox_and_u_takes_it_back() {
    assert_eq!(md("a.md", "- [ ] a\n", 6, "<space>x").0, "- [x] a\n");
    assert_eq!(md("a.md", "- [ ] a\n", 6, "<space>xu").0, "- [ ] a\n");
    assert_eq!(md("a.txt", "- [ ] a\n", 6, "<space>x").0, "- [ ] a\n");
}

#[test]
fn markdown_shift_tab_writes_a_footnote_and_goes_back_to_its_reference() {
    let (text, at) = md("a.md", "See[^1].\n", 4, "<S-tab>");
    assert_eq!(text, "See[^1].\n\n[^1]: \n");
    assert_eq!(at, 16);
    let (text, at) = md("a.md", "See[^1].\n", 4, "<S-tab>inote<esc><S-tab>");
    assert_eq!(text, "See[^1].\n\n[^1]: note\n");
    assert_eq!(at, 7);
}

#[test]
fn markdown_deleting_an_item_renumbers_the_list() {
    assert_eq!(md("a.md", "1. a\n2. b\n3. c\n", 5, "xd").0, "1. a\n2. c\n");
}
