use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Volume events use serde_json::Value for reliable cross-boundary serialization.
pub type VolumeEvent = serde_json::Value;

/// A file entry returned from DB queries, carrying metadata to avoid fs::stat calls.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub dir_path: String,
    pub file_name: String,
    pub is_dir: bool,
    pub size_bytes: Option<u64>,
    pub modified_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SearchMode {
    Substring,
    Pattern,
    Fuzzy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Name,
    Path,
    Type,
    Size,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchOptions {
    pub query: String,
    pub mode: SearchMode,
    pub case_sensitive: bool,
    pub path_prefix: Option<PathBuf>,
    pub include_files: bool,
    pub include_dirs: bool,
    pub limit: Option<usize>,
    pub extensions: Option<Vec<String>>,
    pub size_min_bytes: Option<u64>,
    pub size_max_bytes: Option<u64>,
    pub time_min_ms: Option<u64>,
    pub time_max_ms: Option<u64>,
    pub sort_key: SortKey,
    pub sort_ascending: bool,
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
