use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use crate::app_error::{AppError, ErrorCode};
use crate::image::is_image_file;
use crate::natural_sort::sort_by_natural_path;

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

/// 수집된 전체 엔트리를 공통 뒤처리한다: 정렬 + dedup + Arc. 이미지 목록은
/// 정렬된 전체 목록에서 걸러 내므로 정렬 키를 한 번만 계산한다.
fn finish_entries(mut all: Vec<String>) -> ArchiveEntries {
    sort_by_natural_path(&mut all, |n| n.as_str());
    all.dedup();
    let images = all
        .iter()
        .filter(|name| is_image_file(Path::new(name)))
        .cloned()
        .collect();
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
    ArchiveExtractor::new(archive_path, temp_dir).extract(entry_name, protect)
}

/// 한 아카이브에서 엔트리를 여러 개 추출하는 핸들. 아카이브는 추출물이 없는
/// 첫 엔트리에서 한 번만 열어 이후 엔트리에 재사용한다. 배치 워커는 각자
/// 하나씩 들고 병렬로 추출한다.
pub(crate) struct ArchiveExtractor<'a> {
    archive_path: &'a Path,
    temp_dir: &'a Path,
    zip: Option<zip::ZipArchive<fs::File>>,
}

impl<'a> ArchiveExtractor<'a> {
    pub(crate) fn new(archive_path: &'a Path, temp_dir: &'a Path) -> Self {
        Self {
            archive_path,
            temp_dir,
            zip: None,
        }
    }

    pub(crate) fn extract(&mut self, entry_name: &str, protect: bool) -> Result<PathBuf, AppError> {
        let ext = archive_ext(self.archive_path);
        let out_path = extraction_out_path(self.temp_dir, entry_name);
        // FE는 표시용으로 이 경로를 asset URL로 계속 참조한다. 선로딩은 보호하지
        // 않지만, 두 경로 모두 LRU hit 시 modification time을 갱신한다.
        if protect {
            crate::process_temp::mark_in_use(&out_path);
        }
        // 이미 추출됐으면 아카이브를 열지 않는다 (페이지 넘김 가속).
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
                "cbz" | "zip" => self.extract_zip(entry_name, &out_path),
                _ => Err(AppError::unsupported("Unsupported archive format")),
            }
        })?;
        crate::process_temp::touch_cache_file(&out_path);
        Ok(out_path)
    }

    fn extract_zip(&mut self, entry_name: &str, out_path: &Path) -> Result<(), AppError> {
        let archive = match &mut self.zip {
            Some(archive) => archive,
            None => self.zip.insert(open_zip(self.archive_path)?),
        };
        let mut entry = archive
            .by_name(entry_name)
            .map_err(|e| AppError::not_found(format!("Entry not found: {e}")))?;

        check_entry_size(entry.size())?;

        let buf = read_bounded(&mut entry, MAX_ENTRY_BYTES, "Failed to read entry data")?;

        write_extracted(out_path, &buf)
    }
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

/// 파싱해 둔 ZIP 중앙 디렉터리. 경로, 수정 시각, 크기가 모두 같을 때만 다시 쓴다.
struct ZipIndex {
    path: PathBuf,
    modified: Option<SystemTime>,
    len: u64,
    metadata: Arc<zip::read::ZipArchiveMetadata>,
}

/// 최근에 연 아카이브 몇 개의 중앙 디렉터리. 추출물이 없는 페이지를 열거나
/// 배치 워커가 각자 핸들을 열 때 수천 엔트리의 디렉터리를 다시 파싱하지 않는다.
static ZIP_INDEXES: Mutex<Vec<ZipIndex>> = Mutex::new(Vec::new());
const MAX_ZIP_INDEXES: usize = 4;

/// ZIP을 연다. 같은 파일의 중앙 디렉터리가 캐시에 있으면 파싱을 건너뛴다.
fn open_zip(archive_path: &Path) -> Result<zip::ZipArchive<fs::File>, AppError> {
    let file = open_archive_file(archive_path)?;
    // 방금 연 핸들에서 읽으므로 캐시 판정과 실제 읽을 파일이 어긋나지 않는다.
    let identity = file
        .metadata()
        .ok()
        .map(|meta| (meta.modified().ok(), meta.len()));
    let cached = identity.and_then(|(modified, len)| {
        let indexes = ZIP_INDEXES.lock().ok()?;
        indexes
            .iter()
            .find(|index| {
                index.path == archive_path && index.modified == modified && index.len == len
            })
            .map(|index| Arc::clone(&index.metadata))
    });
    if let Some(metadata) = cached {
        // SAFETY: 이 함수의 `unsafe`는 메모리 안전성이 아니라 reader와 메타데이터가
        // 같은 파일의 것이어야 한다는 계약이다. 메타데이터는 경로, 수정 시각,
        // 크기가 방금 연 핸들과 같은 파일에서 파싱한 것만 꺼냈다.
        return Ok(unsafe { zip::ZipArchive::unsafe_new_with_metadata(file, metadata) });
    }

    let archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::corrupt(format!("Failed to read ZIP: {e}")))?;
    if let (Some((modified, len)), Ok(mut indexes)) = (identity, ZIP_INDEXES.lock()) {
        indexes.retain(|index| index.path != archive_path);
        if indexes.len() >= MAX_ZIP_INDEXES {
            indexes.remove(0);
        }
        indexes.push(ZipIndex {
            path: archive_path.to_path_buf(),
            modified,
            len,
            metadata: archive.metadata(),
        });
    }
    Ok(archive)
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
    let mut archive = open_zip(archive_path)?;

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
        all.push(name);
    }

    // 같은 이름의 엔트리가 여러 번 들어 있으면 추출(`by_name`)은 항상 첫
    // 번째만 돌려준다. 목록에도 한 번만 노출해 페이지 수와 실제 내용을 맞춘다.
    Ok(finish_entries(all))
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
    let mut archive = open_zip(archive_path)?;
    let mut entry = archive
        .by_name(entry_name)
        .map_err(|e| AppError::not_found(format!("Entry not found: {e}")))?;
    if entry.size() > limit {
        return Err(AppError::too_large("Archive entry too large"));
    }
    read_bounded(&mut entry, limit, "Failed to read entry data")
}

/// 아카이브의 엔트리 하나를 교체한다(없으면 추가). ComicInfo.xml 쓰기용이다.
/// 나머지 엔트리는 재압축 없이 그대로 옮기고, 같은 폴더의 임시 파일에 다 쓴 뒤
/// 원본과 바꾼다. 실패하면 원본은 그대로 남는다.
pub(crate) fn replace_archive_entry(
    archive_path: &Path,
    entry_name: &str,
    data: &[u8],
) -> Result<(), AppError> {
    match archive_ext(archive_path).as_str() {
        "cbz" | "zip" => {
            let tmp = crate::sidecar::scratch_path_for(archive_path);
            let written = write_zip_with_entry(archive_path, entry_name, data, &tmp);
            let replaced = written.and_then(|()| {
                fs::rename(&tmp, archive_path)
                    .map_err(|e| AppError::io("Failed to replace archive", e, ErrorCode::Unknown))
            });
            if replaced.is_err() {
                let _ = fs::remove_file(&tmp);
            }
            replaced
        }
        _ => Err(AppError::unsupported("Unsupported archive format")),
    }
}

fn write_zip_with_entry(
    archive_path: &Path,
    entry_name: &str,
    data: &[u8],
    out_path: &Path,
) -> Result<(), AppError> {
    use std::io::Write as _;

    let zip_err = |e: zip::result::ZipError| AppError::unknown(format!("Failed to write ZIP: {e}"));
    let io_err = |e: std::io::Error| AppError::io("Failed to write archive", e, ErrorCode::Unknown);

    let file = open_archive_file(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::corrupt(format!("Failed to read ZIP: {e}")))?;

    let out = fs::File::create(out_path).map_err(io_err)?;
    let mut writer = zip::ZipWriter::new(out);
    writer
        .set_raw_comment(archive.comment().into())
        .map_err(zip_err)?;
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
        if entry.name() == entry_name {
            continue;
        }
        writer.raw_copy_file(entry).map_err(zip_err)?;
    }
    writer
        .start_file(entry_name, zip::write::SimpleFileOptions::default())
        .map_err(zip_err)?;
    writer.write_all(data).map_err(io_err)?;
    let out = writer.finish().map_err(zip_err)?;
    out.sync_all().map_err(io_err)
}

/// 이웃 페이지 선추출. FE 프리패치용 fire-and-forget.
pub fn prefetch_archive_images(
    archive_path: &Path,
    entry_names: &[String],
    temp_dir: &Path,
) -> usize {
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
    // 압축 해제는 엔트리끼리 독립이라 워커마다 핸들을 따로 열어 병렬로 푼다.
    // 같은 엔트리의 동시 추출은 추출기의 파일별 락이 한쪽만 하게 막는다.
    crate::thumbnail::map_with_worker_state(
        &missing,
        || ArchiveExtractor::new(archive_path, temp_dir),
        |extractor, name| extractor.extract(name, false).is_ok(),
    )
    .into_iter()
    .filter(|extracted| *extracted)
    .count()
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
    fn replace_archive_entry_swaps_one_entry_and_keeps_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_zip_fixture(dir.path(), "comic.cbz");

        // 없던 엔트리는 추가되고, 다시 쓰면 교체된다.
        replace_archive_entry(&archive_path, "ComicInfo.xml", b"<a/>").expect("add");
        replace_archive_entry(&archive_path, "ComicInfo.xml", b"<b/>").expect("replace");

        let all = list_archive_entries(&archive_path).expect("list").all;
        assert_eq!(
            *all,
            vec!["001.png", "ComicInfo.xml", "note.txt", "sub/002.jpg"]
        );
        let xml = read_archive_entry_bounded(&archive_path, "ComicInfo.xml", 1024).expect("read");
        assert_eq!(xml, b"<b/>");
        let page = read_archive_entry_bounded(&archive_path, "sub/002.jpg", 1024).expect("read");
        assert_eq!(page, b"fake-jpg-bytes");

        // 임시 파일이 폴더에 남지 않는다.
        let leftovers = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 1);
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
    fn extractor_reuses_one_handle_and_prefetch_runs_in_parallel() {
        use std::io::Write as _;
        let dir = tempfile::tempdir().unwrap();
        let archive_path = dir.path().join("many.cbz");
        let mut writer = zip::ZipWriter::new(fs::File::create(&archive_path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        let names: Vec<String> = (0..24).map(|i| format!("p{i:03}.png")).collect();
        for name in &names {
            writer.start_file(name.as_str(), options).unwrap();
            writer.write_all(name.repeat(64).as_bytes()).unwrap();
        }
        writer.finish().unwrap();

        let out_dir = dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let mut extractor = ArchiveExtractor::new(&archive_path, &out_dir);
        for name in &names[..2] {
            let path = extractor.extract(name, false).expect("extract");
            assert_eq!(fs::read(path).unwrap(), name.repeat(64).as_bytes());
        }
        assert!(extractor.extract("nope.png", false).is_err());

        // 이미 있는 2개는 세지 않고 나머지를 모두 추출한다.
        assert_eq!(prefetch_archive_images(&archive_path, &names, &out_dir), 22);
        assert_eq!(prefetch_archive_images(&archive_path, &names, &out_dir), 0);
        for name in &names {
            let path = extraction_out_path(&out_dir, name);
            assert_eq!(fs::read(path).unwrap(), name.repeat(64).as_bytes());
        }
    }

    #[test]
    fn zip_index_is_not_reused_after_the_archive_changes() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_zip_fixture(dir.path(), "comic.cbz");
        assert_eq!(list_archive_images(&archive_path).unwrap().len(), 2);

        replace_archive_entry(&archive_path, "003.png", b"added-page").expect("add");
        assert_eq!(list_archive_images(&archive_path).unwrap().len(), 3);
        let page = read_archive_entry_bounded(&archive_path, "003.png", 1024).expect("read");
        assert_eq!(page, b"added-page");
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
