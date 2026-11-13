//! GitHub, through the `gh` CLI.

pub mod canonical;
pub mod gh;
pub mod heads;
pub mod pr;
pub mod pulled;
pub mod remote;
pub mod threads;

pub use pr::login;
pub use remote::GhRepo;
