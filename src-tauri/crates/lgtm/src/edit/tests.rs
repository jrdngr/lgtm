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

#[test]
fn word_motions_select_what_they_move_over() {
    check(&[
        ("let [a]rgv = 1", "w", "let [argv ]= 1"),
        ("x = [1]\nfoo", "w", "x = 1\n[foo]"),
        ("[a].b c", "w", "a[.]b c"),
        ("[a].b c", "W", "[a.b ]c"),
        ("[l]et argv", "e", "[let] argv"),
        ("le[t] argv", "e", "let[ argv]"),
        ("let ar[g]v", "b", "let [<arg]v"),
        ("a.b [c]", "B", "[<a.b ]c"),
    ]);
}

#[test]
fn find_and_till_stop_on_or_next_to_the_char_on_the_same_line() {
    check(&[
        ("[a]bcabc", "fc", "[abc]abc"),
        ("[a]bcabc", "tc", "[ab]cabc"),
        ("abcab[c]", "Fa", "abc[<abc]"),
        ("abcab[c]", "Ta", "abca[<bc]"),
        ("[a]bc", "fz", "[a]bc"),
        ("[a]b\nc", "fc", "[a]b\nc"),
    ]);
}

#[test]
fn goto_keys_go_to_file_and_line_ends() {
    check(&[
        ("ab\n  c[d]", "gg", "[a]b\n  cd"),
        ("a[b]\ncd", "ge", "ab\n[c]d"),
        ("  a[b]c", "gh", "[ ] abc"),
        ("[a]bc\nd", "gl", "ab[c]\nd"),
        ("  ab[c]", "gs", "  [a]bc"),
    ]);
}

#[test]
fn x_selects_lines_and_extends_a_line_at_a_time() {
    check(&[
        ("a[b]\ncd\ne", "x", "[ab\n]cd\ne"),
        ("a[b]\ncd\ne", "xx", "[ab\ncd\n]e"),
        ("ab\nc[d]", "x", "ab\n[cd]"),
        ("a[b]c", "%", "[abc]"),
    ]);
}

#[test]
fn select_mode_extends_and_ends_after_an_edit() {
    check(&[
        ("[a]bc", "vll", "[abc]"),
        ("[a]bc", "vll;", "ab[c]"),
        ("[a]b cd", "vww", "[ab cd]"),
        ("[a]bc", "vld", "[c]"),
        ("[a]bcd", "vldl", "c[d]"),
    ]);
}

#[test]
fn delete_change_yank_and_paste() {
    check(&[
        ("a[b]c", "d", "a[c]"),
        ("[ab\n]cd", "d", "[c]d"),
        ("let [a]rgv = 1", "wcx<esc>", "let [x]= 1"),
        ("[a]b", "yp", "a[a]b"),
        ("[a]b\ncd", "xyp", "ab\n[ab\n]cd"),
        ("[a]b\ncd", "xyP", "[ab\n]ab\ncd"),
        ("[a]b\ncd", "xcz<esc>", "[z]\ncd"),
        ("a[b]c", "du", "a[b]c"),
    ]);
}

#[test]
fn replace_case_indent_and_join() {
    check(&[
        ("[ab]\nc", "r-", "[--]\nc"),
        ("[aB]c", "~", "[Ab]c"),
        ("[a]b\ncd", ">", "    [a]b\ncd"),
        ("    [a]b", "<lt>", "[a]b"),
        ("\tx\n[a]", ">", "\tx\n\t[a]"),
        ("[ab\ncd]", ">", "    [ab\n    cd]"),
        ("[a]b\n   cd\ne", "J", "ab[ ]cd\ne"),
        ("[a]b\n\ncd", "J", "ab[\n]cd"),
    ]);
}

#[test]
fn surround_adds_deletes_and_replaces_the_nearest_pair() {
    check(&[
        ("x[ab]y", "ms(", "x[(ab)]y"),
        ("x[ab]y", "ms)", "x[(ab)]y"),
        ("(a[b])", "md(", "a[b]"),
        ("((a)[b])", "md(", "(a)[b]"),
        ("\"a[b]\"", "md\"", "a[b]"),
        ("(a[b])", "mr({", "{a[b]}"),
        ("a[b]", "md(", "a[b]"),
    ]);
}

#[test]
fn star_searches_the_selection_and_n_wraps_around() {
    check(&[
        ("[f]oo bar foo", "*n", "foo bar [f]oo"),
        ("foo bar [f]oo", "*n", "[f]oo bar foo"),
        ("foo bar [f]oo", "*N", "[f]oo bar foo"),
        ("[F]oo foo Foo", "*n", "Foo foo [F]oo"),
        ("[f]oo Foo", "*n", "foo [F]oo"),
    ]);
}
