use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Events emitted by the volume poller thread for frontend status updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VolumeEvent {
    /// A new volume was detected — indexing is about to start.
    #[serde(rename = "mountDetected")]
    MountDetected {
        path: String,
        name: String,
    },
    /// Indexing of a volume has completed.
    #[serde(rename = "indexComplete")]
    IndexComplete {
        path: String,
        #[serde(rename = "fileCount")]
        file_count: usize,
        #[serde(rename = "totalIndexed")]
        total_indexed: usize,
    },
    /// A volume was unmounted — index entries are being removed.
    #[serde(rename = "volumeRemoved")]
    VolumeRemoved {
        path: String,
        name: String,
        #[serde(rename = "totalIndexed")]
        total_indexed: usize,
    },
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
