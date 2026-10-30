//! The name a user gave a Claude Code session with `/rename`, which Claude Code keeps beside
//! the session's transcript and as `custom-title` entries in it.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How much of a transcript's end is read for its latest name. A rename lands at the end, and
/// it's looked for at the next prompt or stop.
const TAIL: u64 = 256 * 1024;

/// Each terminal's transcript, as its hooks last reported it, by terminal panel id.
static TRANSCRIPTS: Mutex<Option<HashMap<String, PathBuf>>> = Mutex::new(None);

/// Remembers terminal `terminal`'s transcript.
pub fn remember(terminal: &str, transcript: &str) {
    TRANSCRIPTS
        .lock()
        .unwrap()
        .get_or_insert_default()
        .insert(terminal.to_string(), PathBuf::from(transcript));
}

/// Forgets terminal `terminal`'s transcript, once its session has ended.
pub fn forget(terminal: &str) {
    if let Some(map) = TRANSCRIPTS.lock().unwrap().as_mut() {
        map.remove(terminal);
    }
}

/// The name of the session terminal `terminal` last reported, if it has one.
pub fn of_terminal(terminal: &str) -> Option<String> {
    let transcript = TRANSCRIPTS.lock().unwrap().as_ref()?.get(terminal)?.clone();
    read(&transcript)
}

/// The session's name: from `<session>/custom-title.json` next to the transcript, else the
/// last `custom-title` entry near the transcript's end.
pub fn read(transcript: &Path) -> Option<String> {
    from_file(transcript).or_else(|| from_tail(transcript))
}

fn from_file(transcript: &Path) -> Option<String> {
    let session = transcript.file_stem()?;
    let file = transcript.parent()?.join(session).join("custom-title.json");
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    named(&json)
}

fn from_tail(transcript: &Path) -> Option<String> {
    let mut file = std::fs::File::open(transcript).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(TAIL))).ok()?;
    let mut tail = Vec::new();
    file.read_to_end(&mut tail).ok()?;
    last_title(&String::from_utf8_lossy(&tail))
}

/// The last `custom-title` entry's name in JSONL `text`. A first line cut short is skipped.
fn last_title(text: &str) -> Option<String> {
    text.lines()
        .rev()
        .filter(|l| l.contains("\"custom-title\""))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .find(|e| e["type"] == "custom-title")
        .and_then(|e| named(&e))
}

fn named(json: &serde_json::Value) -> Option<String> {
    let name = json["customTitle"].as_str()?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_latest_rename_wins_and_a_cut_first_line_is_skipped() {
        let text = concat!(
            "le\":\"cut\"}\n",
            r#"{"type":"custom-title","customTitle":"first","sessionId":"s"}"#,
            "\n",
            r#"{"type":"user","message":{"content":"custom-title"}}"#,
            "\n",
            r#"{"type":"custom-title","customTitle":"  second  ","sessionId":"s"}"#,
            "\n",
        );
        assert_eq!(last_title(text).as_deref(), Some("second"));
        assert_eq!(
            last_title(r#"{"type":"custom-title","customTitle":""}"#),
            None
        );
    }

    #[test]
    fn the_title_file_beside_the_transcript_wins_over_the_transcript() {
        let dir = tempfile::tempdir().unwrap();
        let transcript = dir.path().join("s1.jsonl");
        std::fs::write(
            &transcript,
            r#"{"type":"custom-title","customTitle":"from transcript"}"#,
        )
        .unwrap();
        assert_eq!(read(&transcript).as_deref(), Some("from transcript"));
        std::fs::create_dir(dir.path().join("s1")).unwrap();
        std::fs::write(
            dir.path().join("s1/custom-title.json"),
            r#"{"customTitle":"from file"}"#,
        )
        .unwrap();
        assert_eq!(read(&transcript).as_deref(), Some("from file"));
        assert_eq!(read(&dir.path().join("none.jsonl")), None);
    }
}
