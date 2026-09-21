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
        return FileAvailability::Local;
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        FileAvailability::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn local_file_is_local_on_windows_or_unknown_elsewhere() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.png");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"x").unwrap();
        let entry = fs::read_dir(dir.path())
            .unwrap()
            .find(|e| e.as_ref().map(|e| e.path() == path).unwrap_or(false))
            .unwrap()
            .unwrap();
        let availability = availability_for_entry(&entry);
        #[cfg(windows)]
        assert_eq!(availability, FileAvailability::Local);
        #[cfg(not(windows))]
        assert_eq!(availability, FileAvailability::Unknown);
    }
}
