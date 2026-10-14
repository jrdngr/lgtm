//! Claude Code, run headless with `claude -p` and stream-JSON output.

use serde_json::Value;

/// Arguments for a run that reads its prompt from stdin. It can read files, and with `edit`
/// edit them and use the user's skills. It can't run commands, and the user's MCP servers
/// stay off.
pub fn args<'a>(
    resume: Option<&'a str>,
    edit: bool,
    schema: Option<&'a str>,
    model: Option<&'a str>,
) -> Vec<&'a str> {
    let mut args = vec![
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--strict-mcp-config",
    ];
    let tools = if edit {
        "Read,Grep,Glob,Edit,Write,Skill"
    } else {
        "Read,Grep,Glob"
    };
    args.extend(["--tools", tools]);
    if edit {
        // A headless run can't ask, so edits in the workspace and skills are allowed up front.
        args.extend([
            "--permission-mode",
            "acceptEdits",
            "--allowedTools",
            "Skill",
        ]);
    }
    if let Some(id) = resume {
        args.extend(["--resume", id]);
    }
    if let Some(schema) = schema {
        args.extend(["--json-schema", schema]);
    }
    if let Some(model) = model {
        args.extend(["--model", model]);
    }
    args
}

/// What one line of output says.
#[derive(Debug, PartialEq)]
pub enum Line {
    /// A piece of the reply as it streams.
    Text(String),
    /// A block of text starts, as each assistant message does.
    TextStart,
    /// A tool call, summed up like "Read src/a.rs".
    Tool(String),
    /// The run finished. `text` is the final reply, or the error.
    Done {
        text: String,
        session: Option<String>,
        error: bool,
        /// The reply as JSON, when the run was given a schema.
        structured: Option<Value>,
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
        Some("stream_event")
            if str_at("/event/type") == Some("content_block_start")
                && str_at("/event/content_block/type") == Some("text") =>
        {
            Line::TextStart
        }
        Some("assistant") => v
            .pointer("/message/content")
            .and_then(Value::as_array)
            .and_then(|c| c.iter().rev().find(|b| b["type"] == "tool_use"))
            .map_or(Line::Other, |b| Line::Tool(tool_summary(b))),
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
                structured: v.get("structured_output").filter(|s| !s.is_null()).cloned(),
            }
        }
        _ => Line::Other,
    }
}

/// A tool call as a few words: its name and the file or pattern it's about.
fn tool_summary(block: &Value) -> String {
    let name = block["name"].as_str().unwrap_or("Tool");
    if name == "StructuredOutput" {
        return "Writing up".to_string();
    }
    let input = &block["input"];
    let target = ["file_path", "pattern", "path"]
        .iter()
        .find_map(|k| input[k].as_str());
    match target {
        Some(t) => format!("{name} {t}"),
        None => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_deltas_and_the_result() {
        let delta = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi "}},"session_id":"s"}"#;
        assert_eq!(parse(delta), Line::Text("hi ".into()));
        let start = r#"{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}},"session_id":"s"}"#;
        assert_eq!(parse(start), Line::TextStart);
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":"hi there","session_id":"s1"}"#;
        assert_eq!(
            parse(done),
            Line::Done {
                text: "hi there".into(),
                session: Some("s1".into()),
                error: false,
                structured: None,
            }
        );
        let structured = r#"{"type":"result","subtype":"success","is_error":false,"result":"{}","structured_output":{"a":1},"session_id":"s1"}"#;
        assert!(matches!(parse(structured), Line::Done { structured: Some(v), .. } if v["a"] == 1));
        let tool = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/a.rs"}}]}}"#;
        assert_eq!(parse(tool), Line::Tool("Read src/a.rs".into()));
        let failed = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["boom"],"session_id":"s1"}"#;
        assert!(matches!(parse(failed), Line::Done { error: true, text, .. } if text == "boom"));
        assert_eq!(parse(r#"{"type":"system","subtype":"init"}"#), Line::Other);
        assert_eq!(parse("not json"), Line::Other);
    }
}
