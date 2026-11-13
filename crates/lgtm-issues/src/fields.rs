//! The header fields that tie an issue to code and to other issues: its revision, what blocks
//! it, how bad it is, and what it duplicates.

use crate::issue::{Issue, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub const ALL: [Severity; 4] = [
        Severity::Low,
        Severity::Medium,
        Severity::High,
        Severity::Critical,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Severity::Low => "low",
            Severity::Medium => "medium",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }

    pub fn parse(s: &str) -> Option<Severity> {
        Severity::ALL.into_iter().find(|v| v.name() == s)
    }
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
    /// The issues blocking this one that aren't done or dropped yet.
    pub fn open_blockers(&self, all: &[Issue]) -> Vec<u32> {
        self.blocked_by
            .iter()
            .copied()
            .filter(|b| {
                all.iter()
                    .find(|i| i.id == *b)
                    .is_none_or(|i| !matches!(i.status, Status::Done | Status::Dropped))
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
    fn only_unfinished_blockers_still_block() {
        let mut all: Vec<Issue> = (1..=3)
            .map(|id| Issue::new(id, &format!("Issue {id}"), None, "", 0))
            .collect();
        all[0].status = Status::Done;
        all[1].status = Status::Dropped;
        let mut issue = Issue::new(4, "Blocked", None, "", 0);
        issue.blocked_by = vec![1, 2, 3, 9];
        assert_eq!(issue.open_blockers(&all), [3, 9]);
    }
}
