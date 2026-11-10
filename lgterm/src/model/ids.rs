//! Ids for everything in the model, and the tabs a split holds.

use serde::{Deserialize, Serialize};

pub use lgterm_proto::TermId;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub u64);
    };
}

id!(SplitId);
id!(SpaceId);
id!(LayoutId);
id!(SlotId);

/// What a tab shows: a terminal, or the window's one Messages tab. Saved as the terminal's
/// number, or null for Messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Tab {
    Term(TermId),
    Messages,
}

impl Tab {
    pub fn term(self) -> Option<TermId> {
        match self {
            Tab::Term(t) => Some(t),
            Tab::Messages => None,
        }
    }
}

impl From<TermId> for Tab {
    fn from(t: TermId) -> Tab {
        Tab::Term(t)
    }
}
