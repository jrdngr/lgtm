//! Claude Code, run headless with `claude -p` and stream-JSON output.

use serde_json::Value;

/// Arguments for a run that reads its prompt from stdin. It can read files, and edit them
/// too with `edit`. Nothing else is available, and the user's MCP servers stay off.
pub fn args(resume: Option<&str>, edit: bool) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--strict-mcp-config",
    ]
    .map(str::to_string)
    .into();
    let tools = if edit {
        "Read,Grep,Glob,Edit,Write"
    } else {
        "Read,Grep,Glob"
    };
    args.extend(["--tools".to_string(), tools.to_string()]);
    if edit {
        // A headless run can't ask, so edits in the workspace are allowed up front.
        args.extend(["--permission-mode".to_string(), "acceptEdits".to_string()]);
    }
    if let Some(id) = resume {
        args.extend(["--resume".to_string(), id.to_string()]);
    }
    args
}

/// What one line of output says.
#[derive(Debug, PartialEq)]
pub enum Line {
    /// A piece of the reply as it streams.
    Text(String),
    /// The run finished. `text` is the final reply, or the error.
    Done {
        text: String,
        session: Option<String>,
        error: bool,
    },
    Other,
}

pub fn parse(line: &str) -> Line {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Line::Other;
    };
    let str_at = |path: &str| v.pointer(path).and_then(Value::as_str);
    match str_at("/type") {
        Some("stream_event")
            if str_at("/event/type") == Some("content_block_delta")
                && str_at("/event/delta/type") == Some("text_delta") =>
        {
            Line::Text(str_at("/event/delta/text").unwrap_or_default().to_string())
        }
        Some("result") => {
            let error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false)
                || str_at("/subtype").is_some_and(|s| s != "success");
            let text = str_at("/result")
                .map(str::to_string)
                .or_else(|| {
                    v.get("errors").and_then(Value::as_array).map(|e| {
                        e.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                })
                .unwrap_or_default();
            Line::Done {
                text,
                session: str_at("/session_id").map(str::to_string),
                error,
            }
        }
        _ => Line::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_deltas_and_the_result() {
        let delta = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi "}},"session_id":"s"}"#;
        assert_eq!(parse(delta), Line::Text("hi ".into()));
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":"hi there","session_id":"s1"}"#;
        assert_eq!(
            parse(done),
            Line::Done {
                text: "hi there".into(),
                session: Some("s1".into()),
                error: false
            }
        );
        let failed = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["boom"],"session_id":"s1"}"#;
        assert!(matches!(parse(failed), Line::Done { error: true, text, .. } if text == "boom"));
        assert_eq!(parse(r#"{"type":"system","subtype":"init"}"#), Line::Other);
        assert_eq!(parse("not json"), Line::Other);
    }
}
