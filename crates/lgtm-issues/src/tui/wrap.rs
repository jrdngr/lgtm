//! Styled text broken into screen lines.

use ratatui::style::Style;

/// A screen line: runs of text in their styles.
pub type Runs = Vec<(String, Style)>;

/// `runs` broken at spaces to `width` columns, lines after the first indented by `hang`.
pub fn wrap(runs: &[(String, Style)], width: usize, hang: usize) -> Vec<Runs> {
    let chars: Vec<(char, Style)> = runs
        .iter()
        .flat_map(|(t, s)| t.chars().map(move |c| (c, *s)))
        .collect();
    let width = width.max(hang + 8);
    let mut lines = Vec::new();
    let mut start = 0;
    let mut first = true;
    while start < chars.len() || first {
        let room = if first { width } else { width - hang };
        let mut end = (start + room).min(chars.len());
        if end < chars.len()
            && let Some(sp) = chars[start..end].iter().rposition(|(c, _)| *c == ' ')
            && sp > 0
        {
            end = start + sp;
        }
        let mut line: Runs = Vec::new();
        if !first {
            line.push((" ".repeat(hang), Style::default()));
        }
        for &(c, s) in &chars[start..end] {
            match line.last_mut() {
                Some((t, st)) if *st == s => t.push(c),
                _ => line.push((c.to_string(), s)),
            }
        }
        lines.push(line);
        start = end;
        while start < chars.len() && chars[start].0 == ' ' {
            start += 1;
        }
        first = false;
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(line: &Runs) -> String {
        line.iter().map(|(t, _)| t.as_str()).collect()
    }

    #[test]
    fn long_lines_break_at_spaces_under_a_hanging_indent() {
        let runs = vec![("- one two three four five".to_string(), Style::default())];
        let lines: Vec<String> = wrap(&runs, 14, 2).iter().map(text).collect();
        assert_eq!(lines, ["- one two", "  three four", "  five"]);
    }

    #[test]
    fn a_word_longer_than_the_line_is_cut() {
        let runs = vec![("abcdefghijklmnopqrstuvwxyz".to_string(), Style::default())];
        let lines: Vec<String> = wrap(&runs, 10, 0).iter().map(text).collect();
        assert_eq!(lines, ["abcdefghij", "klmnopqrst", "uvwxyz"]);
    }
}
