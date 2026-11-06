//! The editor's core: buffers, selections, the key table, motions and edits. Nothing here
//! draws, so it's tested through key cases.

pub mod buffer;
pub mod fold;
pub mod gutter;
mod insert;
pub mod keymap;
pub mod keys;
pub mod markdown;
pub mod motion;
pub mod objects;
pub mod ops;

#[cfg(test)]
pub mod spec;
#[cfg(test)]
mod tests;
