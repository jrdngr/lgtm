//! Reports: review findings agents publish through the MCP server, a section per reviewer, with
//! the agent's suggested fixes when it gives them.

mod named;
mod submitted;
#[cfg(test)]
mod tests;

use crate::store::report::output;
use crate::store::{Part, Persona, ReportRun};
pub use submitted::Summary;
