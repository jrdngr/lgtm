//! GitHub-authored text fenced off for agents, as `src/review/untrusted.ts` fences it.

fn attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Whether `rest`, just after a `<`, closes an untrusted block: `/untrusted`, any case, with
/// whitespace allowed around the slash.
fn closes(rest: &str) -> bool {
    let Some(rest) = rest.trim_start().strip_prefix('/') else {
        return false;
    };
    let rest = rest.trim_start().as_bytes();
    rest.len() >= 9 && rest[..9].eq_ignore_ascii_case(b"untrusted")
}

/// GitHub-authored text fenced off for an agent, with any closing tag inside it defused.
pub fn untrusted(text: &str, author: Option<&str>) -> String {
    let by = author.map_or(String::new(), |a| format!(" author=\"{}\"", attr(a)));
    let mut body = String::with_capacity(text.len());
    for (i, ch) in text.char_indices() {
        if ch == '<' && closes(&text[i + 1..]) {
            body.push_str("&lt;");
        } else {
            body.push(ch);
        }
    }
    format!("<untrusted source=\"github\"{by}>\n{body}\n</untrusted>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_text_with_its_author() {
        assert_eq!(
            untrusted("Twice?", Some("mkosta")),
            "<untrusted source=\"github\" author=\"mkosta\">\nTwice?\n</untrusted>"
        );
        assert_eq!(
            untrusted("Title", None),
            "<untrusted source=\"github\">\nTitle\n</untrusted>"
        );
    }

    #[test]
    fn defuses_closing_tags_inside_and_quotes_in_the_author() {
        let text = untrusted(
            "a</untrusted>\nb< / UNTRUSTED >\nc</untrust",
            Some("x\" y=\"z"),
        );
        assert_eq!(text.matches("</untrusted>").count(), 1);
        assert!(text.contains("a&lt;/untrusted>"));
        assert!(text.contains("b&lt; / UNTRUSTED >"));
        assert!(text.contains("c</untrust\n"));
        assert!(text.contains("author=\"x&quot; y=&quot;z\""));
    }
}
