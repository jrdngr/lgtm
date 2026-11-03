//! The editor's core: buffers, selections, the key table, motions and edits. Nothing here
//! draws, so it's tested through key cases.

pub mod buffer;
mod insert;
pub mod keymap;
pub mod keys;
pub mod motion;
pub mod ops;

#[cfg(test)]
mod spec;
#[cfg(test)]
mod tests;
