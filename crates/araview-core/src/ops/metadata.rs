//! EXIF, 히스토그램, 파일 상세, ComicInfo 조회와 표지 지정 커맨드.

use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};

pub fn get_exif_data_impl(file_path: &str) -> Result<HashMap<String, String>, AppError> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    let file = fs::File::open(path)
        .map_err(|e| AppError::io("Failed to open file", e, ErrorCode::Corrupt))?;
    let mut reader = BufReader::new(file);

    // EXIF가 없거나 읽을 수 없는 파일은 실패가 아니라 빈 결과다. 프론트는
    // 빈 맵을 "EXIF 없음" 상태로 보여준다.
    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(exif) => exif,
        Err(e) => {
            log::debug!("[exif] no readable EXIF in {}: {e}", path.display());
            return Ok(HashMap::new());
        }
    };

    let mut data = HashMap::new();

    for field in exif.fields() {
        let tag_name = field.tag.to_string();
        let value = field.display_value().with_unit(&exif).to_string();
        data.insert(tag_name, value);
    }

    Ok(data)
}

/// CBZ/ZIP 안의 ComicInfo.xml 메타데이터를 읽는다.
/// XML이 없거나 CBZ/ZIP이 아니면 `None`을 돌려주고, 깨진 XML은 에러다.
pub fn get_comic_info_blocking(
    file_path: &str,
) -> Result<Option<crate::comic_info::ComicInfo>, AppError> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    crate::comic_info::read_comic_info(path)
}

/// 표지 페이지(`FrontCover`) 집합을 ComicInfo.xml에 쓴다. 원본 아카이브를
/// 고쳐 쓰고, 반영된 메타데이터를 돌려준다.
pub fn set_comic_cover_pages_impl(
    file_path: &str,
    cover_pages: &[u32],
) -> Result<crate::comic_info::ComicInfo, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }
    crate::comic_info::write_cover_pages(path, cover_pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;
    use crate::ops::test_support::*;

    #[test]
    fn get_exif_data_rejects_missing_file() {
        let err = get_exif_data_impl("D:\\no-such-dir-commands\\nope.jpg").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn get_exif_data_without_exif_is_empty_not_error() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/sample.png");
        let data = get_exif_data_impl(sample.to_str().unwrap()).expect("no-exif is not an error");
        assert!(data.is_empty());
    }

    #[test]
    fn get_exif_data_reads_tags_from_jpeg() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/exif-sample.jpg");
        let data = get_exif_data_impl(sample.to_str().unwrap()).expect("exif");
        assert!(!data.is_empty());
    }

    #[test]
    fn get_comic_info_rejects_missing_file() {
        let err = get_comic_info_blocking("D:\\no-such-dir-commands\\nope.cbz").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn get_comic_info_returns_none_without_xml() {
        use std::io::Write as _;

        let dir = unique_dir("comic-noinfo");
        let archive_path = dir.join("plain.cbz");
        let file = fs::File::create(&archive_path).expect("create cbz");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("001.png", options).expect("start entry");
        writer.write_all(b"fake-png-bytes").expect("write entry");
        writer.finish().expect("finish cbz");

        let info = get_comic_info_blocking(archive_path.to_str().unwrap()).expect("read");
        assert!(info.is_none());
        fs::remove_dir_all(&dir).ok();
    }
}
