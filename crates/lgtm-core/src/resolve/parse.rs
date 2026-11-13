//! Reading a file's conflicts out of its markers: jj's diff and snapshot styles, and git's.

use serde::{Deserialize, Serialize};

/// Where a side's text came from.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    /// Empty when the marker doesn't name a commit.
    pub change_id: String,
    pub commit_id: String,
    /// The commit's description as the marker quotes it, else the marker's own words.
    pub description: String,
    /// Length of the shortest unique prefix of the change id, 0 until jj says.
    pub unique: u32,
    pub bookmarks: Vec<String>,
}

/// One side of a conflict, or its base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub label: Label,
    /// Whole lines, each ending in a newline but maybe the file's last.
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conflict {
    /// In the order the markers list them; side `n` has key `n + 1`.
    pub sides: Vec<Side>,
    /// The first base the markers give.
    pub base: Side,
    /// The markers and all, as the file had them.
    pub raw: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Text(String),
    Conflict(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub parts: Vec<Part>,
    pub conflicts: Vec<Conflict>,
}

/// The char a marker line repeats and how many times, when `line` is one: seven or more of
/// it, then a space or the end.
fn marker(line: &str) -> Option<(char, usize)> {
    let c = line.chars().next()?;
    if !matches!(c, '<' | '>' | '%' | '\\' | '+' | '-' | '|' | '=') {
        return None;
    }
    let run = line.chars().take_while(|&x| x == c).count();
    let rest = &line[run..];
    (run >= 7 && (rest.is_empty() || rest.starts_with(' '))).then_some((c, run))
}

/// The words after a marker's run of chars.
fn words(line: &str, len: usize) -> &str {
    line[len..].trim()
}

/// A label from a marker's words: `change commit "description"`, else the words as they are.
pub fn label(words: &str) -> Label {
    let words = words
        .trim_start_matches("diff from:")
        .trim_start_matches("to:");
    let words = words.trim();
    let mut it = words.splitn(3, ' ');
    if let (Some(change), Some(commit), Some(desc)) = (it.next(), it.next(), it.next()) {
        let quoted = desc.len() >= 2 && desc.starts_with('"') && desc.ends_with('"');
        let id = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric());
        if quoted && id(change) && id(commit) {
            return Label {
                change_id: change.into(),
                commit_id: commit.into(),
                description: desc[1..desc.len() - 1].replace("\\\"", "\""),
                ..Label::default()
            };
        }
    }
    Label {
        description: words.into(),
        ..Label::default()
    }
}

/// What the lines after a section marker go to.
enum Section {
    /// A diff from the base to a side: context, removed and added lines.
    Diff {
        base: usize,
        side: usize,
    },
    Side(usize),
    Base(usize),
}

/// Builds conflicts as their markers go by.
#[derive(Default)]
struct Building {
    sides: Vec<Side>,
    bases: Vec<Side>,
    section: Option<Section>,
}

impl Building {
    fn side(&mut self, label: Label) -> usize {
        self.sides.push(Side {
            label,
            text: String::new(),
        });
        self.sides.len() - 1
    }

    fn base(&mut self, label: Label) -> usize {
        self.bases.push(Side {
            label,
            text: String::new(),
        });
        self.bases.len() - 1
    }

    /// Takes a line of content, with its newline.
    fn content(&mut self, line: &str) {
        match self.section {
            Some(Section::Side(i)) => self.sides[i].text.push_str(line),
            Some(Section::Base(i)) => self.bases[i].text.push_str(line),
            Some(Section::Diff { base, side }) => {
                let (sign, rest) = match line.chars().next() {
                    Some(c @ (' ' | '-' | '+')) => (c, &line[1..]),
                    _ => (' ', line),
                };
                if sign != '+' {
                    self.bases[base].text.push_str(rest);
                }
                if sign != '-' {
                    self.sides[side].text.push_str(rest);
                }
            }
            None => {}
        }
    }

    fn finish(self, raw: String) -> Conflict {
        let base = self.bases.into_iter().next().unwrap_or(Side {
            label: Label {
                description: "base".into(),
                ..Label::default()
            },
            text: String::new(),
        });
        Conflict {
            sides: self.sides,
            base,
            raw,
        }
    }
}

/// The file's text between conflicts and its conflicts. Text with a start marker but no end
/// stays text.
pub fn parse(text: &str) -> Parsed {
    let mut out = Parsed::default();
    let mut plain = String::new();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let bare = line.trim_end_matches(['\n', '\r']);
        match marker(bare) {
            Some(('<', len)) => match conflict(&lines[i..], len) {
                Some((c, used)) => {
                    if !plain.is_empty() {
                        out.parts.push(Part::Text(std::mem::take(&mut plain)));
                    }
                    out.parts.push(Part::Conflict(out.conflicts.len()));
                    out.conflicts.push(c);
                    i += used;
                }
                None => {
                    plain.push_str(line);
                    i += 1;
                }
            },
            _ => {
                plain.push_str(line);
                i += 1;
            }
        }
    }
    if !plain.is_empty() {
        out.parts.push(Part::Text(plain));
    }
    out
}

/// The conflict starting at `lines[0]`, a start marker `len` long, and how many lines it
/// takes, or None when no end marker closes it.
fn conflict(lines: &[&str], len: usize) -> Option<(Conflict, usize)> {
    let first = lines[0].trim_end_matches(['\n', '\r']);
    let mut b = Building::default();
    // Git's style puts the first side right after the start marker.
    let next = lines.get(1).map(|l| l.trim_end_matches(['\n', '\r']));
    let jj = next
        .and_then(marker)
        .is_some_and(|(c, n)| n == len && matches!(c, '%' | '+' | '-'));
    if !jj {
        let side = b.side(label(words(first, len)));
        b.section = Some(Section::Side(side));
    }
    let mut raw = lines[0].to_string();
    for (n, line) in lines.iter().enumerate().skip(1) {
        raw.push_str(line);
        let bare = line.trim_end_matches(['\n', '\r']);
        let Some((c, run)) = marker(bare).filter(|&(_, run)| run == len) else {
            b.content(line);
            continue;
        };
        let words = words(bare, run);
        match c {
            '>' => {
                if !jj && let Some(Section::Side(_)) = b.section {
                    // Git's end marker names the last side.
                    if let Some(last) = b.sides.last_mut()
                        && last.label.description.is_empty()
                    {
                        last.label = label(words);
                    }
                }
                return Some((b.finish(raw), n + 1));
            }
            '%' => {
                let base = b.base(label(words));
                let side = b.side(Label::default());
                b.section = Some(Section::Diff { base, side });
            }
            '\\' => {
                if let Some(Section::Diff { side, .. }) = b.section {
                    b.sides[side].label = label(words);
                }
            }
            '+' => {
                let side = b.side(label(words));
                b.section = Some(Section::Side(side));
            }
            '-' | '|' => {
                let base = b.base(label(words));
                b.section = Some(Section::Base(base));
            }
            '=' => {
                let side = b.side(Label::default());
                b.section = Some(Section::Side(side));
            }
            _ => b.content(line),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const THREE: &str = "name = app
<<<<<<< conflict 1 of 1
%%%%%%% diff from: xttlvzwk 48f5d424 \"base: add config\"
\\\\\\\\\\\\\\        to: vtxpkkml b7c93fd8 \"side X: color = red\"
-color = blue
+color = red
%%%%%%% diff from: xttlvzwk 48f5d424 \"base: add config\"
\\\\\\\\\\\\\\        to: rwynkmzx 66dae73f \"side Y\"
-color = blue
+color = green
+++++++ psunvtko a1134658 \"side Z\"
color = yellow
>>>>>>> conflict 1 of 1 ends
mode = dev
";

    #[test]
    fn jj_diff_sections_give_each_side_its_text_and_label_and_the_base() {
        let p = parse(THREE);
        assert_eq!(
            p.parts,
            [
                Part::Text("name = app\n".into()),
                Part::Conflict(0),
                Part::Text("mode = dev\n".into())
            ]
        );
        let c = &p.conflicts[0];
        let texts: Vec<&str> = c.sides.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(
            texts,
            ["color = red\n", "color = green\n", "color = yellow\n"]
        );
        assert_eq!(c.base.text, "color = blue\n");
        assert_eq!(c.base.label.description, "base: add config");
        let x = &c.sides[0].label;
        assert_eq!(
            (
                x.change_id.as_str(),
                x.commit_id.as_str(),
                x.description.as_str()
            ),
            ("vtxpkkml", "b7c93fd8", "side X: color = red")
        );
        assert_eq!(c.sides[2].label.change_id, "psunvtko");
        assert_eq!(c.raw.lines().count(), 12);
    }

    #[test]
    fn context_lines_go_to_both_and_an_empty_snapshot_is_an_empty_side() {
        let text = "<<<<<<< conflict 1 of 1
+++++++ vtxpkkml b7c93fd8 \"X\"
%%%%%%% diff from: xttlvzwk 48f5d424 \"base\"
\\\\\\\\\\\\\\        to: rwynkmzx 66dae73f \"Y\"
 keep
-base
+mod
>>>>>>> conflict 1 of 1 ends
";
        let c = &parse(text).conflicts[0];
        assert_eq!(c.sides[0].text, "");
        assert_eq!(c.sides[1].text, "keep\nmod\n");
        assert_eq!(c.base.text, "keep\nbase\n");
    }

    #[test]
    fn git_style_markers_name_sides_from_the_start_and_end_lines() {
        let text = "a\n<<<<<<< ours\nx\n||||||| base\nb\n=======\ny\n>>>>>>> theirs\nz\n";
        let p = parse(text);
        let c = &p.conflicts[0];
        assert_eq!(c.sides[0].text, "x\n");
        assert_eq!(c.sides[0].label.description, "ours");
        assert_eq!(c.sides[1].text, "y\n");
        assert_eq!(c.sides[1].label.description, "theirs");
        assert_eq!(c.base.text, "b\n");
        assert_eq!(p.parts.len(), 3);
    }

    #[test]
    fn longer_markers_keep_seven_char_lines_as_content_and_unclosed_starts_stay_text() {
        let text = "<<<<<<<<<<< conflict 1 of 1\n+++++++++++ a\n=======\n>>>>>>>>>>> end\n";
        let c = &parse(text).conflicts[0];
        assert_eq!(c.sides[0].text, "=======\n");
        let open = "<<<<<<< conflict\n+++++++ a\nx\n";
        assert_eq!(parse(open).parts, [Part::Text(open.into())]);
    }
}
