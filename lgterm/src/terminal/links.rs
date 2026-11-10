//! Links under the pointer: URLs, and paths with an optional line number.

#[derive(Debug, PartialEq, Eq)]
pub enum Link {
    Url(String),
    /// A path as written, and the line after it if any.
    File(String, Option<u32>),
}

/// The link in `line` around character `col`, if there is one.
pub fn at(line: &str, col: usize) -> Option<Link> {
    let chars: Vec<char> = line.chars().collect();
    if col >= chars.len() {
        return None;
    }
    let stop = |c: char| c.is_whitespace() || "\"'`<>()[]{}|".contains(c);
    if stop(chars[col]) {
        return None;
    }
    let start = (0..col)
        .rev()
        .take_while(|i| !stop(chars[*i]))
        .last()
        .unwrap_or(col);
    let end = (col..chars.len()).take_while(|i| !stop(chars[*i])).last()? + 1;
    let word: String = chars[start..end].iter().collect();
    let word = word.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    if word.starts_with("http://") || word.starts_with("https://") {
        return Some(Link::Url(word.to_owned()));
    }
    let mut parts = word.splitn(3, ':');
    let path = parts.next()?;
    let line = parts.next().and_then(|l| l.parse().ok());
    let looks_like_path = path.contains('/') || path.contains('.');
    (looks_like_path && !path.is_empty()).then(|| Link::File(path.to_owned(), line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_lose_trailing_punctuation() {
        let line = "see https://example.com/a?b=1. ok";
        assert_eq!(
            at(line, 8),
            Some(Link::Url("https://example.com/a?b=1".into()))
        );
    }

    #[test]
    fn paths_carry_their_line() {
        let line = "  --> src/review/arch/mod.rs:4:5";
        assert_eq!(
            at(line, 12),
            Some(Link::File("src/review/arch/mod.rs".into(), Some(4)))
        );
        assert_eq!(
            at("edit (keys.ts)", 8),
            Some(Link::File("keys.ts".into(), None))
        );
    }

    #[test]
    fn plain_words_and_gaps_are_not_links() {
        assert_eq!(at("cargo test", 2), None);
        assert_eq!(at("a b", 1), None);
        assert_eq!(at("short", 40), None);
    }
}
