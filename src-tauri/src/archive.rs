use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::app_error::{AppError, ErrorCode};
use crate::image::is_image_file;

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

/// solid 7z 블록 하나의 선언 크기 합 상한 (zipbomb 가드).
/// solid 블록에서 뒤쪽 엔트리를 읽으려면 앞부분 전체를 디코드해야 하므로,
/// 개별 엔트리 상한만으로는 수백 GB짜리 디코드 작업을 막지 못한다.
/// 실제 대형 만화 아카이브(수백 페이지 x 수 MB)는 이 값을 넘지 않는다.
const MAX_SOLID_BLOCK_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// 아카이브별 추출 디렉터리 상한. 초과분은 가장 오래된 추출물부터 지운다.
/// 아직 화면에 있는 추출물은 `mark_in_use`로 보호되고(FE가 이미 asset URL을
/// 들고 있어 재추출 계기가 없다), 나머지는 다음 접근 시 다시 추출된다.
const MAX_ARCHIVE_DIR_BYTES: u64 = 1024 * 1024 * 1024;

/// 아카이브 내부의 이미지 엔트리 이름을 정렬된 순서로 반환
pub fn list_archive_images(archive_path: &Path) -> Result<Vec<String>, AppError> {
    let ext = archive_ext(archive_path);

    match ext.as_str() {
        "cbz" | "zip" => list_zip_images(archive_path),
        "cb7" | "7z" => list_7z_images(archive_path),
        "cbr" | "rar" => list_rar_images(archive_path),
        "cbt" => list_tar_images(archive_path),
        _ => Err(AppError::unsupported("Unsupported archive format")),
    }
}

/// 아카이브에서 특정 엔트리를 임시 파일로 추출하고 경로를 반환
pub fn extract_archive_image(
    archive_path: &Path,
    entry_name: &str,
    temp_dir: &Path,
) -> Result<PathBuf, AppError> {
    let ext = archive_ext(archive_path);
    let out_path = extraction_out_path(temp_dir, entry_name);
    // FE는 이 경로로 asset URL을 만들어 계속 참조한다. 발행 직후 다른
    // 스레드의 축출이 지우지 못하도록 추출 전에 보호한다.
    crate::process_temp::mark_in_use(&out_path);
    if out_path.is_file() {
        return Ok(out_path);
    }

    match ext.as_str() {
        "cbz" | "zip" => extract_zip_image(archive_path, entry_name, &out_path),
        "cb7" | "7z" => extract_7z_image(archive_path, entry_name, &out_path),
        "cbr" | "rar" => extract_rar_image(archive_path, entry_name, &out_path),
        "cbt" => extract_tar_image(archive_path, entry_name, &out_path),
        _ => Err(AppError::unsupported("Unsupported archive format")),
    }?;
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
    let file = open_archive_file(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::corrupt(format!("Failed to read ZIP: {e}")))?;

    let mut images: Vec<String> = Vec::new();
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

        // 엔트리 이름을 Path로 변환하여 이미지 확장자 확인
        let entry_path = Path::new(&name);
        if is_image_file(entry_path) {
            images.push(name);
        }
    }

    images.sort_by_cached_key(|n| n.to_lowercase());
    // 같은 이름의 엔트리가 여러 번 들어 있으면 추출(`by_name`)은 항상 첫
    // 번째만 돌려준다. 목록에도 한 번만 노출해 페이지 수와 실제 내용을 맞춘다.
    images.dedup();
    Ok(images)
}

fn extract_zip_image(
    archive_path: &Path,
    entry_name: &str,
    out_path: &Path,
) -> Result<(), AppError> {
    // 이미 추출됐으면 아카이브를 다시 열지 않는다 (페이지 넘김 가속).
    if out_path.is_file() {
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

fn list_7z_images(archive_path: &Path) -> Result<Vec<String>, AppError> {
    let reader = sevenz_rust2::ArchiveReader::open(archive_path, sevenz_rust2::Password::empty())
        .map_err(|e| AppError::corrupt(format!("Failed to read 7z: {e}")))?;

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

    images.sort_by_cached_key(|n| n.to_lowercase());
    images.dedup();
    Ok(images)
}

fn extract_7z_image(
    archive_path: &Path,
    entry_name: &str,
    out_path: &Path,
) -> Result<(), AppError> {
    if out_path.is_file() {
        return Ok(());
    }
    let mut reader =
        sevenz_rust2::ArchiveReader::open(archive_path, sevenz_rust2::Password::empty())
            .map_err(|e| AppError::corrupt(format!("Failed to read 7z: {e}")))?;

    // 디코드 전에 선언 크기를 검사한다. solid 아카이브는 대상 앞 엔트리까지
    // 순서대로 디코드해야 하므로, 개별 엔트리 상한과 함께 블록 총량도
    // 제한해 수십 GB짜리 디코드 작업을 막는다.
    {
        let archive = reader.archive();
        if let Some(target_idx) = archive.files.iter().position(|f| f.name() == entry_name) {
            let target_block = archive.stream_map.file_block_index[target_idx];
            let mut block_bytes: u64 = 0;
            for (i, file) in archive.files.iter().enumerate() {
                if archive.stream_map.file_block_index[i] == target_block {
                    check_entry_size(file.size())?;
                    block_bytes = block_bytes.saturating_add(file.size());
                }
            }
            if block_bytes > MAX_SOLID_BLOCK_BYTES {
                return Err(AppError::too_large("Archive block too large"));
            }
        }
    }

    let buf = reader
        .read_file(entry_name)
        .map_err(|e| AppError::not_found(format!("Entry not found: {e}")))?;
    check_entry_size(buf.len() as u64)?;

    write_extracted(out_path, &buf)?;

    Ok(())
}

fn list_rar_images(archive_path: &Path) -> Result<Vec<String>, AppError> {
    let archive = open_rar(archive_path)?;

    let mut images: Vec<String> = Vec::new();
    for member in archive.members() {
        if member.meta.is_directory {
            continue;
        }
        let name = rar_display_name(&member.meta);
        if name.starts_with("__") || name.starts_with('.') {
            continue;
        }
        if is_image_file(Path::new(&name)) {
            images.push(name);
        }
    }

    images.sort_by_cached_key(|n| n.to_lowercase());
    images.dedup();
    Ok(images)
}

fn extract_rar_image(
    archive_path: &Path,
    entry_name: &str,
    out_path: &Path,
) -> Result<(), AppError> {
    if out_path.is_file() {
        return Ok(());
    }
    let archive = open_rar(archive_path)?;

    // 표시 이름은 lossy/`/` 정규화 결과라 추출용 정확한 바이트로 되돌린다
    // (첫 일치 우선, 이전 백엔드와 같은 규칙).
    let mut target: Option<(Vec<u8>, u64)> = None;
    for member in archive.members() {
        if rar_display_name(&member.meta) == entry_name {
            target = Some((member.meta.name_bytes().to_vec(), member.meta.unpacked_size));
            break;
        }
    }
    let (raw_name, declared) =
        target.ok_or_else(|| AppError::not_found(format!("Entry not found: {entry_name}")))?;
    check_entry_size(declared)?;

    // rars의 `read_member`는 병렬 버퍼 디코드로 모든 멤버를 메모리에 올릴 수
    // 있고 상한도 반환 뒤에만 확인한다. 스트리밍 추출에 bounded writer를
    // 붙여 대상 엔트리만 상한 내에서 모은다. solid 아카이브는 호출마다 전체
    // 패스를 돌지만 이전 백엔드도 추출마다 다시 열었으므로 비용 성격이 같다.
    let buf = read_rar_member_bounded(&archive, &raw_name, MAX_ENTRY_BYTES)?
        .ok_or_else(|| AppError::not_found(format!("Entry not found: {entry_name}")))?;

    write_extracted(out_path, &buf)?;

    Ok(())
}

/// 스트리밍 추출로 RAR 멤버 하나를 `limit` 바이트까지만 모은다.
/// 한도를 넘는 순간 쓰기를 거부해 디코드를 중단하므로, 선언 크기를
/// 위조한 헤더도 실제 메모리 할당 전에 막힌다.
fn read_rar_member_bounded(
    archive: &rars::Archive,
    raw_name: &[u8],
    limit: u64,
) -> Result<Option<Vec<u8>>, AppError> {
    struct BoundedBuffer {
        bytes: Vec<u8>,
        limit: u64,
        exceeded: bool,
    }

    impl std::io::Write for BoundedBuffer {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            if self.bytes.len() as u64 + data.len() as u64 > self.limit {
                self.exceeded = true;
                return Err(std::io::Error::other("archive entry too large"));
            }
            self.bytes.extend_from_slice(data);
            Ok(data.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    struct SharedBoundedBuffer(std::sync::Arc<std::sync::Mutex<BoundedBuffer>>);

    impl std::io::Write for SharedBoundedBuffer {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            let mut guard = self
                .0
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::io::Write::write(&mut *guard, data)
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let slot = std::sync::Arc::new(std::sync::Mutex::new(BoundedBuffer {
        bytes: Vec::new(),
        limit,
        exceeded: false,
    }));
    let writer_slot = std::sync::Arc::clone(&slot);
    let mut found = false;
    // 200MB 초과 멤버는 rars가 버퍼 대신 스트리밍 경로로 디코드하게 한다.
    let options = rars::ArchiveReadOptions::default().with_rar50_buffered_decode_limit(limit);
    let result = archive.extract_to_with_options(options, |meta| {
        if meta.name != raw_name || meta.is_directory {
            return Ok(Box::new(std::io::sink()) as Box<dyn std::io::Write>);
        }
        found = true;
        Ok(
            Box::new(SharedBoundedBuffer(std::sync::Arc::clone(&writer_slot)))
                as Box<dyn std::io::Write>,
        )
    });

    let exceeded = slot.lock().map(|buffer| buffer.exceeded).unwrap_or(false);
    if exceeded {
        return Err(AppError::too_large("Archive entry too large"));
    }
    if let Err(e) = result {
        return Err(AppError::corrupt(format!("Failed to read entry data: {e}")));
    }
    if !found {
        return Ok(None);
    }
    let mut buffer = slot
        .lock()
        .map_err(|_| AppError::unknown("Failed to lock RAR buffer"))?;
    Ok(Some(std::mem::take(&mut buffer.bytes)))
}

/// `rars` 파사드로 RAR 열기. RAR 1.3부터 RAR 7까지 시그니처로 분기한다.
/// 암호 항목은 비밀번호 없이 디코드할 때 에러가 나 호출자가 Corrupt로 분류한다.
fn open_rar(archive_path: &Path) -> Result<rars::Archive, AppError> {
    rars::ArchiveReader::read_path(archive_path)
        .map_err(|e| AppError::corrupt(format!("Failed to read RAR: {e}")))
}

/// RAR 멤버 표시 이름: lossy UTF-8에 `\`를 `/`로 정규화한다
/// (아래 tar 경로 처리와 같은 규칙).
fn rar_display_name(meta: &rars::ArchiveMemberMeta) -> String {
    meta.name_lossy().replace('\\', "/")
}

fn list_tar_images(archive_path: &Path) -> Result<Vec<String>, AppError> {
    let file = open_archive_file(archive_path)?;
    let mut archive = tar::Archive::new(file);

    let mut images: Vec<String> = Vec::new();
    let entries = archive
        .entries()
        .map_err(|e| AppError::corrupt(format!("Failed to read TAR: {e}")))?;
    for entry in entries {
        let entry = entry.map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry
            .path()
            .map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
        let name = path.to_string_lossy().replace('\\', "/");
        if name.starts_with("__") || name.starts_with('.') {
            continue;
        }
        if is_image_file(Path::new(&name)) {
            images.push(name);
        }
    }

    images.sort_by_cached_key(|n| n.to_lowercase());
    images.dedup();
    Ok(images)
}

fn extract_tar_image(
    archive_path: &Path,
    entry_name: &str,
    out_path: &Path,
) -> Result<(), AppError> {
    if out_path.is_file() {
        return Ok(());
    }
    let file = open_archive_file(archive_path)?;
    let mut archive = tar::Archive::new(file);

    let entries = archive
        .entries()
        .map_err(|e| AppError::corrupt(format!("Failed to read TAR: {e}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| AppError::corrupt(format!("Failed to read entry: {e}")))?;
        let name = path.to_string_lossy().replace('\\', "/");
        if name != entry_name {
            continue;
        }
        check_entry_size(entry.header().size().unwrap_or(0))?;
        let buf = read_bounded(&mut entry, MAX_ENTRY_BYTES, "Failed to read entry data")?;
        write_extracted(out_path, &buf)?;
        return Ok(());
    }
    Err(AppError::not_found(format!(
        "Entry not found: {entry_name}"
    )))
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
        .filter(|n| !extraction_out_path(temp_dir, n).is_file())
        .collect();
    if missing.is_empty() {
        return 0;
    }
    match ext.as_str() {
        "cbz" | "zip" => prefetch_zip_images(archive_path, &missing, temp_dir),
        "cb7" | "7z" => {
            // 7z는 리더 재사용이 까다로워 개별 추출(내부 exists 스킵)에 맡긴다.
            let mut done = 0;
            for name in missing {
                let out_path = extraction_out_path(temp_dir, name);
                if extract_7z_image(archive_path, name, &out_path).is_ok() {
                    done += 1;
                }
            }
            done
        }
        "cbr" | "rar" => {
            // 표시 경로(extract_archive_image)와 달리 in-use 링을 소비하지
            // 않도록 추출 함수를 직접 부른다. 아직 화면에 없는 페이지가
            // 보호 슬롯을 차지하면 정작 표시 중인 페이지가 밀려난다.
            let mut done = 0;
            for name in missing {
                let out_path = extraction_out_path(temp_dir, name);
                if extract_rar_image(archive_path, name, &out_path).is_ok() {
                    done += 1;
                }
            }
            done
        }
        "cbt" => {
            let mut done = 0;
            for name in missing {
                let out_path = extraction_out_path(temp_dir, name);
                if extract_tar_image(archive_path, name, &out_path).is_ok() {
                    done += 1;
                }
            }
            done
        }
        _ => 0,
    }
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
        if write_extracted(&out_path, &buf).is_ok() {
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
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive_path).expect("create cb7");
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
        let file_name = extracted.file_name().and_then(|n| n.to_str()).unwrap();
        // 고유 prefix + 원본 basename 유지
        assert!(file_name.ends_with("_sub-002.jpg"), "got {file_name}");
        assert_eq!(fs::read(&extracted).unwrap(), b"fake-jpg-bytes");

        let missing = extract_archive_image(&archive_path, "nope.png", &out_dir);
        assert!(missing.is_err());
    }

    #[test]
    fn test_same_basename_in_different_dirs_does_not_collide() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.png");
        let b = dir.path().join("b.png");
        fs::write(&a, b"first-bytes").unwrap();
        fs::write(&b, b"second-bytes").unwrap();

        let archive_path = dir.path().join("comic.cb7");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive_path).expect("create cb7");
        writer.set_encrypt_header(false);
        for (src, name) in [(&a, "ch1/001.png"), (&b, "ch2/001.png")] {
            let file = fs::File::open(src).expect("open fixture");
            let entry = sevenz_rust2::ArchiveEntry::from_path(src, name.to_string());
            writer
                .push_archive_entry(entry, Some(file))
                .expect("push entry");
        }
        writer.finish().expect("finish cb7");

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

    #[test]
    fn test_cb7_uppercase_extension() {
        let dir = tempfile::tempdir().unwrap();
        let lower = write_cb7_fixture(dir.path());
        let upper = dir.path().join("comic.CB7");
        fs::rename(&lower, &upper).unwrap();
        let images = list_archive_images(&upper).expect("list CB7");
        assert_eq!(images.len(), 2);
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
        ] {
            writer.start_file(name, options).expect("start entry");
            writer.write_all(data).expect("write entry");
        }
        writer.finish().expect("finish zip");
        archive_path
    }

    #[test]
    fn test_zip_alias_list_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_zip_fixture(dir.path(), "comic.zip");

        let images = list_archive_images(&archive_path).expect("list zip");
        assert_eq!(images, vec!["001.png", "sub/002.jpg"]);

        let out_dir = dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let extracted = extract_archive_image(&archive_path, "001.png", &out_dir).expect("extract");
        assert_eq!(fs::read(&extracted).unwrap(), b"fake-png-bytes");

        // .cbz 확장자도 같은 zip 경로를 탄다
        let cbz = dir.path().join("comic.cbz");
        fs::rename(&archive_path, &cbz).unwrap();
        let images = list_archive_images(&cbz).expect("list cbz");
        assert_eq!(images.len(), 2);
    }

    /// tar 크레이트 builder로 CBT 픽스처를 만들어 왕복 검증
    fn write_cbt_fixture(dir: &Path) -> PathBuf {
        let archive_path = dir.join("comic.cbt");
        let file = fs::File::create(&archive_path).expect("create cbt");
        let mut builder = tar::Builder::new(file);
        for (name, data) in [
            ("001.png", b"fake-png-bytes".as_slice()),
            ("ch/002.jpg", b"fake-jpg-bytes".as_slice()),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, name, data)
                .expect("append");
        }
        builder.into_inner().expect("finish cbt");
        archive_path
    }

    #[test]
    fn test_cbt_list_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_cbt_fixture(dir.path());

        let images = list_archive_images(&archive_path).expect("list cbt");
        assert_eq!(images, vec!["001.png", "ch/002.jpg"]);

        let out_dir = dir.path().join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let extracted =
            extract_archive_image(&archive_path, "ch/002.jpg", &out_dir).expect("extract");
        assert_eq!(fs::read(&extracted).unwrap(), b"fake-jpg-bytes");

        let missing = extract_archive_image(&archive_path, "nope.png", &out_dir);
        assert!(missing.is_err());
    }

    /// rars Builder로 RAR 픽스처를 만들어 목록/추출 왕복 검증.
    /// `version`에 Rar50/Rar40을 넣어 양쪽 세대를 커버한다.
    fn write_rar_fixture(dir: &Path, file_name: &str, version: rars::ArchiveVersion) -> PathBuf {
        let mut builder = rars::Builder::new(version).store(true);
        builder
            .add_bytes(b"001.png".to_vec(), b"fake-png-bytes".to_vec(), None, None)
            .expect("add png");
        builder
            .add_bytes(
                b"sub/002.jpg".to_vec(),
                b"fake-jpg-bytes".to_vec(),
                None,
                None,
            )
            .expect("add jpg");
        let bytes = builder.to_bytes().expect("build rar");
        let archive_path = dir.join(file_name);
        fs::write(&archive_path, bytes).unwrap();
        archive_path
    }

    fn assert_rar_roundtrip(archive_path: &Path) {
        let images = list_archive_images(archive_path).expect("list rar");
        assert_eq!(images, vec!["001.png", "sub/002.jpg"]);

        let out_dir = archive_path.parent().expect("parent").join("out");
        fs::create_dir_all(&out_dir).unwrap();
        let extracted =
            extract_archive_image(archive_path, "sub/002.jpg", &out_dir).expect("extract");
        assert_eq!(fs::read(&extracted).unwrap(), b"fake-jpg-bytes");

        let missing = extract_archive_image(archive_path, "nope.png", &out_dir);
        assert!(missing.is_err());
    }

    #[test]
    fn test_rar5_list_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_rar_fixture(dir.path(), "comic.cbr", rars::ArchiveVersion::Rar50);
        assert_rar_roundtrip(&archive_path);
    }

    #[test]
    fn test_rar4_list_and_extract_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let archive_path = write_rar_fixture(dir.path(), "old.cbr", rars::ArchiveVersion::Rar40);
        assert_rar_roundtrip(&archive_path);
    }

    #[test]
    fn test_rar_sample_fixture_when_present() {
        // 저장소 samples/sample.cbr(RAR5)로 실파일 회귀 검증.
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("samples")
            .join("sample.cbr");
        if !source.is_file() {
            return;
        }
        let images = list_archive_images(&source).expect("list sample.cbr");
        assert!(!images.is_empty(), "sample.cbr has no images");
        let dir = tempfile::tempdir().unwrap();
        let extracted =
            extract_archive_image(&source, &images[0], dir.path()).expect("extract sample");
        assert!(extracted.is_file());
        assert!(fs::metadata(&extracted).expect("stat").len() > 0);
    }

    #[test]
    fn test_rar_routes_to_rar_handler() {
        // RAR 인코더가 없으므로 라우팅만 검증: 가짜 .rar는
        // "Unsupported archive format"이 아닌 RAR 판독 에러를 낸다.
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake.rar");
        fs::write(&fake, b"not-a-rar").unwrap();
        let err = list_archive_images(&fake).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Corrupt);
        assert!(
            err.message.contains("Failed to read RAR"),
            "unexpected error: {err}"
        );
        let fake_cbr = dir.path().join("fake.cbr");
        fs::write(&fake_cbr, b"not-a-rar").unwrap();
        let err = list_archive_images(&fake_cbr).unwrap_err();
        assert_eq!(err.code, crate::app_error::ErrorCode::Corrupt);
        assert!(
            err.message.contains("Failed to read RAR"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_7z_alias_extension() {
        let dir = tempfile::tempdir().unwrap();
        let lower = write_cb7_fixture(dir.path());
        let alias = dir.path().join("archive.7z");
        fs::rename(&lower, &alias).unwrap();
        let images = list_archive_images(&alias).expect("list 7z");
        assert_eq!(images.len(), 2);
    }
}
