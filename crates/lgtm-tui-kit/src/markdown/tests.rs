use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::style::Modifier;

use super::render::render;
use super::*;
use crate::theme::PANEL;

fn texts(md: &str, width: usize) -> Vec<String> {
    render(md, width)
        .iter()
        .map(|l| l.text().trim_end().to_string())
        .collect()
}

#[test]
fn headings_lose_their_hashes_and_the_top_two_levels_get_a_rule() {
    let lines = render("# Title\n\n## Part\n\n### Small\n", 10);
    let t: Vec<String> = lines.iter().map(|l| l.text()).collect();
    assert_eq!(
        t,
        ["Title", "━━━━━━━━━━", "", "Part", "──────────", "", "Small"]
    );
    assert!(lines[0].runs[0].style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn paragraphs_wrap_at_words_and_keep_inline_styles() {
    let lines = render("one **two** three four\nfive", 14);
    assert_eq!(
        lines.iter().map(|l| l.text()).collect::<Vec<_>>(),
        ["one two three", "four five"]
    );
    let two = lines[0]
        .runs
        .iter()
        .find(|r| r.text.contains("two"))
        .unwrap();
    assert!(two.style.add_modifier.contains(Modifier::BOLD));
    let one = &lines[0].runs[0];
    assert!(!one.style.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn lists_get_bullets_numbers_and_checkboxes_and_wrapped_items_line_up() {
    let md = "- a long item here\n  - inner\n\n1. first\n2. second\n\n- [ ] todo\n- [x] done\n";
    assert_eq!(
        texts(md, 12),
        [
            "• a long",
            "  item here",
            "  ◦ inner",
            "",
            "1. first",
            "2. second",
            "",
            "☐ todo",
            "☑ done",
        ]
    );
}

#[test]
fn code_blocks_fill_their_lines_and_map_each_to_its_source_line() {
    let lines = render("text\n\n```rust\nfn a() {}\nlet b = 1;\n```\n", 20);
    let code: Vec<_> = lines.iter().filter(|l| l.fill == Some(PANEL)).collect();
    assert_eq!(
        code.iter().map(|l| l.text()).collect::<Vec<_>>(),
        [" fn a() {}", " let b = 1;"]
    );
    assert_eq!(code.iter().map(|l| l.src).collect::<Vec<_>>(), [3, 4]);
}

#[test]
fn quotes_keep_their_bar_on_every_line_between_paragraphs() {
    assert_eq!(texts("> one\n>\n> two\n", 20), ["▌ one", "▌", "▌ two"]);
}

#[test]
fn tables_line_up_their_columns_and_cut_cells_that_cannot_fit() {
    let md = "| name | n |\n|---|--:|\n| alpha | 1 |\n| b | 22 |\n";
    assert_eq!(
        texts(md, 40),
        [
            " name  │   n",
            "───────┼─────",
            " alpha │   1",
            " b     │  22",
        ]
    );
    // The bar and spaces take five of eleven columns, which leaves three for each.
    assert_eq!(texts(md, 11)[2], " al… │   1");
}

#[test]
fn links_are_found_where_they_were_drawn() {
    use ratatui::buffer::Buffer;
    let md = "see [docs](https://x.dev) now";
    let area = Rect::new(0, 0, 40, 3);
    let mut screen = Buffer::empty(area);
    let mut p = Preview::new(0);
    p.draw(&mut Canvas::new(&mut screen, area), area, md, 1);
    // Two columns of margin, then "see ".
    assert_eq!(p.link_at(6, 0), Some("https://x.dev"));
    assert_eq!(p.link_at(5, 0), None);
    assert_eq!(p.link_at(10, 0), None);
}

#[test]
fn it_opens_at_the_source_line_given_and_tells_the_line_at_its_top() {
    use ratatui::buffer::Buffer;
    let md: String = (0..30).map(|i| format!("para {i}\n\n")).collect();
    let area = Rect::new(0, 0, 40, 5);
    let mut screen = Buffer::empty(area);
    let mut p = Preview::new(20);
    p.draw(&mut Canvas::new(&mut screen, area), area, &md, 1);
    assert_eq!(p.src_line(), 20);
    assert_eq!(p.key(KeyEvent::from(KeyCode::Char('j'))), Outcome::None);
    assert_eq!(p.src_line(), 20, "the blank row after para 10 maps to it");
    p.key(KeyEvent::from(KeyCode::Char('j')));
    assert_eq!(p.src_line(), 22);
    p.key(KeyEvent::from(KeyCode::Char('g')));
    p.key(KeyEvent::from(KeyCode::Char('e')));
    assert_eq!(p.top, p.lines.len() - 5);
    p.key(KeyEvent::from(KeyCode::Char(' ')));
    assert_eq!(p.key(KeyEvent::from(KeyCode::Char('m'))), Outcome::Close);
}
