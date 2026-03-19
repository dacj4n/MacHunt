use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchMode {
    Substring,
    Pattern,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub query: String,
    pub mode: SearchMode,
    pub path_prefix: Option<PathBuf>,
    pub include_files: bool,
    pub include_dirs: bool,
}

impl SearchOptions {
    pub fn normalize(mut self) -> Self {
        if !self.include_files && !self.include_dirs {
            self.include_files = true;
            self.include_dirs = true;
        }
        self
    }
}
