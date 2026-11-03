//! A review's files as marks and checks see them, matching stored marks and checks onto their
//! rows, and the edits marking and checking make. Shared by the desktop and terminal apps.

pub mod edits;
pub mod file;
pub mod matching;

use serde::{Deserialize, Serialize};
use specta::Type;

/// A row by file index and its index among the file's rows, counting across hunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
pub struct RowRef {
    pub file: u32,
    pub row: u32,
}

impl RowRef {
    pub fn new(file: usize, row: u32) -> Self {
        Self {
            file: file as u32,
            row,
        }
    }
}
