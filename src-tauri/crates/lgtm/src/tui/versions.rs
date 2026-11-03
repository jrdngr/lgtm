//! A change's snapshots under its row in the picker: labeled ones, or every version in its
//! evolog with `a`. Each starts a step, so opening some reviews what was done after them.

use super::load::Target;
use lgtm_core::jj::Version;

pub struct Versions {
    /// The change's row in the picker.
    pub row: usize,
    /// How the change is named to jj.
    rev: String,
    /// Each version, newest first, with its label.
    list: Vec<(Version, Option<String>)>,
    pub error: Option<String>,
    /// Whether unlabeled versions show too.
    pub all: bool,
    /// Indexes into `list`, always a shown one.
    sel: usize,
    anchor: Option<usize>,
}

/// A shown version: where it is in the picked range, and the version with its label.
pub struct Shown<'a> {
    pub mark: Option<&'static str>,
    pub version: &'a Version,
    pub label: Option<&'a str>,
}

impl Versions {
    pub fn new(row: usize, rev: String) -> Self {
        Self {
            row,
            rev,
            list: vec![],
            error: None,
            all: false,
            sel: 0,
            anchor: None,
        }
    }

    pub fn rev(&self) -> &str {
        &self.rev
    }

    pub fn loaded(&mut self, list: Vec<(Version, Option<String>)>) {
        self.list = list;
        self.anchor = None;
        self.sel = self.shown_indexes().first().copied().unwrap_or(0);
    }

    fn shown_indexes(&self) -> Vec<usize> {
        (0..self.list.len())
            .filter(|&i| self.all || self.list[i].1.is_some())
            .collect()
    }

    pub fn anchored(&self) -> bool {
        self.anchor.is_some()
    }

    /// The picked versions' indexes, newest first.
    fn range(&self) -> (usize, usize) {
        let a = self.anchor.unwrap_or(self.sel);
        (a.min(self.sel), a.max(self.sel))
    }

    fn name(&self, i: usize) -> String {
        let (v, label) = &self.list[i];
        label
            .clone()
            .unwrap_or_else(|| v.commit_id.chars().take(8).collect())
    }

    /// What was done after the picked versions: from the oldest picked one to the next shown
    /// version after the newest, or the change as it is now.
    fn target(&self) -> Option<Target> {
        let (newest, oldest) = self.range();
        if self.list.is_empty() {
            return None;
        }
        let after = self.shown_indexes().into_iter().rev().find(|&i| i < newest);
        let to = match after {
            Some(i) => self.list[i].0.commit_id.clone(),
            // The change follows its edits.
            None => self.rev.clone(),
        };
        let span = if newest == oldest {
            self.name(newest)
        } else {
            format!("{} → {}", self.name(oldest), self.name(newest))
        };
        Some(Target::Between {
            from: self.list[oldest].0.commit_id.clone(),
            to,
            change: self.rev.chars().take(8).collect(),
            span,
        })
    }

    /// Puts the cursor on the `j`th shown version.
    pub fn select(&mut self, j: usize) {
        if let Some(&i) = self.shown_indexes().get(j) {
            self.sel = i;
        }
    }

    /// Where the cursor is among the shown versions, if they're shown at all.
    pub fn at(&self) -> Option<usize> {
        self.shown_indexes().iter().position(|&i| i == self.sel)
    }

    /// Starts a range at the cursor, or drops it.
    pub fn toggle_anchor(&mut self) {
        self.anchor = match self.anchor {
            Some(_) => None,
            None => Some(self.sel),
        }
    }

    pub fn clear_anchor(&mut self) {
        self.anchor = None;
    }

    /// Shows unlabeled versions too, or hides them.
    pub fn set_all(&mut self, all: bool) {
        self.all = all;
        let shown = self.shown_indexes();
        if !shown.contains(&self.sel) {
            self.sel = shown.first().copied().unwrap_or(0);
        }
        if self.anchor.is_some_and(|a| !shown.contains(&a)) {
            self.anchor = None;
        }
    }

    /// What Enter opens: what the picked versions changed.
    pub fn open(&self) -> Option<Target> {
        self.target()
    }

    /// The versions to show, in order.
    pub fn shown(&self) -> Vec<Shown<'_>> {
        let (first, last) = self.range();
        self.shown_indexes()
            .into_iter()
            .map(|i| {
                let mark =
                    (self.anchor.is_some() && (first..=last).contains(&i)).then_some(match i {
                        _ if first == last => "▸",
                        _ if i == first => "┌",
                        _ if i == last => "└",
                        _ => "│",
                    });
                Shown {
                    mark,
                    version: &self.list[i].0,
                    label: self.list[i].1.as_deref(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions() -> Versions {
        let mut v = Versions::new(0, "wkpzrmvq".into());
        let list = ["c4", "c3", "c2", "c1", "c0"]
            .iter()
            .zip([Some("third"), None, Some("second"), None, Some("first")])
            .map(|(id, label)| {
                let version = Version {
                    commit_id: id.to_string(),
                    clock: "15:00".into(),
                };
                (version, label.map(str::to_string))
            })
            .collect();
        v.loaded(list);
        v
    }

    fn between(from: &str, to: &str, span: &str) -> Option<Target> {
        Some(Target::Between {
            from: from.into(),
            to: to.into(),
            change: "wkpzrmvq".into(),
            span: span.into(),
        })
    }

    #[test]
    fn a_snapshot_opens_what_was_done_until_the_next_one() {
        let mut v = versions();
        // The newest snapshot runs to the change as it's edited.
        assert_eq!(v.open(), between("c4", "wkpzrmvq", "third"));
        v.select(1);
        assert_eq!(v.open(), between("c2", "c4", "second"));
        v.select(2);
        assert_eq!(v.open(), between("c0", "c2", "first"));
    }

    #[test]
    fn a_range_spans_its_ends_and_all_shows_every_version() {
        let mut v = versions();
        v.select(1);
        v.toggle_anchor();
        v.select(2);
        assert_eq!(v.open(), between("c0", "c4", "first → second"));
        v.clear_anchor();
        v.set_all(true);
        assert_eq!(v.shown().len(), 5);
        assert_eq!(v.at(), Some(4));
        v.select(3);
        assert_eq!(v.open(), between("c1", "c2", "c1"));
        // Hiding unlabeled versions moves the cursor off a hidden one.
        v.set_all(false);
        assert_eq!(v.at(), Some(0));
    }
}
