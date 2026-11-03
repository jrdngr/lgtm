//! A change's snapshots under its row in the picker: labeled ones, or every version in its
//! evolog with `a`. Opening some reviews what they changed, from the version before them.

use lgtm_core::jj::Version;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

use super::load::Target;

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

pub enum Key {
    None,
    Close,
    Open(Target),
}

/// A shown version: its index in the list, whether it's picked and where in the range, and
/// the version with its label.
pub struct Shown<'a> {
    pub sel: bool,
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

    /// What the picked versions changed: from the shown version before the oldest picked
    /// one, or the change's parents, to the newest picked one.
    fn target(&self) -> Option<Target> {
        let (newest, oldest) = self.range();
        if self.list.is_empty() {
            return None;
        }
        let shown = self.shown_indexes();
        let before = shown.iter().find(|&&i| i > oldest);
        // The newest version follows the change as it's edited.
        let to = match newest {
            0 => self.rev.clone(),
            i => self.list[i].0.commit_id.clone(),
        };
        let change: String = self.rev.chars().take(8).collect();
        let span = if newest == oldest {
            self.name(newest)
        } else {
            format!("{} → {}", self.name(oldest), self.name(newest))
        };
        Some(match before {
            Some(&b) => Target::Between {
                from: self.list[b].0.commit_id.clone(),
                to,
                change,
                span,
            },
            None => Target::Revset(to),
        })
    }

    fn step(&mut self, down: bool) {
        let shown = self.shown_indexes();
        let next = if down {
            shown.into_iter().find(|&i| i > self.sel)
        } else {
            shown.into_iter().rev().find(|&i| i < self.sel)
        };
        if let Some(i) = next {
            self.sel = i;
        }
    }

    pub fn key(&mut self, key: KeyEvent) -> Key {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.step(true),
            KeyCode::Char('k') | KeyCode::Up => self.step(false),
            KeyCode::Char(' ') => {
                self.anchor = match self.anchor {
                    Some(_) => None,
                    None => Some(self.sel),
                }
            }
            KeyCode::Char('a') => {
                self.all = !self.all;
                let shown = self.shown_indexes();
                if !shown.contains(&self.sel) {
                    self.sel = shown.first().copied().unwrap_or(0);
                }
                if self.anchor.is_some_and(|a| !shown.contains(&a)) {
                    self.anchor = None;
                }
            }
            KeyCode::Esc if self.anchor.is_some() => self.anchor = None,
            KeyCode::Esc | KeyCode::Char('s') => return Key::Close,
            KeyCode::Enter => {
                if let Some(t) = self.target() {
                    return Key::Open(t);
                }
            }
            _ => {}
        }
        Key::None
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
                    sel: i == self.sel,
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
    use ratatui::crossterm::event::KeyModifiers;

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

    fn press(v: &mut Versions, code: KeyCode) -> Key {
        v.key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn opened(k: Key) -> Target {
        match k {
            Key::Open(t) => t,
            _ => panic!("nothing opened"),
        }
    }

    fn between(from: &str, to: &str, span: &str) -> Target {
        Target::Between {
            from: from.into(),
            to: to.into(),
            change: "wkpzrmvq".into(),
            span: span.into(),
        }
    }

    #[test]
    fn a_labeled_snapshot_opens_from_the_label_before_it() {
        let mut v = versions();
        // The newest version follows the change itself.
        assert_eq!(
            opened(press(&mut v, KeyCode::Enter)),
            between("c2", "wkpzrmvq", "third")
        );
        press(&mut v, KeyCode::Char('j'));
        assert_eq!(
            opened(press(&mut v, KeyCode::Enter)),
            between("c0", "c2", "second")
        );
        // The oldest shown one opens the whole change as it was then.
        press(&mut v, KeyCode::Char('j'));
        assert_eq!(
            opened(press(&mut v, KeyCode::Enter)),
            Target::Revset("c0".into())
        );
    }

    #[test]
    fn a_range_spans_its_ends_and_a_shows_every_version() {
        let mut v = versions();
        press(&mut v, KeyCode::Char(' '));
        press(&mut v, KeyCode::Char('j'));
        assert_eq!(
            opened(press(&mut v, KeyCode::Enter)),
            between("c0", "wkpzrmvq", "second → third")
        );
        press(&mut v, KeyCode::Esc);
        press(&mut v, KeyCode::Char('a'));
        assert_eq!(v.shown().len(), 5);
        press(&mut v, KeyCode::Char('j'));
        assert_eq!(
            opened(press(&mut v, KeyCode::Enter)),
            between("c0", "c1", "c1")
        );
    }
}
