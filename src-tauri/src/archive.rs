use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::app_error::{AppError, ErrorCode};
use crate::image::is_image_file;

/// 이미지 엔트리 + 전체 엔트리(ComicInfo 탐색용)를 한 번의 스캔으로 모은다.
#[derive(Debug, Clone)]
pub struct ArchiveEntries {
    /// 뷰어가 페이지로 넘기는 이미지 엔트리(정렬됨).
    pub images: Arc<Vec<String>>,
    /// 디렉터리·숨김 파일을 제외한 모든 엔트리(정렬됨). ComicInfo.xml 같은
    /// 비-이미지 파일을 포함한다.
    pub all: Arc<Vec<String>>,
}

/// 엔트리 전체 경로를 해시한 prefix로 임시 추출 경로를 고유화한다.
/// 하위 폴더가 다른 동명 엔트리(a/001.jpg, b/001.jpg)가 같은 basename으로
/// 겹쳐 서로를 덮어쓰는 문제를 막는다. 이웃 선추출이 병렬로 돌 때 필수.
/// 해시는 영속 파일명에 쓰이므로 안정 해시(FNV-1a)를 사용한다.
fn extraction_out_path(temp_dir: &Path, entry_name: &str) -> PathBuf {
    let file_name = Path::new(entry_name)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image");
    let hash = crate::sidecar::entry_name_hash(entry_name);
    temp_dir.join(format!("{hash:016x}_{file_name}"))
}

/// 단일 엔트리 압축 해제 상한 (zipbomb 가드). 초과 시 에러로 중단한다.
pub const MAX_ENTRY_BYTES: u64 = 200 * 1024 * 1024;

/// 아카이브별 추출 디렉터리 상한. 초과분은 가장 오래된 추출물부터 지운다.
/// 아직 화면에 있는 추출물은 `mark_in_use`로 보호되고(FE가 이미 asset URL을
/// 들고 있어 재추출 계기가 없다), 나머지는 다음 접근 시 다시 추출된다.
const MAX_ARCHIVE_DIR_BYTES: u64 = 1024 * 1024 * 1024;

/// 아카이브 내부의 이미지 엔트리 이름을 정렬된 순서로 반환
pub fn list_archive_images(archive_path: &Path) -> Result<Vec<String>, AppError> {
    let ext = archive_ext(archive_path);

    match ext.as_str() {
        "cbz" | "zip" => list_zip_images(archive_path),
        _ => Err(AppError::unsupported("Unsupported archive format")),
    }
}

/// 한 번의 스캔으로 이미지 + 전체 엔트리를 모두 수집한다.
/// `archive_index` 캐시가 이 함수의 결과를 저장해 재스캔을 피한다.
pub fn list_archive_entries(archive_path: &Path) -> Result<ArchiveEntries, AppError> {
    let ext = archive_ext(archive_path);

    match ext.as_str() {
        "cbz" | "zip" => collect_zip_entries(archive_path),
        _ => Err(AppError::unsupported("Unsupported archive format")),
    }
}

/// 수집된 (이미지, 전체) 엔트리 쌍을 공통 뒤처리한다: 정렬 + dedup + Arc.
fn finish_entries(mut images: Vec<String>, mut all: Vec<String>) -> ArchiveEntries {
    images.sort_by_cached_key(|n| n.to_lowercase());
    images.dedup();
    all.sort_by_cached_key(|n| n.to_lowercase());
    all.dedup();
    ArchiveEntries {
        images: Arc::new(images),
        all: Arc::new(all),
    }
}

/// 아카이브에서 특정 엔트리를 임시 파일로 추출하고 경로를 반환
pub fn extract_archive_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, AppError> {
    extract_archive_image_with_protection(archive_path, entry_name, temp_dir, true)
}

/// 보호 여부를 지정하는 아카이브 추출 진입점. 선확충/선로딩은 `false`로
/// 호출해 화면에 표시되지 않은 페이지가 보호 슬롯을 소비하지 않게 한다.
pub fn extract_archive_image_with_protection(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
    protect: bool,
) -> Result<PathBuf, AppError> {
    let ext = archive_ext(archive_path);
    let out_path = extraction_out_path(temp_dir, entry_name);
    // FE는 표시용으로 이 경로를 asset URL로 계속 참조한다. 선로딩은 보호하지
    // 않지만, 두 경로 모두 LRU hit 시 modification time을 갱신한다.
    if protect {
        crate::process_temp::mark_in_use(&out_path);
    }
    if out_path.is_file() {
        crate::process_temp::touch_cache_file(&out_path);
        return Ok(out_path);
    }

    // 표시 로드와 선추출이 같은 엔트리를 동시에 요청하면 둘 다 디코드+쓰기를
    // 한다. 결과는 tmp+rename으로 안전하지만 solid 아카이브의 중복 디코드는
    // 크므로, 파일별 락 아래에서 존재를 다시 확인해 한 쪽만 추출하게 한다.
    let key = out_path.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "archive extraction lock", || {
        if out_path.is_file() {
            return Ok(());
        }
        match ext.as_str() {
            "cbz" | "zip" => extract_zip_image(archive_path, entry_name, &out_path),
            _ => Err(AppError::unsupported("Unsupported archive format")),
        }
    })?;
    crate::process_temp::touch_cache_file(&out_path);
    Ok(out_path)
}

/// 소문자 확장자. 지원 판정과 추출 라우팅이 같은 규칙을 쓰게 한다.
fn archive_ext(archive_path: &Path) -> String {
    archive_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default()
}

fn open_archive_file(archive_path: &Path) -> Result<fs::File, AppError> {
    fs::File::open(archive_path)
        .map_err(|e| AppError::io("Failed to open archive", e, ErrorCode::Corrupt))
}

fn write_extracted(out_path: &Path, buf: &[u8]) -> Result<(), AppError> {
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| AppError::io("Failed to create temp dir", e, ErrorCode::Unknown))?;
    }
    // 직접 쓰면 동시 로더가 부분 파일을 읽을 수 있어 temp+rename으로 발행한다.
    let tmp = crate::sidecar::tmp_path_for(out_path);
    fs::write(&tmp, buf)
        .map_err(|e| AppError::io("Failed to write temp file", e, ErrorCode::Unknown))?;
    crate::sidecar::publish_atomic(&tmp, out_path, "Failed to publish temp file")?;
    crate::process_temp::touch_cache_file(out_path);
    // 엔트리마다 디렉터리를 다시 훑지 않도록 누적 바이트로 상한을 추적한다.
    if let Some(dir) = out_path.parent() {
        crate::process_temp::note_written(dir, buf.len() as u64, MAX_ARCHIVE_DIR_BYTES);
    }
    Ok(())
}

fn check_entry_size(len: u64) -> Result<(), AppError> {
    if len > MAX_ENTRY_BYTES {
        return Err(AppError::too_large("Archive entry too large"));
    }
    Ok(())
}

/// `limit` 바이트를 넘기면 중단하는 bounded read (압축 폭탄 대비).
/// 선언 크기와 실제 출력이 다를 수 있으므로 실제 읽기 경로에서 강제한다.
/// ComicInfo.xml(`comic_info.rs`)도 같은 가드를 쓰도록 `pub(crate)`다.
pub(crate) fn read_bounded(
    reader: &mut dyn Read,
    limit: u64,
    context: &str,
) -> Result<Vec<u8>, AppError> {
    let mut buf = Vec::new();
    let mut limited = reader.take(limit.saturating_add(1));
    limited
        .read_to_end(&mut buf)
        .map_err(|e| AppError::io(context, e, ErrorCode::Corrupt))?;
    if buf.len() as u64 > limit {
        return Err(AppError::too_large("Archive entry too large"));
    }
    Ok(buf)
}

fn list_zip_images(archive_path: &Path) -> Result<Vec<String>, AppError> {
    Ok(Arc::unwrap_or_clone(
        collect_zip_entries(archive_path)?.images,
    ))
}

/// ZIP/CBZ: 디렉터리·숨김 파일을 건너뛰고 이미지/전체 엔트리를 분류한다.
fn collect_zip_entries(archive_path: &Path) -> Result<ArchiveEntries, AppError> {
    let file = open_archive_file(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::corrupt(format!("Failed to read ZIP: {e}")))?;

    let mut images: Vec<String> = Vec::new();
    let mut all: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
        let name = entry.name().to_string();

        // 디렉토리 스킵, 숨김 파일(__MACOSX 등) 스킵
        if entry.is_dir()
            || name.ends_with('/')
            || name.ends_with('\\')
            || name.starts_with("__")
            || name.starts_with('.')
        {
            continue;
        }
        all.push(name.clone());

        // 엔트리 이름을 Path로 변환하여 이미지 확장자 확인
        let entry_path = Path::new(&name);
        if is_image_file(entry_path) {
            images.push(name);
        }
    }

    // 같은 이름의 엔트리가 여러 번 들어 있으면 추출(`by_name`)은 항상 첫
    // 번째만 돌려준다. 목록에도 한 번만 노출해 페이지 수와 실제 내용을 맞춘다.
    Ok(finish_entries(images, all))
}

fn extract_zip_image(
    archive_path: &Path,
    entry_name: &str,
    out_path: &Path,
) -> Result<(), AppError> {
    // 이미 추출됐으면 아카이브를 다시 열지 않는다 (페이지 넘김 가속).
    if out_path.is_file() {
        crate::process_temp::touch_cache_file(out_path);
        return Ok(());
    }
    let file = open_archive_file(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::corrupt(format!("Failed to read ZIP: {e}")))?;

    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| AppError::not_found(format!("Entry not found: {e}")))?;

    check_entry_size(entry.size())?;

    let buf = read_bounded(&mut entry, MAX_ENTRY_BYTES, "Failed to read entry data")?;

    write_extracted(out_path, &buf)?;

    Ok(())
}

/// 아카이브 엔트리 하나를 `limit` 바이트까지 메모리로 읽는다.
/// ComicInfo.xml 같은 작은 메타데이터용이다. 임시 파일을 만들지 않고, 선언 크기와
/// 실제 출력 양쪽을 `limit`로 막아 압축 폭탄을 거른다.
pub(crate) fn read_archive_entry_bounded(
    archive_path: &Path,
    entry_name: &str,
    limit: u64,
) -> Result<Vec<u8>, AppError> {
    match archive_ext(archive_path).as_str() {
        "cbz" | "zip" => read_zip_entry_bounded(archive_path, entry_name, limit),
        _ => Err(AppError::unsupported("Unsupported archive format")),
    }
}

fn read_zip_entry_bounded(
    archive_path: &Path,
    entry_name: &str,
    limit: u64,
) -> Result<Vec<u8>, AppError> {
    let file = open_archive_file(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::corrupt(format!("Failed to read ZIP: {e}")))?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| AppError::not_found(format!("Entry not found: {e}")))?;
    if entry.size() > limit {
        return Err(AppError::too_large("Archive entry too large"));
    }
    read_bounded(&mut entry, limit, "Failed to read entry data")
}

/// 이웃 페이지를 아카이브 오픈 1회로 선추출. FE 프리패치용 fire-and-forget.
pub fn prefetch_archive_images(
    archive_path: &Path,
    entry_names: &[String],
    temp_dir: &Path,
) -> usize {
    let ext = archive_ext(archive_path);
    // 이미 있는 항목만 걸러낸다.
    let missing: Vec<&String> = entry_names
        .iter()
        .filter(|n| {
            let out_path = extraction_out_path(temp_dir, n);
            if out_path.is_file() {
                crate::process_temp::touch_cache_file(&out_path);
                false
            } else {
                true
            }
        })
        .collect();
    if missing.is_empty() {
        return 0;
    }
    match ext.as_str() {
        "cbz" | "zip" => prefetch_zip_images(archive_path, &missing, temp_dir),
        _ => 0,
    }
}

/// 디코드 결과가 이미 모인 바이트를 파일별 락 아래에서 발행한다.
fn prefetch_write_bytes(buf: &[u8], out_path: &Path) -> bool {
    let key = out_path.to_string_lossy().into_owned();
    crate::sidecar::with_file_lock(&key, "archive extraction lock", || {
        if out_path.is_file() {
            return Ok(false);
        }
        write_extracted(out_path, buf).map(|_| true)
    })
    .unwrap_or(false)
}

fn prefetch_zip_images(archive_path: &Path, entry_names: &[&String], temp_dir: &Path) -> usize {
    let file = match fs::File::open(archive_path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return 0,
    };
    let mut done = 0;
    for name in entry_names {
        let out_path = extraction_out_path(temp_dir, name);
        if out_path.is_file() {
            continue;
        }
        let mut entry = match archive.by_name(name) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.size() > MAX_ENTRY_BYTES {
            continue;
        }
        let buf = match read_bounded(&mut entry, MAX_ENTRY_BYTES, "Failed to read entry data") {
            Ok(buf) => buf,
            Err(_) => continue,
        };
        if prefetch_write_bytes(&buf, &out_path) {
            done += 1;
        }
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_bounded_enforces_limit() {
        let mut ok: &[u8] = &[0u8; 8];
        assert_eq!(read_bounded(&mut ok, 8, "test").unwrap().len(), 8);

        let mut over: &[u8] = &[0u8; 9];
        let err = read_bounded(&mut over, 8, "test").unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::TooLarge);
    }

    #[test]
    fn test_unsupported_archive_ext() {
        let result = list_archive_images(Path::new("file.rar"));
        assert!(result.is_err());
    }

    #[test]
    fn test_removed_formats_are_unsupported() {
        for ext in ["cb7", "7z", "cbr", "rar", "cbt"] {
            let path = std::path::PathBuf::from(format!("comic.{ext}"));
            let err = list_archive_images(&path).unwrap_err();
            assert_eq!(err.code, crate::app_error::ErrorCode::Unsupported, "{ext}");
        }
    }

    /// zip 크레이트 writer로 ZIP/CBZ 픽스처를 만들어 왕복 검증
    fn write_zip_fixture(dir: &Path, file_name: &str) -> PathBuf {
        use std::io::Write as _;
        let archive_path = dir.join(file_name);
        let file = fs::File::create(&archive_path).expect("create zip");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in [
            ("001.png", b"fake-png-bytes".as_slice()),
            ("sub/002.jpg", b"fake-jpg-bytes".as_slice()),
            ("note.txt", b"not an image".as_slice()),
        ] {
            writer.start_file(name, options).expect("start entry");
            writer.write_all(data).expect("write entry");
        }
        writer.finish().expect("finish zip");
        archive_path
    }

    #[test]
    fn test_zip_list_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_zip_fixture(dir.path(), "comic.zip");

        // 텍스트 파일은 목록에서 제외된다
        let images = list_archive_images(&archive_path).expect("list zip");
        assert_eq!(images, vec!["001.png", "sub/002.jpg"]);

        let out_dir = dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let extracted =
            extract_archive_image(&archive_path, "sub/002.jpg", &out_dir).expect("extract");
        let file_name = extracted.file_name().and_then(|n| n.to_str()).unwrap();
        // 고유 prefix + 원본 basename 유지
        assert!(file_name.ends_with("_002.jpg"), "got {file_name}");
        assert_eq!(fs::read(&extracted).unwrap(), b"fake-jpg-bytes");

        let missing = extract_archive_image(&archive_path, "nope.png", &out_dir);
        assert!(missing.is_err());

        // .cbz 확장자(대문자 포함)도 같은 zip 경로를 탄다
        for name in ["comic.cbz", "comic.CBZ"] {
            let renamed = dir.path().join(name);
            fs::copy(&archive_path, &renamed).unwrap();
            assert_eq!(list_archive_images(&renamed).expect(name).len(), 2);
        }
    }

    #[test]
    fn test_same_basename_in_different_dirs_does_not_collide() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("comic.cbz");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive_path).unwrap());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in [
            ("ch1/001.png", b"first-bytes".as_slice()),
            ("ch2/001.png", b"second-bytes".as_slice()),
        ] {
            writer.start_file(name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();

        let out_dir = dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let first =
            extract_archive_image(&archive_path, "ch1/001.png", &out_dir).expect("extract 1");
        let second =
            extract_archive_image(&archive_path, "ch2/001.png", &out_dir).expect("extract 2");
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), b"first-bytes");
        assert_eq!(fs::read(&second).unwrap(), b"second-bytes");
    }

    #[test]
    fn extraction_out_path_is_unique_per_entry() {
        let dir = tempfile::tempdir().unwrap();
        let a = extraction_out_path(dir.path(), "ch1/001.png");
        let b = extraction_out_path(dir.path(), "ch2/001.png");
        let same = extraction_out_path(dir.path(), "ch1/001.png");
        assert_ne!(a, b);
        assert_eq!(a, same);
        assert!(a
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap()
            .ends_with("_001.png"));
    }
}
