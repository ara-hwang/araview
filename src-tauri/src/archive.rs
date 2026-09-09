use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::image::is_image_file;

/// 아카이브 내부의 이미지 엔트리 이름을 정렬된 순서로 반환
pub fn list_archive_images(archive_path: &Path) -> Result<Vec<String>, String> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "cbz" => list_zip_images(archive_path),
        "cb7" => list_7z_images(archive_path),
        _ => Err("Unsupported archive format".to_string()),
    }
}

/// 아카이브에서 특정 엔트리를 임시 파일로 추출하고 경로를 반환
pub fn extract_archive_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    let ext = archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "cbz" => extract_zip_image(archive_path, entry_name, temp_dir),
        "cb7" => extract_7z_image(archive_path, entry_name, temp_dir),
        _ => Err("Unsupported archive format".to_string()),
    }
}

fn list_zip_images(archive_path: &Path) -> Result<Vec<String>, String> {
    let file =
        fs::File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;

    let mut images: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read entry: {}", e))?;
        let name = entry.name().to_string();

        // 디렉토리 스킵, 숨김 파일(__MACOSX 등) 스킵
        if entry.is_dir() || name.starts_with("__") || name.starts_with('.') {
            continue;
        }

        // 엔트리 이름을 Path로 변환하여 이미지 확장자 확인
        let entry_path = Path::new(&name);
        if is_image_file(entry_path) {
            images.push(name);
        }
    }

    images.sort_by_key(|n| n.to_lowercase());
    Ok(images)
}

fn extract_zip_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {    let file =
        fs::File::open(archive_path).map_err(|e| format!("Failed to open archive: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;

    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| format!("Entry not found: {}", e))?;

    // 엔트리 이름에서 파일명만 추출 (하위 디렉토리 구조 무시)
    let file_name = Path::new(entry_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");

    let out_path = temp_dir.join(file_name);

    let mut buf = Vec::new();
    entry
        .read_to_end(&mut buf)
        .map_err(|e| format!("Failed to read entry data: {}", e))?;

    fs::write(&out_path, &buf).map_err(|e| format!("Failed to write temp file: {}", e))?;

    Ok(out_path)
}

fn list_7z_images(archive_path: &Path) -> Result<Vec<String>, String> {
    let reader =
        sevenz_rust2::ArchiveReader::open(archive_path, sevenz_rust2::Password::empty())
            .map_err(|e| format!("Failed to read 7z: {e}"))?;

    let mut images: Vec<String> = Vec::new();
    for entry in reader.archive().files.iter() {
        if entry.is_directory() {
            continue;
        }
        let name = entry.name().to_string();
        if name.starts_with("__") || name.starts_with('.') {
            continue;
        }
        if is_image_file(Path::new(&name)) {
            images.push(name);
        }
    }

    images.sort_by_key(|n| n.to_lowercase());
    Ok(images)
}

fn extract_7z_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, String> {
    let mut reader =
        sevenz_rust2::ArchiveReader::open(archive_path, sevenz_rust2::Password::empty())
            .map_err(|e| format!("Failed to read 7z: {e}"))?;

    let buf = reader
        .read_file(entry_name)
        .map_err(|e| format!("Entry not found: {e}"))?;

    let file_name = Path::new(entry_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");

    let out_path = temp_dir.join(file_name);
    fs::write(&out_path, &buf).map_err(|e| format!("Failed to write temp file: {e}"))?;

    Ok(out_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unsupported_archive_ext() {
        let result = list_archive_images(Path::new("file.tar"));
        assert!(result.is_err());
    }

    /// sevenz-rust2 writer로 CB7 픽스처를 만들어 목록/추출 왕복 검증
    fn write_cb7_fixture(dir: &Path) -> PathBuf {
        let a = dir.join("001.png");
        let b = dir.join("sub-002.jpg");
        fs::write(&a, b"fake-png-bytes").unwrap();
        fs::write(&b, b"fake-jpg-bytes").unwrap();
        fs::write(dir.join("note.txt"), b"not an image").unwrap();

        let archive_path = dir.join("comic.cb7");
        let mut writer =
            sevenz_rust2::ArchiveWriter::create(&archive_path).expect("create cb7");
        writer.set_encrypt_header(false);
        for (src, name) in [(&a, "001.png"), (&b, "sub/sub-002.jpg")] {
            let file = fs::File::open(src).expect("open fixture");
            let entry = sevenz_rust2::ArchiveEntry::from_path(src, name.to_string());
            writer
                .push_archive_entry(entry, Some(file))
                .expect("push entry");
        }
        // 텍스트 파일은 아카이브에 넣되 목록에서는 제외되어야 함
        let note = dir.join("note.txt");
        let note_file = fs::File::open(&note).expect("open note");
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::from_path(&note, "note.txt".to_string()),
                Some(note_file),
            )
            .expect("push note");
        writer.finish().expect("finish cb7");
        archive_path
    }

    #[test]
    fn test_cb7_list_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_cb7_fixture(dir.path());

        let images = list_archive_images(&archive_path).expect("list cb7");
        assert_eq!(images, vec!["001.png", "sub/sub-002.jpg"]);

        let out_dir = dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let extracted =
            extract_archive_image(&archive_path, "sub/sub-002.jpg", &out_dir).expect("extract");
        assert_eq!(extracted.file_name().unwrap(), "sub-002.jpg");
        assert_eq!(fs::read(&extracted).unwrap(), b"fake-jpg-bytes");

        let missing = extract_archive_image(&archive_path, "nope.png", &out_dir);
        assert!(missing.is_err());
    }

    #[test]
    fn test_cb7_uppercase_extension() {
        let dir = tempfile::tempdir().unwrap();
        let lower = write_cb7_fixture(dir.path());
        let upper = dir.path().join("comic.CB7");
        fs::rename(&lower, &upper).unwrap();
        let images = list_archive_images(&upper).expect("list CB7");
        assert_eq!(images.len(), 2);
    }
}
