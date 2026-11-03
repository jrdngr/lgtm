use super::spec::check;

#[test]
fn h_and_l_stay_on_the_line() {
    check(&[
        ("a[b]c", "l", "ab[c]"),
        ("ab[c]\nd", "l", "ab[c]\nd"),
        ("a\n[b]", "h", "a\n[b]"),
        ("ab[c]", "hh", "[a]bc"),
    ]);
}

#[test]
fn j_and_k_keep_the_column_across_a_shorter_line() {
    check(&[
        ("abc[d]\nx\nabcdef", "j", "abcd\n[x]\nabcdef"),
        ("abc[d]\nx\nabcdef", "jj", "abcd\nx\nabc[d]ef"),
        ("abcd\nx\nabc[d]ef", "kk", "abc[d]\nx\nabcdef"),
        ("ab\n[c]d", "jj", "ab\n[c]d"),
    ]);
}

#[test]
fn insert_and_append_type_before_and_after_the_selection() {
    check(&[
        ("a[b]c", "iX<esc>", "a[X]bc"),
        ("a[b]c", "aX<esc>", "ab[X]c"),
        ("  a[b]c\nd", "IX<esc>", "  [X]abc\nd"),
        ("a[b]c\nd", "AX<esc>", "abc[X]\nd"),
        ("a\n[b]", "i<bs><esc>", "[a]b"),
        ("foo ba[r]", "a<C-w><esc>", "foo[ ]"),
    ]);
}

#[test]
fn new_lines_keep_the_indent() {
    check(&[
        ("    a[b]\nc", "ox<esc>", "    ab\n    [x]\nc"),
        ("a\n  [b]", "Ox<esc>", "a\n  [x]\n  b"),
        ("    a[b]", "a<ret>c<esc>", "    ab\n    [c]"),
    ]);
}

#[test]
fn undo_takes_back_a_whole_insert_and_redo_puts_it_back() {
    check(&[
        ("a[b]c", "iXY<esc>u", "a[b]c"),
        ("a[b]c", "iXY<esc>uU", "aX[Y]bc"),
        ("a[b]c", "i<esc>iX<esc>u", "a[b]c"),
    ]);
}
