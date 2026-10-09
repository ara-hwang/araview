//! Windows cloud-sync placeholder detection (Files On-Demand).
//!
//! Used when building directory listings so the frontend can skip thumbnail
//! hydration for offline-only files without reading file contents.

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileAvailability {
    Local,
    CloudOnly,
    Unknown,
}

/// Classify a directory entry using metadata only (no file read).
pub fn availability_for_entry(entry: &fs::DirEntry) -> FileAvailability {
    entry
        .metadata()
        .map(|meta| availability_from_metadata(&meta))
        .unwrap_or(FileAvailability::Unknown)
}

pub fn availability_from_metadata(meta: &fs::Metadata) -> FileAvailability {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
        const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x40000;
        const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x400000;

        let attrs = meta.file_attributes();
        if attrs & FILE_ATTRIBUTE_OFFLINE != 0
            || attrs & FILE_ATTRIBUTE_RECALL_ON_OPEN != 0
            || attrs & FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS != 0
        {
            return FileAvailability::CloudOnly;
        }
        FileAvailability::Local
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        FileAvailability::Unknown
    }
}
