//! Numbering a batch of new questions. The agent writes `depends` with the ids the batch
//! would get in the order it gave them. The batch is reordered so no question comes before
//! one it depends on, then numbered for real.

use std::collections::HashMap;

use super::queue::NewQuestion;

/// The number in a question id: 4 for `Q4` and `Q4a`.
pub(super) fn number(id: &str) -> u32 {
    id.trim_start_matches('Q')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// Whether `id` is a subquestion of `parent`, like `Q4a` of `Q4`.
pub(super) fn is_sub(id: &str, parent: &str) -> bool {
    id.strip_prefix(parent)
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_lowercase()))
}

/// The id of a unit's `member`th question, with units numbered on from `last`.
fn id(last: u32, unit: usize, member: usize) -> String {
    let n = last + 1 + unit as u32;
    match member {
        0 => format!("Q{n}"),
        m => format!("Q{n}{}", (b'a' + m as u8 - 1) as char),
    }
}

/// A question of the batch with its final id and where it was in the batch.
pub(super) struct Placed {
    pub id: String,
    pub index: usize,
    pub question: NewQuestion,
}

/// Orders and numbers `new` after the question numbered `last`. Each top-level question and
/// the subquestions after it move as a unit, and a unit comes after the ones it depends on,
/// keeping the given order otherwise; a cycle is broken at its earliest unit. Dependencies
/// on the batch are renumbered, ones on questions that `exists` are kept, and the rest go.
pub(super) fn place(
    new: Vec<NewQuestion>,
    last: u32,
    exists: impl Fn(&str) -> bool,
) -> Vec<Placed> {
    let mut units: Vec<Vec<usize>> = vec![];
    for (i, n) in new.iter().enumerate() {
        match units.last_mut() {
            Some(u) if n.sub => u.push(i),
            _ => units.push(vec![i]),
        }
    }
    let mut unit_of = HashMap::new();
    for (u, members) in units.iter().enumerate() {
        for m in 0..members.len() {
            unit_of.insert(id(last, u, m), (u, m));
        }
    }
    let needs: Vec<Vec<usize>> = units
        .iter()
        .enumerate()
        .map(|(u, members)| {
            members
                .iter()
                .flat_map(|&i| &new[i].depends)
                .filter_map(|d| unit_of.get(d).map(|&(du, _)| du))
                .filter(|&du| du != u)
                .collect()
        })
        .collect();
    let mut placed = vec![false; units.len()];
    let mut order = vec![];
    while order.len() < units.len() {
        let mut left = (0..units.len()).filter(|&u| !placed[u]);
        let first = left.clone().next().unwrap();
        let next = left
            .find(|&u| needs[u].iter().all(|&d| placed[d]))
            .unwrap_or(first);
        placed[next] = true;
        order.push(next);
    }
    let position: HashMap<usize, usize> = order.iter().enumerate().map(|(p, &u)| (u, p)).collect();
    let rename = |old: &str| -> Option<String> {
        match unit_of.get(old) {
            Some(&(u, m)) => Some(id(last, position[&u], m)),
            None => exists(old).then(|| old.to_string()),
        }
    };
    let mut new: Vec<Option<NewQuestion>> = new.into_iter().map(Some).collect();
    let mut out = vec![];
    for (p, &u) in order.iter().enumerate() {
        for (m, &index) in units[u].iter().enumerate() {
            let mut question = new[index].take().unwrap();
            let id = id(last, p, m);
            let mut depends = vec![];
            for d in question.depends.iter().filter_map(|d| rename(d)) {
                if d != id && !depends.contains(&d) {
                    depends.push(d);
                }
            }
            question.depends = depends;
            out.push(Placed {
                id,
                index,
                question,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new(subject: &str, sub: bool, depends: &[&str]) -> NewQuestion {
        NewQuestion {
            sub,
            subject: subject.into(),
            context: vec![],
            options: vec![],
            why: String::new(),
            sources: vec![],
            depends: depends.iter().map(|d| d.to_string()).collect(),
            auto: None,
        }
    }

    fn summary(placed: &[Placed]) -> Vec<(String, &str, Vec<String>)> {
        placed
            .iter()
            .map(|p| {
                (
                    p.id.clone(),
                    p.question.subject.as_str(),
                    p.question.depends.clone(),
                )
            })
            .collect()
    }

    #[test]
    fn units_move_after_what_they_depend_on_and_dependencies_are_renumbered() {
        // As given: Q3 a, Q4 b, Q4a b1, Q5 c, after existing questions up to Q2.
        let placed = place(
            vec![
                new("a", false, &["Q5", "Q1", "Q9", "Q3"]),
                new("b", false, &[]),
                new("b1", true, &["Q4"]),
                new("c", false, &["Q4a"]),
            ],
            2,
            |id| id == "Q1",
        );
        let s = |v: &[&str]| v.iter().map(|d| d.to_string()).collect::<Vec<_>>();
        assert_eq!(
            summary(&placed),
            [
                ("Q3".into(), "b", s(&[])),
                ("Q3a".into(), "b1", s(&["Q3"])),
                ("Q4".into(), "c", s(&["Q3a"])),
                ("Q5".into(), "a", s(&["Q4", "Q1"])),
            ]
        );
        assert_eq!(
            placed.iter().map(|p| p.index).collect::<Vec<_>>(),
            [1, 2, 3, 0]
        );
    }

    #[test]
    fn a_cycle_is_broken_at_its_earliest_unit() {
        let placed = place(
            vec![
                new("a", false, &["Q2"]),
                new("b", false, &["Q1"]),
                new("c", false, &[]),
            ],
            0,
            |_| false,
        );
        let ids: Vec<_> = summary(&placed)
            .into_iter()
            .map(|(id, subject, _)| (id, subject))
            .collect();
        assert_eq!(
            ids,
            [
                ("Q1".to_string(), "c"),
                ("Q2".into(), "a"),
                ("Q3".into(), "b")
            ]
        );
    }

    #[test]
    fn subquestions_belong_to_their_parent_only() {
        assert!(is_sub("Q4a", "Q4"));
        assert!(!is_sub("Q41", "Q4"));
        assert!(!is_sub("Q4", "Q4"));
    }
}
