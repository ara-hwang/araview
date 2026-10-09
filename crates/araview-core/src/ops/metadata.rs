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
