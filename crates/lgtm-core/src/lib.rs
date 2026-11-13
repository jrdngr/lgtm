//! What `lgtm` is built on: jj and git, diffs and their anchors, syntax highlighting, GitHub,
//! and the store.

pub mod anchor;
pub mod config;
pub mod diff;
pub mod fuzzy;
pub mod git;
pub mod github;
pub mod jj;
pub mod paths;
pub mod repo;
pub mod resolve;
pub mod review;
pub mod store;
pub mod syntax;
pub mod turns;
pub mod watch;
