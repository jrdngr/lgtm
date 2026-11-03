//! Word wrapping for notes in the diff.

/// `text`'s lines wrapped to `width` columns, breaking between words, or inside a word too long
/// for a line. An empty text is one empty line.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        let mut len = 0;
        for word in para.split(' ') {
            let mut word: Vec<char> = word.chars().collect();
            let gap = usize::from(len > 0);
            if len + gap + word.len() > width && len > 0 {
                out.push(std::mem::take(&mut line));
                len = 0;
            } else if gap == 1 {
                line.push(' ');
                len += 1;
            }
            while word.len() > width - len {
                let rest = word.split_off(width - len);
                line.extend(word);
                out.push(std::mem::take(&mut line));
                len = 0;
                word = rest;
            }
            len += word.len();
            line.extend(word);
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaks_between_words_and_inside_words_too_long_for_a_line() {
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("abcdefghij", 4), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a\n\nb", 5), ["a", "", "b"]);
        assert_eq!(wrap("", 5), [""]);
        assert_eq!(wrap("ab cdefgh", 4), ["ab", "cdef", "gh"]);
    }
}
