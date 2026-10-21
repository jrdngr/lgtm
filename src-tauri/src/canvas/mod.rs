//! Canvases: the drawings and the drawing over design maps. The user draws in the UI; a
//! terminal agent changes one through MCP in passes the user can undo.

mod focus;
mod model;
mod ops;

pub use focus::*;
pub use model::*;
pub use ops::*;
