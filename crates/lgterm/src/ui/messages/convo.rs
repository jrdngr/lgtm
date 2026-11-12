//! Messages sorted into conversations between spaces, and runs of them from one sender.

use crate::agents::Message;
use crate::model::TermId;

/// Two spaces by id, lower first. One space twice for agents talking within it.
pub type Spaces = (u64, u64);
/// Two agents, lower id first.
pub type Pair = (TermId, TermId);

fn ordered<T: Ord>(a: T, b: T) -> (T, T) {
    if a <= b { (a, b) } else { (b, a) }
}

/// Who sent `m`: its agent, or its receiver when it came from outside any terminal.
pub fn sender(m: &Message) -> TermId {
    m.from.unwrap_or(m.to)
}

pub fn pair_of(m: &Message) -> Pair {
    ordered(sender(m), m.to)
}

/// The spaces of `m`'s two ends, `space` giving each end's space.
pub fn spaces_of(m: &Message, space: &dyn Fn(&Message, TermId) -> u64) -> Spaces {
    ordered(space(m, sender(m)), space(m, m.to))
}

/// The messages between two spaces, with the agent pairs who wrote them.
#[derive(Debug, PartialEq)]
pub struct Convo {
    pub spaces: Spaces,
    /// Newest first.
    pub pairs: Vec<Pair>,
    pub unread: usize,
}

/// Every conversation in `messages`, the one with the newest message first.
pub fn convos(messages: &[Message], space: &dyn Fn(&Message, TermId) -> u64) -> Vec<Convo> {
    let mut out: Vec<Convo> = vec![];
    for m in messages.iter().rev() {
        let spaces = spaces_of(m, space);
        let i = match out.iter().position(|c| c.spaces == spaces) {
            Some(i) => i,
            None => {
                out.push(Convo {
                    spaces,
                    pairs: vec![],
                    unread: 0,
                });
                out.len() - 1
            }
        };
        let c = &mut out[i];
        if !c.pairs.contains(&pair_of(m)) {
            c.pairs.push(pair_of(m));
        }
        c.unread += usize::from(!m.read);
    }
    out
}

/// Back-to-back messages from one sender to one receiver, together.
pub fn runs<'a>(shown: &[&'a Message]) -> Vec<Vec<&'a Message>> {
    let mut out: Vec<Vec<&Message>> = vec![];
    for &m in shown {
        match out.last_mut() {
            Some(run) if run[0].from == m.from && run[0].to == m.to => run.push(m),
            _ => out.push(vec![m]),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(from: u64, to: u64, read: bool) -> Message {
        Message {
            time: "09:00".into(),
            from: Some(TermId(from)),
            from_label: String::new(),
            to: TermId(to),
            to_label: String::new(),
            text: format!("{from}>{to}"),
            queued: false,
            dropped: false,
            read,
            from_space: None,
            to_space: None,
        }
    }

    /// Terminals 1-9 are in space 1, 10 and up in space 2.
    fn space(_: &Message, t: TermId) -> u64 {
        if t.0 < 10 { 1 } else { 2 }
    }

    #[test]
    fn conversations_are_between_spaces_newest_first_with_their_agent_pairs() {
        let log = [
            msg(1, 10, true),
            msg(2, 3, false),
            msg(11, 2, false),
            msg(10, 1, false),
        ];
        assert_eq!(
            convos(&log, &space),
            [
                Convo {
                    spaces: (1, 2),
                    pairs: vec![(TermId(1), TermId(10)), (TermId(2), TermId(11))],
                    unread: 2,
                },
                Convo {
                    spaces: (1, 1),
                    pairs: vec![(TermId(2), TermId(3))],
                    unread: 1,
                },
            ]
        );
    }

    #[test]
    fn runs_break_when_the_sender_or_receiver_changes() {
        let log = [
            msg(1, 10, true),
            msg(1, 10, true),
            msg(1, 11, true),
            msg(10, 1, true),
            msg(1, 10, true),
        ];
        let shown: Vec<&Message> = log.iter().collect();
        let lens: Vec<usize> = runs(&shown).iter().map(Vec::len).collect();
        assert_eq!(lens, [2, 1, 1, 1]);
    }
}
