//! Markdown editing: lists and quotes that go on at Enter, ordered lists that keep their
//! numbers, checkboxes, and the smart key for tables and footnotes. Offsets are in chars.

/// Whether `name` is a markdown file.
pub fn is_markdown(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".md", ".markdown", ".mdx"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// Text that replaces chars `from..to`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub from: usize,
    pub to: usize,
    pub text: String,
}

/// Changes that don't overlap, in order, and where the cursor goes once they're made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub changes: Vec<Change>,
    pub cursor: usize,
}

#[cfg(test)]
impl Edit {
    /// `text` with the changes made.
    pub fn apply(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut out = String::new();
        let mut at = 0;
        for c in &self.changes {
            out.extend(&chars[at..c.from]);
            out.push_str(&c.text);
            at = c.to;
        }
        out.extend(&chars[at..]);
        out
    }
}

fn change(from: usize, to: usize, text: impl Into<String>) -> Change {
    Change {
        from,
        to,
        text: text.into(),
    }
}

/// The text's lines, without their newlines, and where each starts.
struct Lines {
    text: Vec<Vec<char>>,
    starts: Vec<usize>,
    len: usize,
}

impl Lines {
    fn new(text: &str) -> Self {
        let mut lines = Lines {
            text: vec![vec![]],
            starts: vec![0],
            len: 0,
        };
        for c in text.chars() {
            lines.len += 1;
            if c == '\n' {
                lines.text.push(vec![]);
                lines.starts.push(lines.len);
            } else if let Some(l) = lines.text.last_mut() {
                l.push(c);
            }
        }
        lines
    }

    fn of(&self, at: usize) -> usize {
        self.starts.partition_point(|&s| s <= at).saturating_sub(1)
    }

    fn get(&self, line: usize) -> String {
        self.text[line].iter().collect()
    }

    fn end(&self, line: usize) -> usize {
        self.starts[line] + self.text[line].len()
    }
}

fn len(s: &str) -> usize {
    s.chars().count()
}

/// A list item's marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Marker {
    Bullet(char),
    Number(u64, char),
    Letter(char, char),
}

impl Marker {
    fn next(self) -> Marker {
        match self {
            Marker::Bullet(_) => self,
            Marker::Number(n, d) => Marker::Number(n + 1, d),
            Marker::Letter(c, d) if c != 'z' && c != 'Z' => {
                Marker::Letter((c as u8 + 1) as char, d)
            }
            Marker::Letter(..) => self,
        }
    }

    /// Whether `other` is in the same ordered list as this.
    fn same_kind(self, other: Marker) -> bool {
        match (self, other) {
            (Marker::Number(_, a), Marker::Number(_, b)) => a == b,
            (Marker::Letter(x, a), Marker::Letter(y, b)) => {
                a == b && x.is_ascii_lowercase() == y.is_ascii_lowercase()
            }
            _ => false,
        }
    }

    fn text(self) -> String {
        match self {
            Marker::Bullet(c) => c.to_string(),
            Marker::Number(n, d) => format!("{n}{d}"),
            Marker::Letter(c, d) => format!("{c}{d}"),
        }
    }
}

/// A list item's line, split up.
struct Item {
    indent: String,
    marker: Marker,
    /// Where the marker ends.
    marker_end: usize,
    gap: String,
    /// The checkbox's mark, ' ' or 'x'.
    check: Option<char>,
    /// Where the item's own text starts.
    body: usize,
}

fn item(line: &str) -> Option<Item> {
    let chars: Vec<char> = line.chars().collect();
    let mut i = chars
        .iter()
        .take_while(|c| **c == ' ' || **c == '\t')
        .count();
    let indent: String = chars[..i].iter().collect();
    let marker = match chars.get(i)? {
        c @ ('-' | '*' | '+') => {
            i += 1;
            Marker::Bullet(*c)
        }
        c if c.is_ascii_digit() => {
            let digits: String = chars[i..]
                .iter()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if digits.len() > 9 {
                return None;
            }
            i += digits.len();
            let d = *chars.get(i).filter(|d| matches!(d, '.' | ')'))?;
            i += 1;
            Marker::Number(digits.parse().ok()?, d)
        }
        c if c.is_ascii_alphabetic() => {
            let d = *chars.get(i + 1).filter(|d| matches!(d, '.' | ')'))?;
            i += 2;
            Marker::Letter(*c, d)
        }
        _ => return None,
    };
    let marker_end = i;
    let gap_len = chars[i..].iter().take_while(|c| **c == ' ').count();
    if gap_len == 0 && i < chars.len() {
        return None;
    }
    let gap = " ".repeat(gap_len.max(1));
    i += gap_len;
    let mut check = None;
    if chars.get(i) == Some(&'[')
        && chars.get(i + 2) == Some(&']')
        && matches!(chars.get(i + 3), None | Some(' '))
        && let Some(&m @ (' ' | 'x' | 'X')) = chars.get(i + 1)
    {
        check = Some(if m == ' ' { ' ' } else { 'x' });
        i += 3;
        i += chars[i..].iter().take_while(|c| **c == ' ').count();
    }
    Some(Item {
        indent,
        marker,
        marker_end,
        gap,
        check,
        body: i,
    })
}

/// The `>` marks starting a quote's line, with their spaces.
fn quote_prefix(line: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = line;
    loop {
        let t = rest.trim_start_matches([' ', '\t']);
        let Some(after) = t.strip_prefix('>') else {
            break;
        };
        out.push_str(&rest[..rest.len() - t.len()]);
        out.push('>');
        rest = after;
        if let Some(after) = rest.strip_prefix(' ') {
            out.push(' ');
            rest = after;
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Enter at `at`: a list item or quote goes on to a new line with the same marker, the next
/// number or letter, and an empty checkbox where it had one. Enter on an empty one ends the
/// list or quote. Enter at the end of a table row starts a new row, and of a table's header,
/// its delimiter row first.
pub fn enter(text: &str, at: usize) -> Option<Edit> {
    let lines = Lines::new(text);
    let l = lines.of(at);
    let start = lines.starts[l];
    let col = at - start;
    let line = lines.get(l);
    if let Some(it) = item(&line).filter(|it| col >= it.body) {
        if line.chars().skip(it.body).all(char::is_whitespace) {
            return Some(Edit {
                changes: vec![change(start, lines.end(l), "")],
                cursor: start,
            });
        }
        let next = it.marker.next();
        let check = if it.check.is_some() { "[ ] " } else { "" };
        let prefix = format!("{}{}{}{check}", it.indent, next.text(), it.gap);
        let mut changes = vec![change(at, at, format!("\n{prefix}"))];
        changes.extend(renumber_after(&lines, l, &it.indent, next.next(), next));
        return Some(Edit {
            changes,
            cursor: at + 1 + len(&prefix),
        });
    }
    if let Some(prefix) = quote_prefix(&line).filter(|p| col >= len(p)) {
        if line.chars().skip(len(&prefix)).all(char::is_whitespace) {
            return Some(Edit {
                changes: vec![change(start, lines.end(l), "")],
                cursor: start,
            });
        }
        return Some(Edit {
            changes: vec![change(at, at, format!("\n{prefix}"))],
            cursor: at + 1 + len(&prefix),
        });
    }
    if is_row(&line) && col == line.chars().count() {
        let indent: String = line.chars().take_while(|c| *c == ' ').collect();
        let headed = l > 0 && is_row(&lines.get(l - 1));
        let delimited = l + 1 < lines.text.len() && is_delimiter(&lines.get(l + 1));
        let mut insert = String::from("\n");
        if !headed && !delimited {
            let cells = cells(&line);
            let row: Vec<String> = cells
                .iter()
                .map(|c| "-".repeat(width(c, true).max(3)))
                .collect();
            insert.push_str(&format!("{indent}| {} |\n", row.join(" | ")));
        }
        insert.push_str(&format!("{indent}| "));
        return Some(Edit {
            changes: vec![change(at, at, insert.clone())],
            cursor: at + len(&insert),
        });
    }
    None
}

/// Fixes the numbers of the items after line `l` in its ordered list, which has `indent`;
/// the first should be `expect`, and `first` is the marker before them.
fn renumber_after(
    lines: &Lines,
    l: usize,
    indent: &str,
    mut expect: Marker,
    first: Marker,
) -> Vec<Change> {
    let mut changes = vec![];
    if matches!(first, Marker::Bullet(_)) {
        return changes;
    }
    let deeper = |s: &str| {
        let ws: String = s.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        ws.len() > indent.len() && ws.starts_with(indent)
    };
    for n in l + 1..lines.text.len() {
        let line = lines.get(n);
        if line.trim().is_empty() || deeper(&line) {
            continue;
        }
        let Some(it) = item(&line).filter(|it| it.indent == indent && first.same_kind(it.marker))
        else {
            break;
        };
        if it.marker != expect {
            let at = lines.starts[n] + len(&it.indent);
            changes.push(change(at, lines.starts[n] + it.marker_end, expect.text()));
        }
        expect = expect.next();
    }
    changes
}

/// Fixes the numbers of the ordered list the line at `at` is in, from its first item.
pub fn renumber(text: &str, at: usize) -> Option<Edit> {
    let lines = Lines::new(text);
    let mut l = lines.of(at);
    // Up to the item the line belongs to.
    let mut it = loop {
        let line = lines.get(l);
        if let Some(it) = item(&line) {
            break it;
        }
        if l == 0 || !(line.trim().is_empty() || line.starts_with([' ', '\t'])) {
            return None;
        }
        l -= 1;
    };
    // Up to the list's first item.
    let mut n = l;
    while n > 0 {
        n -= 1;
        let line = lines.get(n);
        let ws: String = line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        if line.trim().is_empty() || (ws.len() > it.indent.len() && ws.starts_with(&it.indent)) {
            continue;
        }
        match item(&line) {
            Some(up) if up.indent == it.indent && up.marker.same_kind(it.marker) => {
                l = n;
                it = up;
            }
            _ => break,
        }
    }
    let changes = renumber_after(&lines, l, &it.indent.clone(), it.marker.next(), it.marker);
    (!changes.is_empty()).then_some(Edit {
        changes,
        cursor: at,
    })
}

/// Checks or unchecks the checkbox on `at`'s line, or gives a list item without one an
/// empty one.
pub fn toggle_check(text: &str, at: usize) -> Option<Edit> {
    let lines = Lines::new(text);
    let l = lines.of(at);
    let start = lines.starts[l];
    let it = item(&lines.get(l))?;
    let box_at = start + it.marker_end + len(&it.gap);
    let (c, cursor) = match it.check {
        Some(m) => {
            let mark = if m == ' ' { "x" } else { " " };
            (change(box_at + 1, box_at + 2, mark), at)
        }
        None => {
            let c = change(box_at, box_at, "[ ] ");
            let cursor = if at >= box_at { at + 4 } else { at };
            (c, cursor)
        }
    };
    Some(Edit {
        changes: vec![c],
        cursor,
    })
}

/// The smart key: on a footnote reference, goes to its note, writing one at the bottom
/// when there's none; on a note, goes back to its reference; after a bare `[^`, numbers a
/// new footnote. In a table, lines up the columns and moves to the next cell, adding it.
pub fn smart(text: &str, at: usize) -> Option<Edit> {
    footnote(text, at).or_else(|| table(text, at))
}

/// A footnote note's label, when `line` starts one.
fn note_label(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("[^")?;
    let (label, after) = rest.split_once(']')?;
    (after.starts_with(':') && !label.is_empty() && !label.contains(char::is_whitespace))
        .then_some(label)
}

/// The footnote references in `line`: their labels and char spans.
fn refs(line: &str) -> Vec<(String, usize, usize)> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = vec![];
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] == '['
            && chars[i + 1] == '^'
            && let Some(close) = chars[i + 2..].iter().position(|c| *c == ']')
        {
            let label: String = chars[i + 2..i + 2 + close].iter().collect();
            let end = i + 3 + close;
            if !label.is_empty()
                && !label.contains(char::is_whitespace)
                && chars.get(end) != Some(&':')
            {
                out.push((label, i, end));
            }
            i = end;
            continue;
        }
        i += 1;
    }
    out
}

fn footnote(text: &str, at: usize) -> Option<Edit> {
    let lines = Lines::new(text);
    let l = lines.of(at);
    let line = lines.get(l);
    let col = at - lines.starts[l];
    if let Some(label) = note_label(&line) {
        for n in 0..lines.text.len() {
            if let Some(&(_, _, end)) = refs(&lines.get(n)).iter().find(|r| r.0 == label) {
                return Some(Edit {
                    changes: vec![],
                    cursor: lines.starts[n] + end,
                });
            }
        }
        return None;
    }
    let (label, number) = match refs(&line)
        .into_iter()
        .find(|(_, from, to)| (*from..=*to).contains(&col))
    {
        Some((label, ..)) => (label, None),
        None => {
            let chars: Vec<char> = line.chars().collect();
            let bare = col >= 2 && chars[col - 2] == '[' && chars[col - 1] == '^';
            if !bare || chars.get(col) == Some(&']') {
                return None;
            }
            let next =
                (0..lines.text.len())
                    .flat_map(|n| refs(&lines.get(n)))
                    .filter_map(|(label, ..)| label.parse::<u64>().ok())
                    .chain((0..lines.text.len()).filter_map(|n| {
                        note_label(&lines.get(n)).and_then(|l| l.parse::<u64>().ok())
                    }))
                    .max()
                    .unwrap_or(0)
                    + 1;
            (next.to_string(), Some(next))
        }
    };
    for n in 0..lines.text.len() {
        let note = lines.get(n);
        if note_label(&note) == Some(label.as_str()) {
            let head = len(&label) + 4;
            let skip = note.chars().skip(head).take_while(|c| *c == ' ').count();
            return Some(Edit {
                changes: vec![],
                cursor: lines.starts[n] + head + skip,
            });
        }
    }
    // A new note at the bottom, after a blank line unless it follows other notes.
    let mut changes = vec![];
    let mut grew = 0;
    if let Some(n) = number {
        let fill = format!("{n}]");
        grew += len(&fill);
        changes.push(change(at, at, fill));
    }
    let ends_newline = text.ends_with('\n');
    let mut last = lines.text.len() - 1;
    if ends_newline && last > 0 {
        last -= 1;
    }
    let after_note = note_label(&lines.get(last)).is_some();
    let mut note = String::new();
    if !ends_newline && lines.len > 0 {
        note.push('\n');
    }
    if !after_note && lines.len > 0 && !lines.get(last).trim().is_empty() {
        note.push('\n');
    }
    note.push_str(&format!("[^{label}]: "));
    let cursor = lines.len + grew + len(&note);
    if ends_newline {
        note.push('\n');
    }
    changes.push(change(lines.len, lines.len, note));
    Some(Edit { changes, cursor })
}

fn is_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn is_delimiter(line: &str) -> bool {
    let t = line.trim();
    is_row(line) && t.contains('-') && t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

/// A row's cells, as written between its pipes. Escaped pipes stay in their cell.
fn cells(line: &str) -> Vec<String> {
    let t = line.trim_start();
    let t = t.strip_prefix('|').unwrap_or(t);
    let mut out = vec![String::new()];
    let mut escaped = false;
    for c in t.chars() {
        if c == '|' && !escaped {
            out.push(String::new());
        } else if let Some(cell) = out.last_mut() {
            cell.push(c);
        }
        escaped = c == '\\' && !escaped;
    }
    // A closing pipe leaves an empty cell after it.
    if out.len() > 1
        && out.last().is_some_and(|c| c.trim().is_empty())
        && t.trim_end().ends_with('|')
    {
        out.pop();
    }
    out
}

/// How wide a cell's text is. A header cell's spaces count, since they set its column's
/// width; one space each side is the pipe's padding.
fn width(cell: &str, header: bool) -> usize {
    let text = len(cell.trim());
    if !header {
        return text;
    }
    let mut w = len(cell);
    if cell.starts_with(' ') {
        w -= 1;
    }
    if cell.ends_with(' ') && w > 0 {
        w -= 1;
    }
    w.max(text)
}

/// A delimiter cell's alignment colons: left, right.
fn align(cell: &str) -> (bool, bool) {
    let t = cell.trim();
    (t.starts_with(':'), t.len() > 1 && t.ends_with(':'))
}

fn table(text: &str, at: usize) -> Option<Edit> {
    let lines = Lines::new(text);
    let l = lines.of(at);
    let line = lines.get(l);
    if !is_row(&line) {
        return None;
    }
    let col = at - lines.starts[l];
    let mut first = l;
    while first > 0 && is_row(&lines.get(first - 1)) {
        first -= 1;
    }
    let mut last = l;
    while last + 1 < lines.text.len() && is_row(&lines.get(last + 1)) {
        last += 1;
    }
    // The cell the cursor is in.
    let before: String = line.chars().take(col).collect();
    let cell = cells(&before).len() - 1;
    let delimited = first < last && is_delimiter(&lines.get(first + 1));
    if !delimited {
        let pad = if before.ends_with(' ') { "| " } else { " | " };
        return Some(Edit {
            changes: vec![change(at, at, pad)],
            cursor: at + len(pad),
        });
    }
    let indent: String = lines.get(first).chars().take_while(|c| *c == ' ').collect();
    let mut rows: Vec<Vec<String>> = (first..=last).map(|n| cells(&lines.get(n))).collect();
    let columns = rows[0].len();
    let row = l - first;
    if row == 1 {
        return None;
    }
    let at_end = cell + 1 >= columns.max(rows[row].len());
    if !at_end {
        while rows[row].len() < cell + 2 {
            rows[row].push(String::new());
        }
    }
    let count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..count)
        .map(|c| {
            rows.iter()
                .enumerate()
                .filter(|(r, _)| *r != 1)
                .filter_map(|(r, cells)| cells.get(c).map(|cell| width(cell, r == 0)))
                .max()
                .unwrap_or(0)
                .max(3)
        })
        .collect();
    let aligns: Vec<(bool, bool)> = (0..count)
        .map(|c| rows[1].get(c).map(|c| align(c)).unwrap_or((false, false)))
        .collect();
    let mut out = String::new();
    let mut cursor = None;
    for (r, cells) in rows.iter().enumerate() {
        if r > 0 {
            out.push('\n');
        }
        out.push_str(&indent);
        out.push('|');
        let n = if r == 1 {
            count.max(cells.len())
        } else {
            cells.len()
        };
        for c in 0..n {
            let w = widths[c];
            if r == 1 {
                let (left, right) = aligns[c];
                let dashes = w - usize::from(left) - usize::from(right);
                out.push(' ');
                if left {
                    out.push(':');
                }
                out.push_str(&"-".repeat(dashes));
                if right {
                    out.push(':');
                }
                out.push_str(" |");
                continue;
            }
            out.push(' ');
            let t = cells[c].trim();
            if r == row && c == cell + 1 {
                cursor = Some(len(&out));
                // The cell just started stays open to type into.
                if t.is_empty() && c + 1 == cells.len() {
                    break;
                }
            }
            out.push_str(t);
            out.push_str(&" ".repeat(w - len(t)));
            out.push_str(" |");
        }
        if r == row && cursor.is_none() {
            // The last cell: a new row to type into.
            out.push('\n');
            out.push_str(&indent);
            out.push_str("| ");
            cursor = Some(len(&out));
        }
    }
    let from = lines.starts[first];
    Some(Edit {
        changes: vec![change(from, lines.end(last), out)],
        cursor: from + cursor.unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs `f` on `text` with the cursor at `|`, and gives the result with `|` at the
    /// cursor.
    fn run(f: fn(&str, usize) -> Option<Edit>, text: &str) -> Option<String> {
        let at = text.chars().position(|c| c == '|').expect("a cursor");
        let text: String = text.chars().filter(|c| *c != '|').collect();
        let edit = f(&text, at)?;
        let mut out: Vec<char> = edit.apply(&text).chars().collect();
        out.insert(edit.cursor, '|');
        Some(out.into_iter().collect())
    }

    /// Like `run`, with `‸` as the cursor, for tables.
    fn run_t(f: fn(&str, usize) -> Option<Edit>, text: &str) -> Option<String> {
        let at = text.chars().position(|c| c == '‸').expect("a cursor");
        let text: String = text.chars().filter(|c| *c != '‸').collect();
        let edit = f(&text, at)?;
        let mut out: Vec<char> = edit.apply(&text).chars().collect();
        out.insert(edit.cursor, '‸');
        Some(out.into_iter().collect())
    }

    #[test]
    fn enter_goes_on_with_the_same_bullet_the_next_number_or_letter_and_an_empty_box() {
        let cases = [
            ("- a|", "- a\n- |"),
            ("  * a|", "  * a\n  * |"),
            ("+   a|", "+   a\n+   |"),
            ("9. a|", "9. a\n10. |"),
            ("1) a|", "1) a\n2) |"),
            ("a. x|", "a. x\nb. |"),
            ("C) x|", "C) x\nD) |"),
            ("- [x] done|", "- [x] done\n- [ ] |"),
            ("1. [ ] a|", "1. [ ] a\n2. [ ] |"),
            ("- ab|cd", "- ab\n- |cd"),
            ("> quote|", "> quote\n> |"),
            ("> > deep|", "> > deep\n> > |"),
        ];
        for (from, to) in cases {
            assert_eq!(run(enter, from).as_deref(), Some(to), "{from}");
        }
    }

    #[test]
    fn enter_on_an_empty_item_or_quote_ends_it_and_elsewhere_does_nothing() {
        assert_eq!(run(enter, "- a\n- |").as_deref(), Some("- a\n|"));
        assert_eq!(run(enter, "- a\n- [ ] |").as_deref(), Some("- a\n|"));
        assert_eq!(run(enter, "> a\n> |").as_deref(), Some("> a\n|"));
        assert_eq!(run(enter, "plain|"), None);
        // Before the marker's text, Enter is plain.
        assert_eq!(run(enter, "|- a"), None);
        assert_eq!(run(enter, "---|"), None);
    }

    #[test]
    fn enter_in_an_ordered_list_renumbers_the_items_after_it_but_not_nested_ones() {
        let from = "1. a|\n2. b\n   1. inner\n3. c\n\npara\n4. other";
        let to = "1. a\n2. |\n3. b\n   1. inner\n4. c\n\npara\n4. other";
        assert_eq!(run(enter, from).as_deref(), Some(to));
    }

    #[test]
    fn renumber_fixes_a_list_from_its_first_item_whichever_line_the_cursor_is_on() {
        let text = "x\n\n3. a\n5. b\n   more\n|9. c\n- d";
        assert_eq!(
            run(renumber, text).as_deref(),
            Some("x\n\n3. a\n4. b\n   more\n|5. c\n- d")
        );
        assert_eq!(run(renumber, "1. a\n2. |b"), None);
        assert_eq!(run(renumber, "a\n- |b\n- c"), None);
    }

    #[test]
    fn toggle_check_flips_a_box_and_gives_a_plain_item_one() {
        assert_eq!(run(toggle_check, "- [ ] a|").as_deref(), Some("- [x] a|"));
        assert_eq!(
            run(toggle_check, "  1. [X] |a").as_deref(),
            Some("  1. [ ] |a")
        );
        assert_eq!(run(toggle_check, "- a|").as_deref(), Some("- [ ] a|"));
        assert_eq!(run(toggle_check, "|- a").as_deref(), Some("|- [ ] a"));
        assert_eq!(run(toggle_check, "plain|"), None);
    }

    #[test]
    fn smart_on_a_reference_writes_its_note_at_the_bottom_or_goes_to_the_one_there() {
        assert_eq!(
            run(smart, "Body[^a]| text.\n").as_deref(),
            Some("Body[^a] text.\n\n[^a]: |\n")
        );
        assert_eq!(
            run(smart, "B[^|a] c[^b].\n\n[^b]: one\n").as_deref(),
            Some("B[^a] c[^b].\n\n[^b]: one\n[^a]: |\n")
        );
        assert_eq!(
            run(smart, "B[^a]|.\n\n[^a]: note\n").as_deref(),
            Some("B[^a].\n\n[^a]: |note\n")
        );
        assert_eq!(run(smart, "B[^a].|"), None);
    }

    #[test]
    fn smart_after_a_bare_caret_numbers_a_new_footnote() {
        assert_eq!(
            run(smart, "A[^1] b[^|\n\n[^1]: one\n").as_deref(),
            Some("A[^1] b[^2]\n\n[^1]: one\n[^2]: |\n")
        );
        assert_eq!(run(smart, "x[^|").as_deref(), Some("x[^1]\n\n[^1]: |"));
    }

    #[test]
    fn smart_on_a_note_goes_back_to_its_reference() {
        assert_eq!(
            run(smart, "See[^n] and [^m].\n\n[^m]: em|\n").as_deref(),
            Some("See[^n] and [^m]|.\n\n[^m]: em\n")
        );
    }

    #[test]
    fn smart_in_a_header_without_a_delimiter_starts_a_column() {
        assert_eq!(
            run_t(smart, "| Name      ‸").as_deref(),
            Some("| Name      | ‸")
        );
        assert_eq!(run_t(smart, "| Name‸").as_deref(), Some("| Name | ‸"));
    }

    #[test]
    fn smart_in_a_row_pads_the_cell_to_its_column_and_moves_to_the_next() {
        let from = "| Name      | Age |\n| --- | :-: |\n| Bob‸";
        let to = "| Name      | Age |\n| --------- | :-: |\n| Bob       | ‸";
        assert_eq!(run_t(smart, from).as_deref(), Some(to));
        let from = "| Name      | Age |\n| --- | --- |\n| Bob       | 4‸";
        let to = "| Name      | Age |\n| --------- | --- |\n| Bob       | 4   |\n| ‸";
        assert_eq!(run_t(smart, from).as_deref(), Some(to));
    }

    #[test]
    fn smart_widens_a_column_whose_cell_outgrows_the_header() {
        let from = "| A   | B |\n|---|--:|\n| longer‸";
        let to = "| A      | B   |\n| ------ | --: |\n| longer | ‸";
        assert_eq!(run_t(smart, from).as_deref(), Some(to));
    }

    #[test]
    fn enter_at_a_header_adds_its_delimiter_and_at_a_row_starts_another() {
        assert_eq!(
            run_t(enter, "| Name      | Age |‸").as_deref(),
            Some("| Name      | Age |\n| --------- | --- |\n| ‸")
        );
        assert_eq!(
            run_t(enter, "| a |\n| --- |\n| b |‸").as_deref(),
            Some("| a |\n| --- |\n| b |\n| ‸")
        );
    }
}
