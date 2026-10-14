use super::tests::{jj, repo, write};
use super::*;
use crate::store::{Category, FlowStep, Level, Source};

fn input(path: &str, start: u32, end: u32) -> NoteInput {
    NoteInput {
        path: path.into(),
        start,
        end,
        category: Category::Important,
        level: Some(Level::High),
        title: "check the bound".into(),
        body: "`x + 1` overflows at `u32::MAX`.".into(),
        flow: None,
    }
}

#[tokio::test]
async fn notes_land_on_changed_rows_and_follow_edits() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    jj(d, &["commit", "-m", "base"]);
    write(
        d,
        "a.rs",
        "fn a() {}\n\nfn b(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    let review = ctx.open(None, "@", false).await.unwrap();

    let inputs = [input("a.rs", 3, 5), input("elsewhere.rs", 1, 2)];
    let (state, skipped) = ctx
        .save_notes(review.id, &inputs, Source::Headless)
        .unwrap();
    assert_eq!(skipped, 1);
    let h = &state.notes[0];
    assert_eq!((h.level, h.source), (Some(Level::High), Source::Headless));
    let rows = h.rows.unwrap();
    let text = |r: u32| {
        review.files[0]
            .hunks
            .iter()
            .flat_map(|h| &h.rows)
            .nth(r as usize)
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(text(rows.start), "fn b(x: u32) -> u32 {");
    assert_eq!(text(rows.end), "}");

    // New code above moves the rows; the note moves with them.
    write(
        d,
        "a.rs",
        "fn a() {}\n\nfn c() {}\n\nfn b(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    let r = ctx.refresh(review.id, None).await.unwrap().review;
    let rows = r.state.notes[0].rows.unwrap();
    let text = |row: u32| {
        r.files[0]
            .hunks
            .iter()
            .flat_map(|h| &h.rows)
            .nth(row as usize)
            .unwrap()
            .text
            .clone()
    };
    assert_eq!(text(rows.start), "fn b(x: u32) -> u32 {");

    // A new walkthrough replaces the old one.
    let (state, _) = ctx.save_notes(review.id, &[], Source::Terminal).unwrap();
    assert!(state.notes.is_empty());
}

#[tokio::test]
async fn a_range_with_no_changed_rows_takes_the_nearest() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\nfn z() {}\n");
    jj(d, &["commit", "-m", "base"]);
    write(d, "a.rs", "fn a() {}\nfn z() {}\nfn y() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();
    let (state, _) = ctx
        .save_notes(review.id, &[input("a.rs", 1, 1)], Source::Headless)
        .unwrap();
    let rows = state.notes[0].rows.unwrap();
    assert_eq!(rows.start, rows.end);
    let row = review.files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.rows)
        .nth(rows.start as usize)
        .unwrap();
    assert_eq!(row.text, "fn y() {}");
}

#[tokio::test]
async fn notes_keep_the_agents_order_and_only_important_ones_have_a_level() {
    let (dir, ctx) = repo();
    let d = dir.path();
    write(d, "a.rs", "fn a() {}\n");
    jj(d, &["commit", "-m", "base"]);
    write(d, "a.rs", "fn a() {}\nfn b() {}\nfn c() {}\n");
    let review = ctx.open(None, "@", false).await.unwrap();

    let mut flow = input("a.rs", 3, 3);
    flow.category = Category::Flow;
    flow.flow = Some(FlowStep {
        flow: "Startup".into(),
        step: 1,
    });
    let mut unlevelled = input("a.rs", 2, 2);
    unlevelled.level = None;
    let (state, _) = ctx
        .save_notes(
            review.id,
            &[flow, input("elsewhere.rs", 1, 1), unlevelled],
            Source::Headless,
        )
        .unwrap();
    let got: Vec<_> = state
        .notes
        .iter()
        .map(|n| (n.position, n.category, n.level))
        .collect();
    assert_eq!(
        got,
        [
            (0, Category::Flow, None),
            (1, Category::Important, Some(Level::Medium)),
        ]
    );
    assert_eq!(state.notes[0].flow.as_ref().unwrap().flow, "Startup");
}

fn numbered(lines: impl IntoIterator<Item = String>) -> String {
    lines.into_iter().map(|l| l + "\n").collect()
}

#[tokio::test]
async fn ranges_match_new_side_lines_only() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let base: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    write(d, "a.rs", &numbered(base.clone()));
    jj(d, &["commit", "-m", "base"]);
    // Five lines added after line 1 (new lines 2-6), and old line 12 removed.
    let mut new = base.clone();
    new.remove(11);
    for i in (1..=5).rev() {
        new.insert(1, format!("add {i}"));
    }
    write(d, "a.rs", &numbered(new));
    let review = ctx.open(None, "@", false).await.unwrap();

    let (state, _) = ctx
        .save_notes(review.id, &[input("a.rs", 6, 12)], Source::Headless)
        .unwrap();
    let rows = state.notes[0].rows.unwrap();
    assert_eq!(rows.start, rows.end);
    let row = review.files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.rows)
        .nth(rows.start as usize)
        .unwrap();
    assert_eq!(row.text, "add 5");
}

#[tokio::test]
async fn notes_on_rows_that_cant_be_anchored_are_skipped() {
    let (dir, ctx) = repo();
    let d = dir.path();
    let base: Vec<String> = (1..=20).map(|i| format!("line {i}")).collect();
    write(d, "a.rs", &numbered(base.clone()));
    jj(d, &["commit", "-m", "base"]);
    let mut first = base.clone();
    first.insert(15, "X".into());
    write(d, "a.rs", &numbered(first));
    ctx.open(None, "@", false).await.unwrap();

    // Next session X is gone and line 5 changed, so the interdiff removes X.
    let mut second = base.clone();
    second[4] = "five".into();
    write(d, "a.rs", &numbered(second));
    let review = ctx.open(None, "@", false).await.unwrap();
    let change = review.changes[0].change_id.clone();
    let view = View::SinceLast { change };
    let inter = ctx.refresh(review.id, Some(view)).await.unwrap().review;
    assert!(inter.interdiff);

    let inputs = [input("a.rs", 14, 17), input("a.rs", 5, 5)];
    let (state, skipped) = ctx
        .save_notes(review.id, &inputs, Source::Headless)
        .unwrap();
    assert_eq!((skipped, state.notes.len()), (1, 1));
}
