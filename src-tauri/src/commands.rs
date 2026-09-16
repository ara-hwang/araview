use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::Path;

use crate::app_error::{AppError, ErrorCode};
use crate::archive;
use crate::image::{is_archive_file, is_supported_file, DirectoryImages, ImageInfo};
use crate::process_temp::process_temp_dir;
use tauri::Manager;

/// asset 프로토콜 scope 매칭은 canonicalize된 요청 경로 기준이므로
/// 허용할 때도 canonicalize한 경로를 등록한다.
pub(crate) fn allow_asset_path(app: &tauri::AppHandle, path: &Path) -> Result<(), AppError> {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    app.asset_protocol_scope()
        .allow_file(&canonical)
        .map_err(|e| AppError::unknown(format!("Failed to allow asset path: {e}")))
}

#[tauri::command]
pub fn load_image(app: tauri::AppHandle, file_path: String) -> Result<ImageInfo, AppError> {
    let info = crate::image::load_viewable(Path::new(&file_path))?;
    allow_asset_path(&app, Path::new(&info.file_path))?;
    Ok(info)
}

#[tauri::command]
pub fn get_directory_images(
    file_path: String,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    let opts = options.unwrap_or_default();
    let path = Path::new(&file_path);
    let parent = path
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;

    let images = crate::dir_cache::get_sorted_images(parent, &opts)?;
    let paths: Vec<String> = images.into_iter().map(|e| e.path).collect();

    let current_index = paths.iter().position(|p| p == &file_path).unwrap_or(0);

    Ok(DirectoryImages {
        images: paths,
        current_index,
    })
}

/// 디렉토리 목록 정렬/수집 옵션 (프론트 settingsStore와 대응)
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirListOptions {
    #[serde(default)]
    pub sort_key: DirSortKey,
    #[serde(default)]
    pub descending: bool,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Default, serde::Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DirSortKey {
    #[default]
    Name,
    Date,
    Size,
}

// 드롭된 경로를 해석한다. 파일이면 그대로, 디렉토리면 내부의 첫 이미지 경로를 반환.
#[tauri::command]
pub fn resolve_dropped_path(path: String) -> Result<String, AppError> {
    let p = Path::new(&path);

    if !p.exists() {
        return Err(AppError::not_found("Path not found"));
    }

    if p.is_file() {
        return Ok(path);
    }

    if p.is_dir() {
        let entries = fs::read_dir(p)
            .map_err(|e| AppError::io("Failed to read directory", e, ErrorCode::Corrupt))?;

        let mut images: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() && is_supported_file(&entry_path) {
                if let Some(s) = entry_path.to_str() {
                    images.push(s.to_string());
                }
            }
        }

        if images.is_empty() {
            return Err(AppError::not_found("No images found in directory"));
        }

        images.sort_by_key(|s| s.to_lowercase());
        return Ok(images.remove(0));
    }

    Err(AppError::unsupported("Unsupported path type"))
}

#[tauri::command]
pub fn get_exif_data(file_path: String) -> Result<HashMap<String, String>, AppError> {
    let path = Path::new(&file_path);

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
pub fn get_image_histogram(file_path: String) -> Result<crate::image_info::Histogram, AppError> {
    crate::image_info::histogram_for_path(Path::new(&file_path))
}

/// 파일 상세(크기/치수/색상/날짜/DPI/ICC). EXIF가 없어도 성공한다.
#[tauri::command]
pub fn get_image_details(file_path: String) -> Result<crate::image_info::ImageDetails, AppError> {
    crate::image_info::details_for_path(Path::new(&file_path))
}

/// 아카이브(CBZ/CB7) 파일 내부의 이미지 엔트리 목록을 반환
#[tauri::command]
pub fn get_archive_images(file_path: String) -> Result<DirectoryImages, AppError> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    if !is_archive_file(path) {
        return Err(AppError::unsupported("Not an archive file"));
    }

    let images = archive::list_archive_images(path)?;

    if images.is_empty() {
        return Err(AppError::not_found("No images found in archive"));
    }

    Ok(DirectoryImages {
        images,
        current_index: 0,
    })
}

/// 아카이브에서 특정 엔트리를 추출하여 ImageInfo를 반환
/// entry_name은 get_archive_images에서 반환된 엔트리 이름
#[tauri::command]
pub fn load_archive_image(
    app: tauri::AppHandle,
    archive_path: String,
    entry_name: String,
) -> Result<ImageInfo, AppError> {
    let arch_path = Path::new(&archive_path);

    if !arch_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }

    let sub_dir = archive_sub_dir(arch_path)?;

    let extracted_path = archive::extract_archive_image(arch_path, &entry_name, &sub_dir)?;

    let info = crate::image::load_viewable(&extracted_path)?;
    allow_asset_path(&app, Path::new(&info.file_path))?;
    Ok(info)
}

fn archive_sub_dir(archive_path: &Path) -> Result<std::path::PathBuf, AppError> {
    let temp_dir = process_temp_dir()?;
    // 같은 stem을 가진 다른 아카이브가 임시 캐시를 공유하지 않도록
    // canonical 경로 + mtime + 크기로 하위 디렉터리를 구분한다.
    // 영속 파일명에 쓰이므로 안정 해시를 사용한다.
    let hash = crate::sidecar::file_identity_hash(archive_path, &[])?;
    let sub_dir = temp_dir.join(format!("archive-{hash:016x}"));
    // 새 아카이브를 처음 열 때만 전체 temp 상한을 강제한다. 재사용 시에는
    // 매번 전체를 훑지 않아 페이지 넘김/썸네일 비용을 늘리지 않는다.
    let is_new = !sub_dir.is_dir();
    fs::create_dir_all(&sub_dir)
        .map_err(|e| AppError::io("Failed to create sub dir", e, ErrorCode::Unknown))?;
    if is_new {
        // 전체 트리 순회라 파일 수가 많으면 수 초가 걸린다. best-effort이므로
        // 첫 페이지 렌더를 막지 않도록 백그라운드로 돌린다.
        std::thread::spawn(|| {
            crate::process_temp::enforce_total_cap(MAX_PROCESS_TEMP_BYTES).ok();
        });
    }
    Ok(sub_dir)
}

/// 프로세스 temp 전체 상한. 썸네일/paint/아카이브별 상한에 더해 여러 아카이브를
/// 연달아 열 때의 총량 팽창을 막는다.
const MAX_PROCESS_TEMP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// 이웃 페이지 선추출 (zip은 오픈 1회). FE fire-and-forget용으로 항상 Ok다.
#[tauri::command]
pub fn archive_prefetch(archive_path: String, entry_names: Vec<String>) -> Result<usize, AppError> {
    let arch_path = Path::new(&archive_path);
    if !arch_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }
    let sub_dir = archive_sub_dir(arch_path)?;
    Ok(archive::prefetch_archive_images(
        arch_path,
        &entry_names,
        &sub_dir,
    ))
}

#[tauri::command]
pub fn get_file_associations() -> Result<Vec<crate::file_assoc::FileAssociation>, AppError> {
    crate::file_assoc::list_associations()
}

#[tauri::command]
pub fn set_file_association(
    window: tauri::WebviewWindow,
    extension: String,
    associate: bool,
) -> Result<crate::file_assoc::FileAssociation, AppError> {
    crate::file_assoc::set_association(&extension, associate, window_hwnd(&window))
}

fn window_hwnd(window: &tauri::WebviewWindow) -> *mut std::ffi::c_void {
    window
        .hwnd()
        .map(|hwnd| hwnd.0)
        .unwrap_or(std::ptr::null_mut())
}

#[tauri::command]
pub fn set_all_file_associations(
    associate: bool,
) -> Result<Vec<crate::file_assoc::FileAssociation>, AppError> {
    crate::file_assoc::set_all_associations(associate)
}

#[tauri::command]
pub fn open_default_apps_settings() -> Result<(), AppError> {
    crate::file_assoc::open_default_apps_settings()
}

/// 썸네일 스트립용 축소 JPEG 경로 반환. 디코드 불가 시 에러 → 프론트는 원본으로 폴백.
#[tauri::command]
pub fn generate_thumbnail(
    file_path: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    crate::thumbnail::generate_thumbnail(
        Path::new(&file_path),
        max_side.unwrap_or_else(crate::thumbnail::default_max_side),
    )
}

/// 썸네일 윈도우 배치 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub fn generate_thumbnails_batch(
    file_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<crate::thumbnail::BatchThumb>, AppError> {
    Ok(crate::thumbnail::generate_thumbnails_batch(
        &file_paths,
        max_side.unwrap_or_else(crate::thumbnail::default_max_side),
    ))
}

/// 아카이브 엔트리 썸네일 생성 코어. AppHandle 없이 테스트 가능하다.
pub(crate) fn generate_archive_thumbnail_impl(
    archive_path: &Path,
    entry_name: &str,
    max_side: u32,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    if !archive_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }
    let sub_dir = archive_sub_dir(archive_path)?;
    let extracted = archive::extract_archive_image(archive_path, entry_name, &sub_dir)?;
    crate::thumbnail::generate_thumbnail(&extracted, max_side)
}

/// 아카이브 엔트리용 축소 JPEG 경로 반환. 추출물과 썸네일 캐시를 재사용하므로
/// 썸네일 그리드처럼 여러 엔트리를 동시에 볼 때 원본 풀사이즈 로드를 피한다.
#[tauri::command]
pub fn generate_archive_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    entry_name: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    let thumb = generate_archive_thumbnail_impl(
        Path::new(&archive_path),
        &entry_name,
        max_side.unwrap_or_else(crate::thumbnail::default_max_side),
    )?;
    allow_asset_path(&app, Path::new(&thumb.file_path))?;
    Ok(thumb)
}

/// 현재 이미지를 OS 휴지통으로 이동 (영구 삭제 아님)
#[tauri::command]
pub fn trash_file(file_path: String) -> Result<(), AppError> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    if !path.is_file() {
        return Err(AppError::invalid_input("Only files can be moved to trash"));
    }

    trash::delete(path).map_err(|e| AppError::unknown(format!("Failed to move to trash: {e}")))
}

/// 같은 폴더 안에서 파일 이름 변경. 새 ImageInfo를 반환해 이름/MIME/크기를 일괄 갱신
#[tauri::command]
pub fn rename_file(
    app: tauri::AppHandle,
    old_path: String,
    new_name: String,
) -> Result<ImageInfo, AppError> {
    let info = rename_file_impl(&old_path, &new_name)?;
    allow_asset_path(&app, Path::new(&info.file_path))?;
    Ok(info)
}

fn rename_file_impl(old_path: &str, new_name: &str) -> Result<ImageInfo, AppError> {
    use crate::image::is_supported_file;

    let old = Path::new(old_path);

    if !old.is_file() {
        return Err(AppError::not_found("File not found"));
    }

    let name = crate::image::validate_new_file_name(new_name)?;

    let parent = old
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;
    let new_path = parent.join(name);

    // 대소문자만 바꾸는 경우 등 동일 파일이면 이동 생략
    if new_path != old {
        if !is_supported_file(&new_path) {
            return Err(AppError::unsupported("Unsupported image format"));
        }
        if new_path.exists() {
            return Err(AppError::already_exists(
                "A file with that name already exists",
            ));
        }
        fs::rename(old, &new_path)
            .map_err(|e| AppError::io("Failed to rename", e, ErrorCode::Unknown))?;
    }

    crate::image::load_viewable(&new_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_error::ErrorCode;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_dir(suffix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("tiv-rename-{suffix}-{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn rename_rejects_missing_file() {
        let err = rename_file_impl("D:\\no-such-dir\\nope.png", "new.png").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "File not found");
    }

    #[test]
    fn rename_rejects_bad_names() {
        let dir = unique_dir("bad");
        let old = dir.join("a.png");
        fs::write(&old, []).expect("write dummy");
        let old_str = old.to_str().unwrap().to_string();

        for bad in ["", "   ", "sub/dir.png", "a<b.png", "trail."] {
            let err = rename_file_impl(&old_str, bad).unwrap_err();
            assert_eq!(
                err.code,
                ErrorCode::InvalidInput,
                "name {bad:?} should fail"
            );
            assert!(!err.message.is_empty());
        }
        // 실패한 검증 뒤 원본은 그대로
        assert!(old.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_rejects_unsupported_extension_before_moving() {
        let dir = unique_dir("ext");
        let old = dir.join("a.png");
        fs::write(&old, []).expect("write dummy");

        let err = rename_file_impl(old.to_str().unwrap(), "b.txt").unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
        assert_eq!(err.message, "Unsupported image format");
        // rename 전에 차단되므로 원본 유지
        assert!(old.exists());
        assert!(!dir.join("b.txt").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_rejects_duplicate() {
        let dir = unique_dir("dup");
        let old = dir.join("a.png");
        fs::write(&old, []).expect("write dummy");
        fs::write(dir.join("b.png"), []).expect("write dummy");

        let err = rename_file_impl(old.to_str().unwrap(), "b.png").unwrap_err();
        assert_eq!(err.code, ErrorCode::AlreadyExists);
        assert_eq!(err.message, "A file with that name already exists");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_success_returns_updated_info() {
        let dir = unique_dir("ok");
        let old = dir.join("a.png");
        fs::write(&old, [0u8; 16]).expect("write dummy");

        let info = rename_file_impl(old.to_str().unwrap(), "b.png").expect("rename ok");
        assert_eq!(info.file_name, "b.png");
        assert_eq!(info.file_size, 16);
        // 더미 바이트는 디코드 불가라 치수 생략
        assert_eq!(info.width, None);
        assert_eq!(info.height, None);
        assert!(!old.exists());
        assert!(dir.join("b.png").exists());
        fs::remove_dir_all(&dir).ok();
    }

    fn write_sized(dir: &std::path::Path, name: &str, size: usize) -> String {
        let path = dir.join(name);
        fs::write(&path, vec![0u8; size]).expect("write dummy");
        path.to_str().unwrap().to_string()
    }

    fn list_paths(file_path: &str, options: Option<DirListOptions>) -> Vec<String> {
        get_directory_images(file_path.to_string(), options)
            .expect("list ok")
            .images
    }

    fn file_names(paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .map(|p| {
                Path::new(p)
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn dir_list_default_is_name_ascending() {
        let dir = unique_dir("sort-default");
        let b = write_sized(&dir, "b.png", 10);
        write_sized(&dir, "a.png", 30);
        write_sized(&dir, "c.png", 20);

        let names = file_names(&list_paths(&b, None));
        assert_eq!(names, vec!["a.png", "b.png", "c.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_size_and_descending() {
        let dir = unique_dir("sort-size");
        let a = write_sized(&dir, "a.png", 30);
        write_sized(&dir, "b.png", 10);
        write_sized(&dir, "c.png", 20);

        let by_size = list_paths(
            &a,
            Some(DirListOptions {
                sort_key: DirSortKey::Size,
                ..Default::default()
            }),
        );
        assert_eq!(file_names(&by_size), vec!["b.png", "c.png", "a.png"]);

        let desc = list_paths(
            &a,
            Some(DirListOptions {
                descending: true,
                ..Default::default()
            }),
        );
        assert_eq!(file_names(&desc), vec!["c.png", "b.png", "a.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_date_orders_by_mtime() {
        let dir = unique_dir("sort-date");
        let old = write_sized(&dir, "old.png", 10);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let new = write_sized(&dir, "new.png", 10);

        let names = file_names(&list_paths(
            &old,
            Some(DirListOptions {
                sort_key: DirSortKey::Date,
                ..Default::default()
            }),
        ));
        assert_eq!(names, vec!["old.png", "new.png"]);

        let index = get_directory_images(
            new.clone(),
            Some(DirListOptions {
                sort_key: DirSortKey::Date,
                ..Default::default()
            }),
        )
        .expect("list ok")
        .current_index;
        assert_eq!(index, 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_shuffle_is_stable_per_directory() {
        let dir = unique_dir("shuffle");
        let a = write_sized(&dir, "a.png", 10);
        for name in [
            "b.png", "c.png", "d.png", "e.png", "f.png", "g.png", "h.png",
        ] {
            write_sized(&dir, name, 10);
        }
        let opts = || {
            Some(DirListOptions {
                shuffle: true,
                ..Default::default()
            })
        };

        // 같은 폴더는 반복 조회해도 같은 순서
        let first = list_paths(&a, opts());
        let second = list_paths(&a, opts());
        assert_eq!(first, second);
        // 이름순과는 다를 가능성이 극히 높음 (8! = 40320)
        let ordered = list_paths(&a, None);
        assert_ne!(first, ordered);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_recursive_includes_subfolders() {
        let dir = unique_dir("recursive");
        let top = write_sized(&dir, "top.png", 10);
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).expect("create sub");
        write_sized(&sub, "nested.png", 10);

        let flat = list_paths(&top, None);
        assert_eq!(flat.len(), 1);

        let all = list_paths(
            &top,
            Some(DirListOptions {
                recursive: true,
                ..Default::default()
            }),
        );
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|p| p.ends_with("nested.png")));
        fs::remove_dir_all(&dir).ok();
    }

    /// 실제 PNG 1장을 담은 CBZ 픽스처로 아카이브 썸네일 왕복을 검증한다.
    fn write_cbz_fixture(dir: &Path, file_name: &str) -> std::path::PathBuf {
        use std::io::Write as _;

        let png_path = dir.join("page.png");
        let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(200, 100, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
        }));
        img.save(&png_path).expect("write png fixture");

        let archive_path = dir.join(file_name);
        let file = fs::File::create(&archive_path).expect("create cbz");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer.start_file("page.png", options).expect("start entry");
        writer
            .write_all(&fs::read(&png_path).expect("read png"))
            .expect("write entry");
        writer.finish().expect("finish cbz");
        archive_path
    }

    #[test]
    fn archive_thumbnail_downscales_and_reuses_cache() {
        let dir = unique_dir("archive-thumb");
        let archive = write_cbz_fixture(&dir, "comic.cbz");

        let first = generate_archive_thumbnail_impl(&archive, "page.png", 64).expect("first");
        assert!(first.file_path.ends_with(".jpg"));
        assert_eq!((first.width, first.height), (64, 32));

        let second = generate_archive_thumbnail_impl(&archive, "page.png", 64).expect("second");
        assert_eq!(first.file_path, second.file_path);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn archive_thumbnail_missing_archive_is_not_found() {
        let err =
            generate_archive_thumbnail_impl(Path::new("no-such.cbz"), "page.png", 64).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }
}
