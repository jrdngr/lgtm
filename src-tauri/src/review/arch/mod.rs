//! The architecture review from `lgtm-arch`, and the app side of it: loading a review's
//! architecture and storing what agents send, on the repo.

pub use lgtm_arch::*;

pub mod assess;
pub mod describe;
pub mod design;
pub mod edge;
pub mod explore;
pub mod inputs;
mod load;
pub mod state;
pub mod symbols;
pub mod system;
#[cfg(test)]
mod tests;
pub mod tour;
pub mod tour_prompt;
pub mod view;
