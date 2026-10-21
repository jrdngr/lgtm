use super::reply::{FileRows, GuidedReply, promoted, validate};
use super::*;
use crate::diff::RowKind;
use crate::review::build::RowPos;

fn row(kind: RowKind, old: Option<u32>, new: Option<u32>) -> RowPos {
    RowPos {
        kind,
        old_line: old,
        new_line: new,
        replaced: false,
    }
}

fn reply(json: serde_json::Value) -> GuidedReply {
    serde_json::from_value(json).unwrap()
}

/// `a.rs` adds lines 3-4 and 10; `b.rs` removes its old line 5, which sat before new line 5.
fn files() -> Vec<FileRows> {
    use RowKind::*;
    let a = [
        row(Context, Some(2), Some(2)),
        row(Added, None, Some(3)),
        row(Added, None, Some(4)),
        row(Context, Some(3), Some(5)),
        row(Context, Some(8), Some(9)),
        row(Added, None, Some(10)),
    ];
    let b = [row(Context, Some(4), Some(4)), row(Removed, Some(5), None), row(Context, Some(6), Some(5))];
    vec![FileRows::of("a.rs", &a), FileRows::of("b.rs", &b)]
}

#[test]
fn changed_spans_place_removed_rows_after_the_last_new_line() {
    let f = files();
    assert_eq!(f[0].changed, [(3, 4), (10, 10)]);
    assert_eq!(f[1].changed, [(5, 5)]);
}

#[test]
fn chunks_keep_only_review_files_and_get_unique_ids() {
    let (chunks, _, _) = validate(
        reply(serde_json::json!({
            "revset": "@", "from": "a", "to": "b",
            "chunks": [
                { "id": "Lock wait", "title": "t", "risk": "high", "writeup": "w",
                  "ranges": [{ "path": "a.rs", "start": 3, "end": 4 },
                             { "path": "gone.rs", "start": 1, "end": 2 }],
                  "docs": ["docs/adr/1.md", "nowhere.md"] },
                { "id": "lock-wait", "title": "t", "risk": "low", "writeup": "w",
                  "ranges": [{ "path": "a.rs", "start": 10, "end": 9 }] },
                { "id": "x", "title": "t", "risk": "medium", "writeup": "w",
                  "ranges": [{ "path": "gone.rs", "start": 1, "end": 2 }] }
            ],
            "excluded": [],
            "context": [{ "path": "docs/adr/1.md", "kind": "adr", "title": "Locks", "summary": "s" }]
        })),
        &files(),
    );
    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0].id, "lock-wait");
    assert_eq!(chunks[1].id, "lock-wait-2");
    assert_eq!(chunks[0].ranges.len(), 1);
    assert_eq!(chunks[0].docs, ["docs/adr/1.md"]);
    // An end before the start becomes the start.
    assert_eq!((chunks[1].ranges[0].start, chunks[1].ranges[0].end), (10, 10));
}

#[test]
fn changed_lines_nothing_names_are_listed_as_excluded() {
    let (_, excluded, _) = validate(
        reply(serde_json::json!({
            "revset": "@", "from": "a", "to": "b",
            "chunks": [{ "id": "c", "title": "t", "risk": "low", "writeup": "w",
                         "ranges": [{ "path": "a.rs", "start": 3, "end": 3 }] }],
            "excluded": [{ "path": "b.rs", "reason": "simple", "why": "rename" }]
        })),
        &files(),
    );
    assert_eq!(excluded.len(), 2);
    // A whole-file exclusion takes the file's changed lines.
    assert_eq!(excluded[0].reason, ExcludeReason::Simple);
    assert_eq!((excluded[0].ranges[0].start, excluded[0].ranges[0].end), (5, 5));
    // a.rs:10 is in no chunk; 3-4 overlaps one, so it isn't listed.
    assert_eq!(excluded[1].reason, ExcludeReason::Unlisted);
    assert_eq!(excluded[1].path, "a.rs");
    let listed: Vec<_> = excluded[1].ranges.iter().map(|r| (r.start, r.end)).collect();
    assert_eq!(listed, [(10, 10)]);
}

#[test]
fn a_promoted_part_keeps_its_lines_at_the_picked_risk() {
    let e = ExcludedDto {
        path: "src/keys.rs".into(),
        ranges: vec![GuidedRange {
            path: "src/keys.rs".into(),
            start: 21,
            end: 21,
        }],
        reason: ExcludeReason::Simple,
        why: "one constant".into(),
    };
    let c = promoted(e, ChunkRisk::Medium, &[]);
    assert_eq!(c.risk, ChunkRisk::Medium);
    assert!(c.promoted);
    assert_eq!(c.id, "promoted-keys-rs");
    assert_eq!(c.ranges[0].start, 21);
}

#[test]
fn an_update_prompt_names_the_last_chunks_and_what_changed() {
    let last = Guided {
        chunks: vec![ChunkDto {
            id: "lock-wait".into(),
            title: "Lock wait".into(),
            risk: ChunkRisk::High,
            tags: vec![],
            writeup: String::new(),
            ranges: vec![GuidedRange {
                path: "a.rs".into(),
                start: 3,
                end: 4,
            }],
            related: vec![],
            steps: vec![],
            docs: vec![],
            tests: None,
            promoted: false,
        }],
        excluded: vec![],
        context: vec![],
        created_at: 0.0,
        commits: Commits {
            from: Some("base".into()),
            to: "old".into(),
        },
    };
    let text = prompt::text("@", "base", "new", Some(&last), "Rules.");
    assert!(text.contains("Rules."));
    assert!(text.contains("`lock-wait`"));
    assert!(text.contains("a.rs:3-4"));
    assert!(text.contains("jj diff --from old --to new"));
    let fresh = prompt::text("@", "base", "new", None, "Rules.");
    assert!(!fresh.contains("Updating"));
}
