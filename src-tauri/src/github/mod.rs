//! GitHub, through the `gh` CLI.

pub mod gh;
pub mod pr;
pub mod publish;
pub mod remote;
pub mod threads;

pub use pr::{PrInfo, login};
pub use publish::Verdict;
pub use remote::GhRepo;
