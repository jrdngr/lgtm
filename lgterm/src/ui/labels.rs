//! Names shown for terminals, spaces and layouts.

/// A program's title without the spinner or status glyph agents put in front.
pub fn clean_title(title: &str) -> Option<&str> {
    let t = title
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim();
    (!t.is_empty()).then_some(t)
}

/// Whether a process is a coding agent.
pub fn is_agent(process: &str) -> bool {
    matches!(process, "claude" | "codex")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_lose_leading_glyphs() {
        assert_eq!(clean_title("✳ Review keys"), Some("Review keys"));
        assert_eq!(clean_title("⠂ "), None);
        assert_eq!(clean_title("vim"), Some("vim"));
    }
}
