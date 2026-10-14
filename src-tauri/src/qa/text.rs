//! Questions and answers as plain text, for the agent and for copying.

use super::queue::QaQuestion;

/// An answer in words, for the agent and for copying.
pub fn answer_text(q: &QaQuestion) -> String {
    let Some(a) = &q.answer else {
        return "not decided".into();
    };
    let mut out = match (a.option, &a.other) {
        (Some(o), _) => {
            let label = q
                .text
                .options
                .get(o as usize)
                .map_or("", |c| c.label.as_str());
            format!("option {}: {label}", o + 1)
        }
        (None, Some(other)) => format!("none of the options: {other}"),
        (None, None) => "none of the options".into(),
    };
    if let Some(note) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
        out.push_str(&format!(" (note: {})", note.trim()));
    }
    out
}

/// A question in the qqueue template, as plain text.
pub fn question_text(q: &QaQuestion) -> String {
    let t = &q.text;
    let mut out = format!("{} — {}\n\nContext:\n\n", q.id, t.subject);
    for c in &t.context {
        out.push_str(&format!("- {c}\n"));
    }
    out.push_str("\nOptions:\n\n");
    for (i, o) in t.options.iter().enumerate() {
        let rec = if i == 0 { " (recommended)" } else { "" };
        out.push_str(&format!("{}. {}{rec}\n", i + 1, o.label));
        if !o.means.is_empty() {
            out.push_str(&format!("   {}\n", o.means));
        }
        let tagged = [("Pro", &o.pros), ("Con", &o.cons), ("Risk", &o.risks)];
        for (tag, items) in tagged {
            for b in items {
                out.push_str(&format!("   - {tag}: {b}\n"));
            }
        }
        for b in &o.bullets {
            out.push_str(&format!("   - {b}\n"));
        }
    }
    out.push_str(&format!("\nRecommendation: option 1. {}", t.why));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qa::queue::{Answer, Choice, QState, QText};

    #[test]
    fn the_text_follows_the_qqueue_template() {
        let q = QaQuestion {
            id: "Q1".into(),
            round: 1,
            text: QText {
                subject: "Where files live".into(),
                context: vec!["c".into()],
                options: vec![
                    Choice {
                        label: "yes".into(),
                        means: "do it".into(),
                        pros: vec!["simple".into()],
                        risks: vec!["breaks old files".into()],
                        ..Choice::default()
                    },
                    Choice {
                        label: "no".into(),
                        ..Choice::default()
                    },
                ],
                why: "because".into(),
            },
            sources: vec![],
            depends: vec![],
            state: QState::Decided,
            answer: Some(Answer {
                option: Some(1),
                note: Some("keep it simple".into()),
                other: None,
                at: 1.0,
                auto: None,
            }),
            dropped: None,
            change: None,
            discussion: vec![],
        };
        assert_eq!(answer_text(&q), "option 2: no (note: keep it simple)");
        assert_eq!(
            question_text(&q),
            "Q1 — Where files live\n\nContext:\n\n- c\n\nOptions:\n\n1. yes (recommended)\n   do it\n\
             \x20  - Pro: simple\n   - Risk: breaks old files\n2. no\n\nRecommendation: option 1. because"
        );
    }
}
