//! Where the cursor goes in a line of text: by character, and by word.

use unicode_segmentation::UnicodeSegmentation;

/// The start of the character before `at`.
pub fn prev_char(text: &str, at: usize) -> usize {
    text.grapheme_indices(true)
        .rev()
        .find_map(|(i, _)| (i < at).then_some(i))
        .unwrap_or(0)
}

/// The end of the character at `at`.
pub fn next_char(text: &str, at: usize) -> usize {
    text.grapheme_indices(true)
        .find_map(|(i, _)| (i > at).then_some(i))
        .unwrap_or(text.len())
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The start of the word before `at`, past any gap before it.
pub fn prev_word(text: &str, at: usize) -> usize {
    let mut chars = text[..at].char_indices().rev().peekable();
    while chars.next_if(|&(_, c)| !is_word(c)).is_some() {}
    let mut start = chars.peek().map_or(0, |&(i, c)| i + c.len_utf8());
    while let Some((i, _)) = chars.next_if(|&(_, c)| is_word(c)) {
        start = i;
    }
    start
}

/// The end of the word after `at`, past any gap before it.
pub fn next_word(text: &str, at: usize) -> usize {
    let mut chars = text[at..].char_indices().peekable();
    while chars.next_if(|&(_, c)| !is_word(c)).is_some() {}
    while chars.next_if(|&(_, c)| is_word(c)).is_some() {}
    chars.peek().map_or(text.len(), |&(i, _)| at + i)
}

/// The word around `at`, or the gap there when `at` isn't in a word.
pub fn word_at(text: &str, at: usize) -> std::ops::Range<usize> {
    let word = text[at..].chars().next().is_some_and(is_word)
        || text[..at].chars().next_back().is_some_and(is_word);
    let same = |c: char| is_word(c) == word;
    let start = text[..at]
        .char_indices()
        .rev()
        .take_while(|&(_, c)| same(c))
        .last()
        .map_or(at, |(i, _)| i);
    let end = text[at..]
        .char_indices()
        .find(|&(_, c)| !same(c))
        .map_or(text.len(), |(i, _)| at + i);
    start..end
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "cc --model opus-5";

    #[test]
    fn word_moves_skip_the_gap_then_the_word() {
        assert_eq!(prev_word(LINE, LINE.len()), 16);
        assert_eq!(prev_word(LINE, 11), 5);
        assert_eq!(prev_word(LINE, 5), 0);
        assert_eq!(prev_word(LINE, 0), 0);
        assert_eq!(next_word(LINE, 0), 2);
        assert_eq!(next_word(LINE, 2), 10);
        assert_eq!(next_word(LINE, 16), LINE.len());
    }

    #[test]
    fn a_double_click_picks_the_word_or_the_gap_under_it() {
        assert_eq!(word_at(LINE, 7), 5..10);
        assert_eq!(word_at(LINE, 10), 5..10);
        assert_eq!(word_at(LINE, 3), 2..5);
        assert_eq!(word_at("", 0), 0..0);
    }

    #[test]
    fn character_moves_keep_combined_characters_whole() {
        let t = "ae\u{301}b";
        assert_eq!(next_char(t, 1), 4);
        assert_eq!(prev_char(t, 4), 1);
        assert_eq!(prev_char(t, 0), 0);
        assert_eq!(next_char(t, t.len()), t.len());
    }
}
