//! Messages between agents: what they say, and what the receiver reads.

use serde::{Deserialize, Serialize};

use crate::model::TermId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Local time sent, as HH:MM.
    pub time: String,
    pub from: Option<TermId>,
    pub from_label: String,
    pub to: TermId,
    pub to_label: String,
    pub text: String,
    pub queued: bool,
    /// The agent it was queued for left before it could go in.
    #[serde(default)]
    pub dropped: bool,
}

/// The oldest messages are dropped past this many.
const KEEP: usize = 500;

/// The local time as HH:MM.
pub fn clock() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: localtime_r only writes the struct it's given.
    let tm = unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&now, &mut tm);
        tm
    };
    format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
}

/// Drops the oldest message past `KEEP`, sparing queued ones unless nothing else is left.
pub fn trim(messages: &mut Vec<Message>) {
    if messages.len() > KEEP {
        let i = messages.iter().position(|m| !m.queued).unwrap_or(0);
        messages.remove(i);
    }
}

/// What the receiving agent reads.
pub fn envelope(m: &Message) -> String {
    match m.from {
        Some(from) => format!(
            "[lgterm message from {from} · {}] {}\n(Reply with: lgterm send {from} \"...\")",
            m.from_label, m.text
        ),
        None => format!("[lgterm message] {}", m.text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(from: Option<u64>) -> Message {
        Message {
            time: "14:31".into(),
            from: from.map(TermId),
            from_label: "Review keys".into(),
            to: TermId(5),
            to_label: "Modal bar".into(),
            text: "tests pass".into(),
            queued: true,
            dropped: false,
        }
    }

    #[test]
    fn envelopes_say_who_to_reply_to() {
        let e = envelope(&message(Some(2)));
        assert!(e.starts_with("[lgterm message from t2 · Review keys] tests pass"));
        assert!(e.contains("lgterm send t2"));
        assert_eq!(envelope(&message(None)), "[lgterm message] tests pass");
    }

    #[test]
    fn trimming_keeps_undelivered_messages() {
        let mut log = vec![message(Some(1))];
        log.extend((0..KEEP).map(|_| Message {
            queued: false,
            ..message(Some(2))
        }));
        trim(&mut log);
        assert_eq!(log.len(), KEEP);
        assert_eq!(log[0], message(Some(1)));

        let mut queued = vec![message(Some(1)); KEEP + 1];
        queued[1].text = "second".into();
        trim(&mut queued);
        assert_eq!((queued.len(), queued[0].text.as_str()), (KEEP, "second"));
    }

    #[test]
    fn the_clock_reads_hours_and_minutes() {
        let c = clock();
        assert_eq!(c.len(), 5);
        assert_eq!(&c[2..3], ":");
    }
}
