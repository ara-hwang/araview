//! EXIF, 히스토그램, 파일 상세, ComicInfo 조회 커맨드.

use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};

use super::run_blocking;

#[tauri::command]
pub async fn get_exif_data(file_path: String) -> Result<HashMap<String, String>, AppError> {
    run_blocking("exif", move || get_exif_data_impl(&file_path)).await
}

fn get_exif_data_impl(file_path: &str) -> Result<HashMap<String, String>, AppError> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    let file = fs::File::open(path)
        .map_err(|e| AppError::io("Failed to open file", e, ErrorCode::Corrupt))?;
    let mut reader = BufReader::new(file);

    let exif_reader = exif::Reader::new();
    let exif = exif_reader
        .read_from_container(&mut reader)
        .map_err(|e| AppError::unsupported(format!("No EXIF data found: {e}")))?;

    let mut data = HashMap::new();

    for field in exif.fields() {
        let tag_name = field.tag.to_string();
        let value = field.display_value().with_unit(&exif).to_string();
        data.insert(tag_name, value);
    }

    Ok(data)
}

/// RGB 히스토그램(채널별 256빈). 디코드 불가 포맷은 에러 → 프론트가 섹션을 숨긴다.
#[tauri::command]
pub async fn get_image_histogram(
    file_path: String,
) -> Result<crate::image_info::Histogram, AppError> {
    run_blocking("histogram", move || {
        crate::image_info::histogram_for_path(Path::new(&file_path))
    })
    .await
}

/// 파일 상세(크기/치수/색상/날짜/DPI/ICC). EXIF가 없어도 성공한다.
#[tauri::command]
pub async fn get_image_details(
    file_path: String,
) -> Result<crate::image_info::ImageDetails, AppError> {
    run_blocking("details", move || {
        crate::image_info::details_for_path(Path::new(&file_path))
    })
    .await
}

/// CBZ/ZIP 안의 ComicInfo.xml 메타데이터를 읽는다 (읽기 전용, 표시용).
/// XML이 없거나 CBZ/ZIP이 아니면 `None`을 돌려주고, 깨진 XML은 에러다.
#[tauri::command]
pub async fn get_comic_info(
    file_path: String,
) -> Result<Option<crate::comic_info::ComicInfo>, AppError> {
    run_blocking("comic info", move || get_comic_info_blocking(&file_path)).await
}

fn get_comic_info_blocking(
    file_path: &str,
) -> Result<Option<crate::comic_info::ComicInfo>, AppError> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    crate::comic_info::read_comic_info(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;
    use crate::commands::test_support::*;

    #[test]
    fn get_exif_data_rejects_missing_file() {
        let err = get_exif_data_impl("D:\\no-such-dir-commands\\nope.jpg").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
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
