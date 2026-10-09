//! 프런트엔드가 `invoke()`로 부르는 Tauri 커맨드. 구현은 `araview_core::ops`와
//! 도메인 모듈에 있고, 여기서는 blocking 풀 전환과 asset scope 허용만 맡는다.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use araview_core::app_error::AppError;
use araview_core::cache::{CacheClearResult, CacheScope, CacheStats};
use araview_core::clipboard_png::ClipboardPng;
use araview_core::comic_info::ComicInfo;
use araview_core::file_assoc::FileAssociation;
use araview_core::image::{ArchiveImages, DirectoryImages, ImageInfo};
use araview_core::image_info::{Histogram, ImageDetails};
use araview_core::ops::{self, DirListOptions, LicenseBundle};
use araview_core::pixel_art::PixelArtDetection;
use araview_core::process_temp::process_temp_dir;
use araview_core::thumb_shell::{self, PsdThumbStatus};
use araview_core::thumbnail::{self, BatchThumb, ThumbnailInfo};
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
        // 캐시 산출물은 대부분 루트 경로 그대로 만들어져 문자열 비교로 끝난다.
        if path.starts_with(&temp) {
            return Ok(());
        }
        let temp_canon = fs::canonicalize(&temp).unwrap_or(temp.clone());
        let path_canon = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if path_canon.starts_with(&temp_canon) {
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

fn max_side_or_default(max_side: Option<u32>) -> u32 {
    max_side.unwrap_or_else(thumbnail::default_max_side)
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
        let info = ops::load_image_blocking(
            &file_path,
            max_side,
            image_scaling_mode,
            auto_detect_pixel_art,
        )?;
        allow_asset_path(&app, Path::new(&info.file_path))?;
        Ok(info)
    })
    .await
}

/// 이미지 표시에 사용할 픽셀 아트 힌트를 비동기로 분석한다.
/// 분석 실패는 이미지 열기 흐름에 영향을 주지 않도록 `uncertain`으로 안전하게 반환한다.
#[tauri::command]
pub async fn detect_pixel_art(file_path: String) -> Result<PixelArtDetection, AppError> {
    run_blocking("pixel-art detection", move || {
        Ok(araview_core::pixel_art::detect_file(Path::new(&file_path)))
    })
    .await
}

#[tauri::command]
pub async fn get_directory_images(
    file_path: String,
    options: Option<DirListOptions>,
) -> Result<DirectoryImages, AppError> {
    run_blocking("directory listing", move || {
        ops::get_directory_images_impl(&file_path, options)
    })
    .await
}

/// 드롭된 경로를 해석한다. 파일이면 그대로, 디렉토리면 내부의 첫 이미지 경로를 반환.
#[tauri::command]
pub async fn resolve_dropped_path(path: String) -> Result<String, AppError> {
    run_blocking("dropped path", move || {
        ops::resolve_dropped_path_impl(&path)
    })
    .await
}

#[tauri::command]
pub async fn get_exif_data(file_path: String) -> Result<HashMap<String, String>, AppError> {
    run_blocking("exif", move || ops::get_exif_data_impl(&file_path)).await
}

/// RGB 히스토그램(채널별 256빈). 디코드 불가 포맷은 에러 → 프론트가 섹션을 숨긴다.
#[tauri::command]
pub async fn get_image_histogram(file_path: String) -> Result<Histogram, AppError> {
    run_blocking("histogram", move || {
        araview_core::image_info::histogram_for_path(Path::new(&file_path))
    })
    .await
}

/// 파일 상세(크기/치수/색상/날짜/DPI/ICC). EXIF가 없어도 성공한다.
#[tauri::command]
pub async fn get_image_details(file_path: String) -> Result<ImageDetails, AppError> {
    run_blocking("details", move || {
        araview_core::image_info::details_for_path(Path::new(&file_path))
    })
    .await
}

/// CBZ/ZIP 안의 ComicInfo.xml 메타데이터를 읽는다.
/// XML이 없거나 CBZ/ZIP이 아니면 `None`을 돌려주고, 깨진 XML은 에러다.
#[tauri::command]
pub async fn get_comic_info(file_path: String) -> Result<Option<ComicInfo>, AppError> {
    run_blocking("comic info", move || {
        ops::get_comic_info_blocking(&file_path)
    })
    .await
}

/// 표지 페이지(`FrontCover`) 집합을 ComicInfo.xml에 쓴다. 원본 아카이브를
/// 고쳐 쓰고, 반영된 메타데이터를 돌려준다.
#[tauri::command]
pub async fn set_comic_cover_pages(
    file_path: String,
    cover_pages: Vec<u32>,
) -> Result<ComicInfo, AppError> {
    run_blocking("comic cover", move || {
        ops::set_comic_cover_pages_impl(&file_path, &cover_pages)
    })
    .await
}

/// 아카이브(CBZ/ZIP) 파일 내부의 이미지 엔트리 목록을 반환
#[tauri::command]
pub async fn get_archive_images(file_path: String) -> Result<ArchiveImages, AppError> {
    run_blocking("archive listing", move || {
        ops::get_archive_images_impl(Path::new(&file_path))
    })
    .await
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
        let info = ops::load_archive_image_blocking(
            &archive_path,
            &entry_name,
            max_side,
            protect,
            image_scaling_mode,
            auto_detect_pixel_art,
        )?;
        allow_asset_path(&app, Path::new(&info.file_path))?;
        Ok(info)
    })
    .await
}

/// 이웃 페이지 선추출 (zip은 오픈 1회). FE fire-and-forget용으로 항상 Ok다.
/// 조인 실패까지 흡수해 시크 지연을 만들지 않는다.
#[tauri::command]
pub async fn archive_prefetch(
    archive_path: String,
    entry_names: Vec<String>,
) -> Result<usize, AppError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        ops::archive_prefetch_blocking(&archive_path, &entry_names)
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

/// 썸네일 스트립용 축소 JPEG 경로 반환. 디코드 불가 시 에러 → 프론트는 원본으로 폴백.
#[tauri::command]
pub async fn generate_thumbnail(
    file_path: String,
    max_side: Option<u32>,
) -> Result<ThumbnailInfo, AppError> {
    run_blocking("thumbnail", move || {
        thumbnail::generate_thumbnail(Path::new(&file_path), max_side_or_default(max_side))
    })
    .await
}

/// 캐시에 이미 있는 썸네일만 반환한다. 없으면 null이며 프론트는 바로 원본을 그린다.
#[tauri::command]
pub async fn get_cached_thumbnail(file_path: String) -> Option<ThumbnailInfo> {
    // 캐시 조회만 하므로 조인 실패도 캐시 미스로 취급한다(폴백은 원본 렌더).
    tauri::async_runtime::spawn_blocking(move || thumbnail::cached_thumbnail(Path::new(&file_path)))
        .await
        .unwrap_or_default()
}

/// 썸네일 윈도우 배치 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub async fn generate_thumbnails_batch(
    file_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<BatchThumb>, AppError> {
    run_blocking("thumbnail batch", move || {
        Ok(thumbnail::generate_thumbnails_batch(
            &file_paths,
            max_side_or_default(max_side),
        ))
    })
    .await
}

/// 현재 활성 이미지 캐시의 종류별 사용량. 큰 트리 조회는 UI를 막지 않도록
/// blocking task 안에서 실행한다.
#[tauri::command]
pub async fn get_cache_stats() -> Result<CacheStats, AppError> {
    run_blocking("cache stats", araview_core::cache::get_cache_stats).await
}

/// 종류별 또는 전체 캐시 삭제. 화면에 전달된 in-use 파일은 보존한다.
#[tauri::command]
pub async fn clear_cache(scope: CacheScope) -> Result<CacheClearResult, AppError> {
    run_blocking("cache clear", move || {
        araview_core::cache::clear_cache(scope)
    })
    .await
}

/// 아카이브 엔트리용 축소 JPEG 경로 반환. 추출물과 썸네일 캐시를 재사용하므로
/// 썸네일 그리드처럼 여러 엔트리를 동시에 볼 때 원본 풀사이즈 로드를 피한다.
#[tauri::command]
pub async fn generate_archive_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    entry_name: String,
    max_side: Option<u32>,
) -> Result<ThumbnailInfo, AppError> {
    run_blocking("archive thumbnail", move || {
        let thumb = ops::generate_archive_thumbnail_impl(
            Path::new(&archive_path),
            &entry_name,
            max_side_or_default(max_side),
        )?;
        allow_asset_path(&app, Path::new(&thumb.file_path))?;
        Ok(thumb)
    })
    .await
}

/// 배치 결과에서 asset scope 허용에 실패한 항목을 실패 항목으로 바꾼다.
fn allow_batch_thumbs(app: &tauri::AppHandle, thumbs: Vec<BatchThumb>) -> Vec<BatchThumb> {
    thumbs
        .into_iter()
        .map(|item| match &item.thumb {
            Some(thumb) => match allow_asset_path(app, Path::new(&thumb.file_path)) {
                Ok(()) => item,
                Err(e) => BatchThumb::from_result(&item.source, Err(e)),
            },
            None => item,
        })
        .collect()
}

/// 썸네일 그리드/스트립의 아카이브 엔트리 창을 1회 invoke로 처리한다. 항목별
/// 성공/실패를 함께 반환하고, 실패 항목은 FE가 원본 로드로 폴백한다.
#[tauri::command]
pub async fn generate_archive_thumbnails_batch(
    app: tauri::AppHandle,
    archive_path: String,
    entry_names: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<BatchThumb>, AppError> {
    run_blocking("archive thumbnail batch", move || {
        let thumbs = ops::generate_archive_thumbnails_batch_impl(
            Path::new(&archive_path),
            &entry_names,
            max_side_or_default(max_side),
        )?;
        Ok(allow_batch_thumbs(&app, thumbs))
    })
    .await
}

/// 폴더 목록에 있는 아카이브 파일(.cbz 등)의 표지 썸네일. 첫 이미지 엔트리를 사용한다.
#[tauri::command]
pub async fn generate_archive_file_thumbnail(
    app: tauri::AppHandle,
    archive_path: String,
    max_side: Option<u32>,
) -> Result<ThumbnailInfo, AppError> {
    run_blocking("archive file thumbnail", move || {
        let thumb = ops::generate_archive_file_thumbnail_blocking(
            Path::new(&archive_path),
            max_side_or_default(max_side),
        )?;
        allow_asset_path(&app, Path::new(&thumb.file_path))?;
        Ok(thumb)
    })
    .await
}

/// 폴더 목록의 아카이브 표지 썸네일 일괄 처리. 항목별 성공/실패를 함께 반환한다.
#[tauri::command]
pub async fn generate_archive_file_thumbnails_batch(
    app: tauri::AppHandle,
    archive_paths: Vec<String>,
    max_side: Option<u32>,
) -> Result<Vec<BatchThumb>, AppError> {
    run_blocking("archive cover batch", move || {
        let thumbs = ops::generate_archive_file_thumbnails_batch_impl(
            &archive_paths,
            max_side_or_default(max_side),
        );
        Ok(allow_batch_thumbs(&app, thumbs))
    })
    .await
}

#[tauri::command]
pub async fn get_file_associations() -> Result<Vec<FileAssociation>, AppError> {
    run_blocking("associations", araview_core::file_assoc::list_associations).await
}

#[tauri::command]
pub async fn set_file_association(
    window: tauri::WebviewWindow,
    extension: String,
    associate: bool,
) -> Result<FileAssociation, AppError> {
    run_blocking("association", move || {
        araview_core::file_assoc::set_association(&extension, associate, window_hwnd(&window))
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
        araview_core::file_assoc::open_default_apps_settings,
    )
    .await
}

/// 동봉된 라이선스 자료를 읽어 돌려준다. 고지 문서가 문서 목록 맨 앞, 나머지는 이름순.
#[tauri::command]
pub async fn get_license_bundle(app: tauri::AppHandle) -> Result<LicenseBundle, AppError> {
    let dir = app
        .path()
        .resource_dir()
        .map_err(|e| AppError::unknown(format!("Failed to resolve resource dir: {e}")))?
        .join("licenses");
    run_blocking("licenses", move || ops::license_bundle_in(&dir)).await
}

#[tauri::command]
pub fn get_psd_thumbnail_status() -> Result<PsdThumbStatus, AppError> {
    thumb_shell::status()
}

#[tauri::command]
pub fn register_psd_thumbnail() -> Result<PsdThumbStatus, AppError> {
    thumb_shell::register()
}

#[tauri::command]
pub fn unregister_psd_thumbnail() -> Result<PsdThumbStatus, AppError> {
    thumb_shell::unregister()
}

/// 현재 이미지를 OS 휴지통으로 이동 (영구 삭제 아님)
#[tauri::command]
pub async fn trash_file(file_path: String) -> Result<(), AppError> {
    run_blocking("trash", move || ops::trash_file_impl(&file_path)).await
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
        let mode = ops::parse_image_scaling_mode(image_scaling_mode, auto_detect_pixel_art);
        let info = ops::rename_file_impl_with_mode(&old_path, &new_name, max_side, mode)?;
        allow_asset_path(&app, Path::new(&info.file_path))?;
        Ok(info)
    })
    .await
}

/// 현재 이미지를 클립보드 복사용 PNG로 변환해 경로를 돌려준다(§11.3).
/// 디코드·인코딩은 백엔드가 맡고, 클립보드 쓰기는 프론트가 한다.
#[tauri::command]
pub async fn export_clipboard_png(
    app: tauri::AppHandle,
    file_path: String,
) -> Result<ClipboardPng, AppError> {
    run_blocking("clipboard png", move || {
        let png = araview_core::clipboard_png::export_clipboard_png(Path::new(&file_path))?;
        // 발행 경로는 파생 캐시 루트 아래라 대부분 이미 허용돼 있다. 세션 폴백
        // 루트 등 경로 차이에 대비해 멱등하게 한 번 더 거친다.
        allow_asset_path(&app, Path::new(&png.file_path))?;
        Ok(png)
    })
    .await
}
