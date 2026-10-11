use super::tests::{jj, repo, write};
use super::*;
use crate::store::{Level, Source};

fn input(path: &str, start: u32, end: u32) -> HotspotInput {
    HotspotInput {
        path: path.into(),
        start,
        end,
        level: Level::High,
        reason: "check the bound".into(),
    }
}

#[tokio::test]
async fn hotspots_land_on_changed_rows_and_follow_edits() {
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
        .save_hotspots(review.id, &inputs, Source::Headless)
        .unwrap();
    assert_eq!(skipped, 1);
    let h = &state.hotspots[0];
    assert_eq!((h.level, h.source), (Level::High, Source::Headless));
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

    // New code above moves the rows; the hotspot moves with them.
    write(
        d,
        "a.rs",
        "fn a() {}\n\nfn c() {}\n\nfn b(x: u32) -> u32 {\n    x + 1\n}\n",
    );
    let r = ctx.refresh(review.id, None).await.unwrap().review;
    let rows = r.state.hotspots[0].rows.unwrap();
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

    // A new pass replaces the old one.
    let (state, _) = ctx.save_hotspots(review.id, &[], Source::Terminal).unwrap();
    assert!(state.hotspots.is_empty());
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
        .save_hotspots(review.id, &[input("a.rs", 1, 1)], Source::Headless)
        .unwrap();
    let rows = state.hotspots[0].rows.unwrap();
    assert_eq!(rows.start, rows.end);
    let row = review.files[0]
        .hunks
        .iter()
        .flat_map(|h| &h.rows)
        .nth(rows.start as usize)
        .unwrap();
    assert_eq!(row.text, "fn y() {}");
}
