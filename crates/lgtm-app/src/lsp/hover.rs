//! A hover's contents as markdown, whichever of LSP's shapes they come in.

use serde_json::Value;

fn piece(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => {
            let value = o.get("value")?.as_str()?;
            if let Some(lang) = o.get("language").and_then(Value::as_str) {
                return Some(format!("```{lang}\n{value}\n```"));
            }
            match o.get("kind").and_then(Value::as_str) {
                Some("plaintext") => Some(format!("```text\n{value}\n```")),
                _ => Some(value.to_string()),
            }
        }
        _ => None,
    }
}

/// `contents` as markdown, or `None` when it's empty.
pub fn markdown(contents: &Value) -> Option<String> {
    let parts: Vec<String> = match contents {
        Value::Array(a) => a.iter().filter_map(piece).collect(),
        one => piece(one).into_iter().collect(),
    };
    let text = parts.join("\n\n---\n\n");
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn markup_strings_and_language_blocks_all_become_markdown() {
        let markup = json!({"kind": "markdown", "value": "```rust\nfn a()\n```"});
        assert_eq!(markdown(&markup).unwrap(), "```rust\nfn a()\n```");
        let old = json!([{"language": "python", "value": "def f(): ..."}, "Docs."]);
        assert_eq!(
            markdown(&old).unwrap(),
            "```python\ndef f(): ...\n```\n\n---\n\nDocs."
        );
        assert_eq!(markdown(&json!("")), None);
        assert_eq!(markdown(&Value::Null), None);
    }
}
