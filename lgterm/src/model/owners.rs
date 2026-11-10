//! Who opened each terminal, and the names tabs were given.

use super::{Model, TermId};

/// Most steps up the openers a check follows.
const MAX_DEPTH: usize = 64;

impl Model {
    pub fn set_opener(&mut self, term: TermId, by: TermId) {
        self.openers.insert(term, by);
    }

    /// Whether `term` is `by` or was opened by it, directly or through terminals it opened.
    pub fn opened_by(&self, term: TermId, by: TermId) -> bool {
        let mut at = term;
        for _ in 0..MAX_DEPTH {
            if at == by {
                return true;
            }
            match self.openers.get(&at) {
                Some(&up) => at = up,
                None => return false,
            }
        }
        false
    }

    pub fn name(&self, term: TermId) -> Option<&str> {
        self.names.get(&term).map(String::as_str)
    }

    pub fn set_name(&mut self, term: TermId, name: String) {
        self.names.insert(term, name);
    }

    /// Drops a closed terminal's name and place among the openers. What it opened passes to
    /// its own opener, so that one can still close them.
    pub fn forget(&mut self, term: TermId) {
        self.names.remove(&term);
        let up = self.openers.remove(&term);
        let children: Vec<TermId> = self
            .openers
            .iter()
            .filter(|&(_, &o)| o == term)
            .map(|(&t, _)| t)
            .collect();
        for child in children {
            match up {
                Some(up) => self.openers.insert(child, up),
                None => self.openers.remove(&child),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_lead_hands_its_team_to_the_orchestrator() {
        let (mut m, boss) = Model::new();
        let (lead, member, other) = (TermId(100), TermId(101), TermId(102));
        m.set_opener(lead, boss);
        m.set_opener(member, lead);
        assert!(m.opened_by(member, boss));
        assert!(m.opened_by(lead, lead));
        assert!(!m.opened_by(other, boss));
        assert!(!m.opened_by(boss, lead));
        m.forget(lead);
        assert!(m.opened_by(member, boss));
        m.forget(boss);
        assert!(!m.opened_by(member, boss));
    }
}
