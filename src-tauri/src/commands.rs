use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use crate::app_error::{AppError, ErrorCode};
use crate::archive;
use crate::image::{is_archive_file, is_supported_file, DirectoryImages, ImageInfo};
use crate::process_temp::process_temp_dir;
use tauri::Manager;

/// 이미 허용한 asset 디렉터리. 파일마다 scope를 추가하면 세션 내내 팽창하므로
/// 부모 디렉터리 단위로 1회만 허용한다. 활성 파생 이미지 캐시 루트는 시작 시
/// 통째로 허용되므로 여기서 건너뛴다.
static ALLOWED_ASSET_DIRS: LazyLock<Mutex<HashSet<PathBuf>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

/// asset 프로토콜 scope 매칭은 canonicalize된 요청 경로 기준이므로
/// 허용할 때도 canonicalize한 경로를 등록한다.
pub(crate) fn allow_asset_path(app: &tauri::AppHandle, path: &Path) -> Result<(), AppError> {
    if let Ok(temp) = process_temp_dir() {
        let temp_canon = fs::canonicalize(&temp).unwrap_or(temp.clone());
        let path_canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if path_canon.starts_with(&temp_canon) || path.starts_with(&temp) {
            return Ok(());
        }
    }
    let parent = path.parent().unwrap_or(path);
    let canon_parent = fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    if ALLOWED_ASSET_DIRS
        .lock()
        .map(|set| set.contains(&canon_parent))
        .unwrap_or(false)
    {
        return Ok(());
    }
    // 재귀 허용은 금지한다. `D:\a.jpg`의 부모는 `D:\`라서 recursive=true면
    // 드라이브 전체가 webview에 열린다. 하위 폴더 이미지는 자기 파일을 열 때
    // 그 폴더가 개별로 허용되므로 재귀 없이도 충분하다.
    if app
        .asset_protocol_scope()
        .allow_directory(&canon_parent, false)
        .is_ok()
    {
        if let Ok(mut set) = ALLOWED_ASSET_DIRS.lock() {
            set.insert(canon_parent);
        }
        return Ok(());
    }
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    app.asset_protocol_scope()
        .allow_file(&canonical)
        .map_err(|e| AppError::unknown(format!("Failed to allow asset path: {e}")))
}

/// 블로킹 작업을 런타임 밖 스레드에서 실행한다. join 실패(패닉 등)는
/// `AppError::unknown("Failed to join <task> task: ...")`로 통일한다.
async fn run_blocking<T, F>(task: &'static str, f: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::unknown(format!("Failed to join {task} task: {e}")))?
}

#[tauri::command]
pub async fn load_image(
    app: tauri::AppHandle,
    file_path: String,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    // 동기 커맨드는 메인 스레드에서 실행된다. 디코드 같은 무거운 작업은
    // blocking 풀로 넘겨 창 이벤트 루프와 다른 커맨드를 막지 않는다.
    run_blocking("image load", move || {
        load_image_blocking(
            &app,
            &file_path,
            max_side,
            image_scaling_mode,
            auto_detect_pixel_art,
        )
    })
    .await
}

fn load_image_blocking(
    app: &tauri::AppHandle,
    file_path: &str,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    let mode = parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
    let info = crate::image::load_viewable_with_limit_mode(Path::new(file_path), max_side, mode)?;
    allow_asset_path(app, Path::new(&info.file_path))?;
    Ok(info)
}

fn parse_image_scaling_mode(
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> crate::scaled::ImageScalingMode {
    crate::scaled::ImageScalingMode::from_command(
        image_scaling_mode.as_deref(),
        auto_detect_pixel_art.unwrap_or(false),
    )
}

/// 이미지 표시에 사용할 픽셀 아트 힌트를 비동기로 분석한다.
/// 분석 실패는 이미지 열기 흐름에 영향을 주지 않도록 `uncertain`으로 안전하게 반환한다.
#[tauri::command]
pub async fn detect_pixel_art(
    file_path: String,
) -> Result<crate::pixel_art::PixelArtDetection, AppError> {
    run_blocking("pixel-art detection", move || {
        Ok(crate::pixel_art::detect_file(Path::new(&file_path)))
    })
    .await
}

#[tauri::command]
pub async fn get_directory_images(
    file_path: String,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    run_blocking("directory listing", move || {
        get_directory_images_impl(&file_path, options)
    })
    .await
}

fn get_directory_images_impl(
    file_path: &str,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    let opts = options.unwrap_or_default();
    let path = Path::new(file_path);
    let parent = path
        .parent()
        .ok_or_else(|| AppError::not_found("Cannot get parent directory"))?;

    let images = crate::dir_cache::get_sorted_images(parent, &opts)?;
    // 지원 형식 판정은 스캔 시점에 끝났다. 비재귀 목록은 폴더 mtime으로
    // 검증되므로 삭제가 이미 반영되어 있고, 재귀 목록은 워처 이벤트가
    // 도착하기 전 틈이 있어 그때만 존재 여부를 다시 확인한다.
    let recheck_exists = opts.recursive;
    let mut paths = Vec::with_capacity(images.len());
    let mut availability = Vec::with_capacity(images.len());
    for entry in images {
        if recheck_exists && !Path::new(&entry.path).is_file() {
            continue;
        }
        paths.push(entry.path);
        availability.push(entry.availability);
    }

    // 보통은 문자열이 그대로 일치한다. 대소문자나 8.3 단축 경로처럼 철자가
    // 다르게 들어온 경우에만 canonical 비교로 폴백한다.
    let current_index = index_of_current(&paths, path).unwrap_or(0);

    Ok(DirectoryImages {
        images: paths,
        current_index,
        availability,
    })
}

/// 대소문자만 다른 철자로 들어와도 현재 파일을 찾는다. 목록은 호출자가 준
/// 부모 경로로 스캔되므로 보통 첫 비교에서 끝난다. 폴백에서 경로마다
/// canonicalize를 돌리면 수천 장 폴더에서 syscall 폭주가 나므로 하지 않는다.
fn index_of_current(paths: &[String], current: &Path) -> Option<usize> {
    if let Some(pos) = paths.iter().position(|p| Path::new(p) == current) {
        return Some(pos);
    }
    let lowered = current.to_string_lossy().to_lowercase();
    paths.iter().position(|p| p.to_lowercase() == lowered)
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
pub async fn resolve_dropped_path(path: String) -> Result<String, AppError> {
    run_blocking("dropped path", move || resolve_dropped_path_impl(&path)).await
}

fn resolve_dropped_path_impl(path: &str) -> Result<String, AppError> {
    let p = Path::new(path);

    if !p.exists() {
        return Err(AppError::not_found("Path not found"));
    }

    if p.is_file() {
        return Ok(path.to_string());
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

        return images
            .into_iter()
            .min_by_key(|s| s.to_lowercase())
            .ok_or_else(|| AppError::not_found("No images found in directory"));
    }

    Err(AppError::unsupported("Unsupported path type"))
}

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

/// 아카이브(CBZ/ZIP) 파일 내부의 이미지 엔트리 목록을 반환
#[tauri::command]
pub async fn get_archive_images(file_path: String) -> Result<DirectoryImages, AppError> {
    run_blocking("archive listing", move || {
        get_archive_images_impl(Path::new(&file_path))
    })
    .await
}

fn get_archive_images_impl(path: &Path) -> Result<DirectoryImages, AppError> {
    if !path.exists() {
        return Err(AppError::not_found("File not found"));
    }

    if !is_archive_file(path) {
        return Err(AppError::unsupported("Not an archive file"));
    }

    // 인덱스 캐시로 조회한다. 오픈 흐름에서 comic_info·표지 커맨드가
    // 같은 스캔을 재사용하므로 아카이브당 파싱은 1회로 수렴한다.
    let entries = crate::archive_index::get_archive_entries(path)?;

    if entries.images.is_empty() {
        return Err(AppError::not_found("No images found in archive"));
    }

    Ok(DirectoryImages {
        images: Arc::unwrap_or_clone(entries.images),
        current_index: 0,
        availability: Vec::new(),
    })
}

/// 아카이브에서 특정 엔트리를 추출하여 ImageInfo를 반환
/// entry_name은 get_archive_images에서 반환된 엔트리 이름
#[tauri::command]
pub async fn load_archive_image(
    app: tauri::AppHandle,
    archive_path: String,
    entry_name: String,
    max_side: Option<u32>,
    protect: Option<bool>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    run_blocking("archive load", move || {
        load_archive_image_blocking(
            &app,
            &archive_path,
            &entry_name,
            max_side,
            protect,
            image_scaling_mode,
            auto_detect_pixel_art,
        )
    })
    .await
}

fn load_archive_image_blocking(
    app: &tauri::AppHandle,
    archive_path: &str,
    entry_name: &str,
    max_side: Option<u32>,
    protect: Option<bool>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    let arch_path = Path::new(archive_path);

    if !arch_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }

    let sub_dir = archive_sub_dir(arch_path)?;

    let extracted_path = archive::extract_archive_image_with_protection(
        arch_path,
        entry_name,
        &sub_dir,
        protect.unwrap_or(true),
    )?;

    let mode = parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
    let info = crate::image::load_viewable_with_limit_mode(&extracted_path, max_side, mode)?;
    allow_asset_path(app, Path::new(&info.file_path))?;
    Ok(info)
}

fn archive_sub_dir(archive_path: &Path) -> Result<std::path::PathBuf, AppError> {
    let temp_dir = process_temp_dir()?;
    // 같은 stem을 가진 다른 아카이브가 캐시를 공유하지 않도록 canonical 경로 +
    // mtime + 크기로 하위 디렉터리를 구분한다. 영속 파일명에 쓰이므로 안정
    // 해시와 캐시 포맷 revision을 사용한다.
    let hash = crate::sidecar::file_identity_hash(archive_path, &[])?;
    let sub_dir = temp_dir
        .join(crate::cache::ARCHIVES_SUBDIR)
        .join(format!("{hash:016x}"));
    // 새 아카이브를 처음 열 때만 전체 temp 상한을 강제한다. 재사용 시에는
    // 매번 전체를 훑지 않아 페이지 넘김/썸네일 비용을 늘리지 않는다.
    let is_new = !sub_dir.is_dir();
    fs::create_dir_all(&sub_dir)
        .map_err(|e| AppError::io("Failed to create sub dir", e, ErrorCode::Unknown))?;
    if is_new {
        // 전체 트리 순회라 파일 수가 많으면 수 초가 걸린다. 연속 요청은
        // 실행 중인 한 스윕으로 모아 중복 스캔과 detached 스레드를 막는다.
        crate::process_temp::request_total_cap_scan();
    }
    Ok(sub_dir)
}

/// 이웃 페이지 선추출 (zip은 오픈 1회). FE fire-and-forget용으로 항상 Ok다.
/// 조인 실패까지 흡수해 시크 지연을 만들지 않는다.
#[tauri::command]
pub async fn archive_prefetch(
    archive_path: String,
    entry_names: Vec<String>,
) -> Result<usize, AppError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        archive_prefetch_blocking(&archive_path, &entry_names)
    })
    .await;
    match result {
        Ok(value) => value,
        Err(e) => {
            log::warn!("[archive] prefetch join failed: {e}");
            Ok(0)
        }
    }
}

fn archive_prefetch_blocking(
    archive_path: &str,
    entry_names: &[String],
) -> Result<usize, AppError> {
    let arch_path = Path::new(archive_path);
    if !arch_path.exists() {
        return Err(AppError::not_found("Archive not found"));
    }
    let sub_dir = archive_sub_dir(arch_path)?;
    Ok(archive::prefetch_archive_images(
        arch_path,
        entry_names,
        &sub_dir,
    ))
}

#[tauri::command]
pub async fn get_file_associations() -> Result<Vec<crate::file_assoc::FileAssociation>, AppError> {
    run_blocking("associations", crate::file_assoc::list_associations).await
}

#[tauri::command]
pub async fn set_file_association(
    window: tauri::WebviewWindow,
    extension: String,
    associate: bool,
) -> Result<crate::file_assoc::FileAssociation, AppError> {
    run_blocking("association", move || {
        crate::file_assoc::set_association(&extension, associate, window_hwnd(&window))
    })
    .await
}

/// 소유자 창의 HWND 값(없으면 0). `file_assoc`가 raw 포인터를 safe
/// 시그니처로 받지 않도록 정수로 넘긴다.
fn window_hwnd(window: &tauri::WebviewWindow) -> isize {
    window.hwnd().map(|hwnd| hwnd.0 as isize).unwrap_or(0)
}

#[tauri::command]
pub async fn open_default_apps_settings() -> Result<(), AppError> {
    run_blocking(
        "default apps",
        crate::file_assoc::open_default_apps_settings,
    )
    .await
}

/// 설치 프로그램에 동봉된 서드파티 라이선스 문서(고지 문서 + 라이브러리 원문).
#[derive(serde::Serialize)]
pub struct LicenseDocument {
    pub name: String,
    pub content: String,
}

/// 라이선스 대화상자용 묶음: 문서 + 패키지별 라이선스 목록과 본문(id로 중복 제거).
/// `packages`/`texts`는 `THIRD_PARTY_LICENSES.json`(scripts/generate-license-data.mjs)을 그대로 넘긴다.
#[derive(serde::Serialize)]
pub struct LicenseBundle {
    pub documents: Vec<LicenseDocument>,
    pub packages: serde_json::Value,
    pub texts: serde_json::Value,
}

const LICENSE_DATA_FILE: &str = "THIRD_PARTY_LICENSES.json";

/// 동봉된 라이선스 자료를 읽어 돌려준다. 고지 문서가 문서 목록 맨 앞, 나머지는 이름순.
#[tauri::command]
pub async fn get_license_bundle(app: tauri::AppHandle) -> Result<LicenseBundle, AppError> {
    use tauri::Manager;

    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| AppError::unknown(format!("Failed to resolve resource dir: {e}")))?
        .join("licenses");
    run_blocking("licenses", move || {
        let entries = std::fs::read_dir(&dir).map_err(|e| {
            AppError::io(
                &format!("read licenses dir {}", dir.display()),
                e,
                crate::app_error::ErrorCode::NotFound,
            )
        })?;
        let mut docs: Vec<LicenseDocument> = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter(|entry| entry.file_name().to_string_lossy() != LICENSE_DATA_FILE)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let bytes = std::fs::read(entry.path()).ok()?;
                Some(LicenseDocument {
                    name,
                    content: String::from_utf8_lossy(&bytes).into_owned(),
                })
            })
            .collect();
        docs.sort_by(|a, b| {
            let notice = |d: &LicenseDocument| d.name != "THIRD_PARTY_LICENSES.md";
            (notice(a), a.name.to_lowercase()).cmp(&(notice(b), b.name.to_lowercase()))
        });
        if docs.is_empty() {
            return Err(AppError::not_found("No license documents are bundled"));
        }

        // 패키지 데이터가 없거나 깨져도 문서는 보여준다.
        let data: serde_json::Value = std::fs::read(dir.join(LICENSE_DATA_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(serde_json::Value::Null);
        Ok(LicenseBundle {
            documents: docs,
            packages: data
                .get("packages")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
            texts: data
                .get("texts")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({})),
        })
    })
    .await
}

/// 썸네일 스트립용 축소 JPEG 경로 반환. 디코드 불가 시 에러 → 프론트는 원본으로 폴백.
#[tauri::command]
pub async fn generate_thumbnail(
    file_path: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    run_blocking("thumbnail", move || {
        crate::thumbnail::generate_thumbnail(
            Path::new(&file_path),
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        )
    })
    .await
}

/// 캐시에 이미 있는 썸네일만 반환한다. 없으면 null이며 프론트는 바로 원본을 그린다.
#[tauri::command]
pub async fn get_cached_thumbnail(file_path: String) -> Option<crate::thumbnail::ThumbnailInfo> {
    // 캐시 조회만 하므로 조인 실패도 캐시 미스로 취급한다(폴백은 원본 렌더).
    tauri::async_runtime::spawn_blocking(move || {
        crate::thumbnail::cached_thumbnail(Path::new(&file_path))
    })
    .await
    .unwrap_or_default()
}

/// 현재 활성 이미지 캐시의 종류별 사용량. 큰 트리 조회는 UI를 막지 않도록
/// blocking task 안에서 실행한다.
#[tauri::command]
pub async fn get_cache_stats() -> Result<crate::cache::CacheStats, AppError> {
    run_blocking("cache stats", crate::cache::get_cache_stats).await
}

/// 종류별 또는 전체 캐시 삭제. 화면에 전달된 in-use 파일은 보존한다.
#[tauri::command]
pub async fn clear_cache(
    scope: crate::cache::CacheScope,
) -> Result<crate::cache::CacheClearResult, AppError> {
    run_blocking("cache clear", move || crate::cache::clear_cache(scope)).await
}

/// 썸네일 윈도우 배치 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub async fn generate_thumbnails_batch(
    file_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<crate::thumbnail::BatchThumb>, AppError> {
    run_blocking("thumbnail batch", move || {
        Ok(crate::thumbnail::generate_thumbnails_batch(
            &file_paths,
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        ))
    })
    .await
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
pub async fn generate_archive_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    entry_name: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    run_blocking("archive thumbnail", move || {
        generate_archive_thumbnail_blocking(
            &app,
            Path::new(&archive_path),
            &entry_name,
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        )
    })
    .await
}

fn generate_archive_thumbnail_blocking(
    app: &tauri::AppHandle,
    archive_path: &Path,
    entry_name: &str,
    max_side: u32,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    let thumb = generate_archive_thumbnail_impl(archive_path, entry_name, max_side)?;
    allow_asset_path(app, Path::new(&thumb.file_path))?;
    Ok(thumb)
}

/// 폴더 목록에 있는 아카이브 파일(.cbz 등)의 표지 썸네일. 첫 이미지 엔트리를 사용한다.
#[tauri::command]
pub async fn generate_archive_file_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    max_side: Option<u32>,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    run_blocking("archive file thumbnail", move || {
        generate_archive_file_thumbnail_blocking(
            &app,
            Path::new(&archive_path),
            max_side.unwrap_or_else(crate::thumbnail::default_max_side),
        )
    })
    .await
}

fn generate_archive_file_thumbnail_blocking(
    app: &tauri::AppHandle,
    archive_path: &Path,
    max_side: u32,
) -> Result<crate::thumbnail::ThumbnailInfo, AppError> {
    // 인덱스 캐시로 조회한다. 폴더 표지 배치는 같은 아카이브를 여러 커맨드
    // (목록+표지)에서 건드리므로 파싱이 1회로 수렴한다.
    let entries = crate::archive_index::get_archive_entries(archive_path)?;
    let first = entries
        .images
        .first()
        .ok_or_else(|| AppError::not_found("No images in archive"))?;
    let thumb = generate_archive_thumbnail_impl(archive_path, first, max_side)?;
    allow_asset_path(app, Path::new(&thumb.file_path))?;
    Ok(thumb)
}

/// 폴더 목록의 아카이브 표지 썸네일 일괄 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub async fn generate_archive_file_thumbnails_batch(
    app: tauri::AppHandle,
    archive_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<crate::thumbnail::BatchThumb>, AppError> {
    run_blocking("archive cover batch", move || {
        let max_side = max_side.unwrap_or_else(crate::thumbnail::default_max_side);
        Ok(crate::thumbnail::map_with_workers(&archive_paths, |path| {
            match generate_archive_file_thumbnail_blocking(&app, Path::new(path), max_side) {
                Ok(thumb) => crate::thumbnail::BatchThumb {
                    source: path.clone(),
                    thumb: Some(thumb),
                    error: None,
                },
                Err(e) => crate::thumbnail::BatchThumb {
                    source: path.clone(),
                    thumb: None,
                    error: Some(e.message),
                },
            }
        }))
    })
    .await
}

/// 현재 이미지를 OS 휴지통으로 이동 (영구 삭제 아님)
#[tauri::command]
pub async fn trash_file(file_path: String) -> Result<(), AppError> {
    run_blocking("trash", move || trash_file_impl(&file_path)).await
}

fn trash_file_impl(file_path: &str) -> Result<(), AppError> {
    let path = Path::new(file_path);

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
pub async fn rename_file(
    app: tauri::AppHandle,
    old_path: String,
    new_name: String,
    max_side: Option<u32>,
    image_scaling_mode: Option<String>,
    auto_detect_pixel_art: Option<bool>,
) -> Result<ImageInfo, AppError> {
    run_blocking("rename", move || {
        let mode = parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
        let info = rename_file_impl_with_mode(&old_path, &new_name, max_side, mode)?;
        allow_asset_path(&app, Path::new(&info.file_path))?;
        Ok(info)
    })
    .await
}

#[cfg(test)]
fn rename_file_impl(
    old_path: &str,
    new_name: &str,
    max_side: Option<u32>,
) -> Result<ImageInfo, AppError> {
    rename_file_impl_with_mode(
        old_path,
        new_name,
        max_side,
        crate::scaled::ImageScalingMode::Smooth,
    )
}

fn rename_file_impl_with_mode(
    old_path: &str,
    new_name: &str,
    max_side: Option<u32>,
    mode: crate::scaled::ImageScalingMode,
) -> Result<ImageInfo, AppError> {
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

    crate::image::load_viewable_with_limit_mode(&new_path, max_side, mode)
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
        let dir = std::env::temp_dir().join(format!(
            "tiv-rename-{suffix}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn rename_rejects_missing_file() {
        let err = rename_file_impl("D:\\no-such-dir\\nope.png", "new.png", None).unwrap_err();
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
            let err = rename_file_impl(&old_str, bad, None).unwrap_err();
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

        let err = rename_file_impl(old.to_str().unwrap(), "b.txt", None).unwrap_err();
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

        let err = rename_file_impl(old.to_str().unwrap(), "b.png", None).unwrap_err();
        assert_eq!(err.code, ErrorCode::AlreadyExists);
        assert_eq!(err.message, "A file with that name already exists");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_success_returns_updated_info() {
        let dir = unique_dir("ok");
        let old = dir.join("a.png");
        fs::write(&old, [0u8; 16]).expect("write dummy");

        let info = rename_file_impl(old.to_str().unwrap(), "b.png", None).expect("rename ok");
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
        get_directory_images_impl(file_path, options)
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

        let index = get_directory_images_impl(
            &new,
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
    fn dir_list_excludes_subdirectory_entries() {
        let dir = unique_dir("no-subdir-rows");
        let top = write_sized(&dir, "top.png", 10);
        let sub = dir.join("nested");
        fs::create_dir_all(&sub).expect("create sub");
        write_sized(&sub, "inner.png", 10);

        let names = file_names(&list_paths(&top, None));
        assert_eq!(names, vec!["top.png"]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dir_list_drops_file_deleted_after_cached_listing() {
        let dir = unique_dir("deleted-after-cache");
        let a = write_sized(&dir, "a.png", 10);
        let b = write_sized(&dir, "b.png", 10);
        assert_eq!(file_names(&list_paths(&a, None)), vec!["a.png", "b.png"]);

        // 비재귀 목록은 존재 여부를 다시 확인하지 않으므로 삭제가 폴더
        // mtime 변경으로 캐시를 무효화해야 한다.
        fs::remove_file(&b).expect("remove b");
        assert_eq!(file_names(&list_paths(&a, None)), vec!["a.png"]);
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

    #[test]
    fn archive_file_cover_matches_first_listed_entry() {
        let dir = unique_dir("archive-file-thumb");
        let archive = write_cbz_fixture(&dir, "comic.cbz");
        let entries = archive::list_archive_images(&archive).expect("list");
        let first = entries.first().expect("entry");
        let cover = generate_archive_thumbnail_impl(&archive, first, 64).expect("cover");
        let direct = generate_archive_thumbnail_impl(&archive, "page.png", 64).expect("page");
        assert_eq!(cover.file_path, direct.file_path);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn index_of_current_matches_exact_then_case_insensitive() {
        let paths = vec!["/pics/a.png".to_string(), "/pics/b.png".to_string()];
        assert_eq!(index_of_current(&paths, Path::new("/pics/b.png")), Some(1));
        assert_eq!(index_of_current(&paths, Path::new("/PICS/B.PNG")), Some(1));
        assert_eq!(index_of_current(&paths, Path::new("/pics/z.png")), None);
        assert_eq!(index_of_current(&[], Path::new("/pics/a.png")), None);
    }

    #[test]
    fn dir_list_options_default_to_name_ascending() {
        let opts = DirListOptions::default();
        assert_eq!(opts.sort_key, DirSortKey::Name);
        assert!(!opts.descending);
        assert!(!opts.recursive);
    }

    #[test]
    fn resolve_dropped_path_rejects_missing() {
        let err = resolve_dropped_path_impl("D:\\no-such-dir-commands\\nope.png").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn resolve_dropped_path_returns_file_as_is() {
        let dir = unique_dir("drop-file");
        let file = write_sized(&dir, "photo.png", 8);
        let resolved = resolve_dropped_path_impl(&file).expect("file ok");
        assert_eq!(resolved, file);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_dropped_path_returns_first_image_in_dir() {
        let dir = unique_dir("drop-dir");
        write_sized(&dir, "b.png", 8);
        write_sized(&dir, "a.png", 8);
        write_sized(&dir, "notes.txt", 8);
        let resolved = resolve_dropped_path_impl(dir.to_str().unwrap()).expect("dir ok");
        assert!(resolved.ends_with("a.png"), "got {resolved}");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resolve_dropped_path_rejects_dir_without_images() {
        let dir = unique_dir("drop-empty");
        write_sized(&dir, "notes.txt", 8);
        let err = resolve_dropped_path_impl(dir.to_str().unwrap()).unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn trash_file_rejects_missing_and_directories() {
        let err = trash_file_impl("D:\\no-such-dir-commands\\nope.png").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);

        let dir = unique_dir("trash-dir");
        let err = trash_file_impl(dir.to_str().unwrap()).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn get_exif_data_rejects_missing_file() {
        let err = get_exif_data_impl("D:\\no-such-dir-commands\\nope.jpg").unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    fn get_archive_images_rejects_missing_and_non_archive() {
        let err = match get_archive_images_impl(Path::new("D:\\no-such-dir-commands\\nope.cbz")) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code, ErrorCode::NotFound);

        let dir = unique_dir("archive-not");
        let png = write_sized(&dir, "a.png", 16);
        let err = match get_archive_images_impl(Path::new(&png)) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code, ErrorCode::Unsupported);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn get_archive_images_rejects_archive_without_images() {
        use std::io::Write as _;

        let dir = unique_dir("archive-noimg");
        let archive_path = dir.join("empty.cbz");
        let file = fs::File::create(&archive_path).expect("create cbz");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        writer
            .start_file("notes.txt", options)
            .expect("start entry");
        writer.write_all(b"hello").expect("write entry");
        writer.finish().expect("finish cbz");

        let err = match get_archive_images_impl(Path::new(archive_path.to_str().unwrap())) {
            Ok(_) => panic!("expected error"),
            Err(e) => e,
        };
        assert_eq!(err.code, ErrorCode::NotFound);
        fs::remove_dir_all(&dir).ok();
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
