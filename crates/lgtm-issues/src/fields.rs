//! Header fields: the three built in ones that tie an issue to code and to other issues, and
//! the ones a tracker defines in its `workflow.md`, like severity.

use crate::issue::Issue;

/// The fields every tracker uses unless its `workflow.md` lists others.
pub const DEFAULT: [&str; 4] = ["severity", "revision", "blocked-by", "duplicate-of"];

/// Severity's values when `workflow.md` doesn't give them.
pub const SEVERITY: &str = "low:muted, medium:fg, high:amber, critical:red";

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub value: String,
    /// A theme color's name.
    pub color: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// The jj change, or git branch or commit, holding the code written for an issue.
    Revision,
    /// Issues that must be done first.
    BlockedBy,
    /// The issue this one repeats; setting it drops this one.
    DuplicateOf,
    Choice(Vec<Choice>),
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub kind: Kind,
}

impl Field {
    /// A field by its name, with the values `workflow.md` gives it, if any.
    pub fn new(name: &str, values: Option<&str>) -> Field {
        let kind = match name {
            "revision" => Kind::Revision,
            "blocked-by" => Kind::BlockedBy,
            "duplicate-of" => Kind::DuplicateOf,
            _ => match values.or((name == "severity").then_some(SEVERITY)) {
                Some(v) => Kind::Choice(choices(v)),
                None => Kind::Text,
            },
        };
        Field {
            name: name.to_owned(),
            kind,
        }
    }

    /// Where `value` comes among a choice field's values, for sorting.
    pub fn rank(&self, value: &str) -> Option<usize> {
        match &self.kind {
            Kind::Choice(cs) => cs.iter().position(|c| c.value == value),
            _ => None,
        }
    }

    /// The color `value` is shown in, when the field gives it one.
    pub fn color(&self, value: &str) -> Option<&str> {
        match &self.kind {
            Kind::Choice(cs) => cs.iter().find(|c| c.value == value)?.color.as_deref(),
            _ => None,
        }
    }
}

/// `low:muted, medium, high:amber` as values with their colors.
fn choices(line: &str) -> Vec<Choice> {
    crate::header::list(line)
        .into_iter()
        .map(|item| match item.split_once(':') {
            Some((v, c)) => Choice {
                value: v.trim().to_owned(),
                color: Some(c.trim().to_owned()).filter(|c| !c.is_empty()),
            },
            None => Choice {
                value: item,
                color: None,
            },
        })
        .collect()
}

/// Issue ids written as `3, 7`, `#3 #7` or `3 7`.
pub fn parse_ids(s: &str) -> Result<Vec<u32>, String> {
    let mut ids: Vec<u32> = Vec::new();
    for word in s.split([',', ' ']).filter(|w| !w.is_empty()) {
        let id = word
            .trim_start_matches('#')
            .parse()
            .map_err(|_| format!("{word} isn't an issue id"))?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

pub fn format_ids(ids: &[u32]) -> String {
    ids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

impl Issue {
    /// The issues blocking this one that aren't in a done status yet. `done` says whether a
    /// status is one.
    pub fn open_blockers(&self, all: &[Issue], done: impl Fn(&str) -> bool) -> Vec<u32> {
        self.blocked_by
            .iter()
            .copied()
            .filter(|b| {
                all.iter()
                    .find(|i| i.id == *b)
                    .is_none_or(|i| !done(&i.status))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_read_in_any_of_the_written_forms() {
        assert_eq!(parse_ids("3, 7"), Ok(vec![3, 7]));
        assert_eq!(parse_ids("#3 #7 #3"), Ok(vec![3, 7]));
        assert_eq!(parse_ids(""), Ok(vec![]));
        assert!(parse_ids("3, x").is_err());
    }

    #[test]
    fn a_values_line_makes_a_choice_with_optional_colors_and_severity_has_defaults() {
        let area = Field::new("area", Some("ui, core:cyan"));
        assert_eq!(area.rank("core"), Some(1));
        assert_eq!((area.color("ui"), area.color("core")), (None, Some("cyan")));
        assert_eq!(Field::new("notes", None).kind, Kind::Text);
        assert_eq!(Field::new("severity", None).color("critical"), Some("red"));
        assert_eq!(Field::new("blocked-by", Some("a, b")).kind, Kind::BlockedBy);
    }

    #[test]
    fn only_blockers_outside_a_done_status_still_block() {
        let mut all: Vec<Issue> = (1..=3)
            .map(|id| Issue::new(id, &format!("Issue {id}"), None, "", 0))
            .collect();
        all[0].status = "done".into();
        all[1].status = "dropped".into();
        let mut issue = Issue::new(4, "Blocked", None, "", 0);
        issue.blocked_by = vec![1, 2, 3, 9];
        let done = |s: &str| s == "done" || s == "dropped";
        assert_eq!(issue.open_blockers(&all, done), [3, 9]);
    }
}
