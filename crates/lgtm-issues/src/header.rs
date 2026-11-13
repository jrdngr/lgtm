//! The `key: value` header between `---` lines that issue files, `STATUS.md`, `TEMPLATE.md`
//! and `workflow.md` all start with.

/// A header's lines as written, in order.
pub type Pairs = Vec<(String, String)>;

/// Splits a file into its header's pairs and the text after it. A file with no header is all
/// text.
pub fn split(text: &str) -> Result<(Pairs, &str), String> {
    let Some(rest) = text.strip_prefix("---\n") else {
        return Ok((Vec::new(), text));
    };
    let (head, rest) = rest
        .split_once("\n---\n")
        .or_else(|| rest.strip_suffix("\n---").map(|h| (h, "")))
        .or_else(|| rest.strip_prefix("---\n").map(|r| ("", r)))
        .ok_or("its header has no closing ---")?;
    let pairs = head
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect();
    Ok((pairs, rest))
}

/// The value of `key`, when it's there and not empty.
pub fn get<'a>(pairs: &'a Pairs, key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .filter(|v| !v.is_empty())
}

/// A comma-separated value's items.
pub fn list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_splits_from_its_text_and_a_file_without_one_is_all_text() {
        let (pairs, rest) = split("---\nlabel: In review\ndone:\n---\n\nBody").unwrap();
        assert_eq!(get(&pairs, "label"), Some("In review"));
        assert_eq!(get(&pairs, "done"), None);
        assert_eq!(rest, "\nBody");
        assert_eq!(split("Body").unwrap(), (vec![], "Body"));
        assert_eq!(split("---\n---\nBody").unwrap(), (vec![], "Body"));
        assert!(split("---\nlabel: x\n").is_err());
        assert_eq!(list(" a, b ,, c"), ["a", "b", "c"]);
    }
}
