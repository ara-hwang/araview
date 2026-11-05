//! 덮어쓰지 않는 파일 이동.
//!
//! `fs::rename`은 대상이 있으면 조용히 덮어쓴다(Windows에서도 std는
//! `MOVEFILE_REPLACE_EXISTING`을 쓴다). 사용자 파일의 이름 변경은 존재 확인과
//! 이동 사이에 대상이 생겨도 다른 파일을 지우면 안 되므로, OS가 원자적으로
//! "대상이 없을 때만" 옮기는 호출을 쓴다: Linux `renameat2(RENAME_NOREPLACE)`,
//! macOS `renamex_np(RENAME_EXCL)`, Windows `MoveFileExW`(`REPLACE_EXISTING` 없음).
//! 파일시스템이 그 호출을 지원하지 않으면(FAT/exFAT, 일부 네트워크 볼륨)
//! 하드 링크 + 원본 삭제로, 그것도 안 되면 존재 확인 + 이동으로 내려간다.
//! 마지막 단계만 확인과 이동 사이의 틈이 남는다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// `from`을 `to`로 옮긴다. `to`가 이미 있으면 `ErrorKind::AlreadyExists`로
/// 실패하고 두 파일 모두 그대로 둔다.
pub(crate) fn move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    match platform_move_no_replace(from, to) {
        Ok(()) => Ok(()),
        Err(e) if is_unsupported(&e) => move_by_link(from, to),
        Err(e) => Err(e),
    }
}

/// 대소문자만 바뀌는 이름 변경(`a.jpg` -> `A.jpg`)에서 두 이름이 같은 파일을
/// 가리킬 때(대소문자 무시 볼륨) 쓴다. 대상 이름이 자기 자신으로 풀려
/// 덮어쓰지 않는 이동은 거부되므로, 같은 폴더의 임시 이름을 거쳐 두 번 옮긴다.
/// 두 번째 이동이 실패하면 원래 이름으로 되돌린다.
pub(crate) fn move_case_only(from: &Path, to: &Path) -> io::Result<()> {
    let tmp = move_to_unique_temp(from)?;
    match move_no_replace(&tmp, to) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Err(restore) = move_no_replace(&tmp, from) {
                log::error!(
                    "[rename] failed to restore {} from {}: {restore}",
                    from.display(),
                    tmp.display()
                );
            }
            Err(e)
        }
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 이름 변경 중간 단계의 임시 이름. 원래 이름을 담아 중단돼도 사용자가 알아볼 수
/// 있게 하고, 지원 이미지 확장자로 끝나지 않게 해 목록에 끼지 않게 한다.
/// `sidecar::SCRATCH_EXT`는 시작 시 청소 대상이라 쓰지 않는다(사용자 파일이다).
fn move_to_unique_temp(from: &Path) -> io::Result<PathBuf> {
    let name = from
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut last_error = None;
    for _ in 0..8 {
        let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = from.with_file_name(format!(
            ".{name}.araview-rename-{}-{count}",
            std::process::id()
        ));
        match move_no_replace(from, &tmp) {
            Ok(()) => return Ok(tmp),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last_error = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::from(io::ErrorKind::AlreadyExists)))
}

/// 덮어쓰지 않는 이동 플래그를 파일시스템이 지원하지 않는다는 오류인지.
fn is_unsupported(e: &io::Error) -> bool {
    if e.kind() == io::ErrorKind::Unsupported {
        return true;
    }
    #[cfg(unix)]
    {
        // EINVAL: 플래그 미지원 파일시스템, ENOSYS: 커널에 호출 없음,
        // ENOTSUP/EOPNOTSUPP: macOS 볼륨 미지원.
        const EINVAL: i32 = 22;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        const ENOSYS: i32 = 38;
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        const ENOSYS: i32 = 78;
        #[cfg(target_os = "macos")]
        const ENOTSUP: i32 = 45;
        #[cfg(not(target_os = "macos"))]
        const ENOTSUP: i32 = 95;
        matches!(e.raw_os_error(), Some(EINVAL | ENOSYS | ENOTSUP))
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// 하드 링크는 대상이 있으면 원자적으로 실패하므로 덮어쓰지 않는 이동의 대안이
/// 된다. 링크를 못 만드는 파일시스템이면 존재 확인 후 이동한다.
fn move_by_link(from: &Path, to: &Path) -> io::Result<()> {
    match fs::hard_link(from, to) {
        Ok(()) => {
            if let Err(e) = fs::remove_file(from) {
                // 두 이름이 남지 않게 방금 만든 링크를 지우고 실패를 알린다.
                let _ = fs::remove_file(to);
                return Err(e);
            }
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(e),
        Err(_) => move_checked(from, to),
    }
}

/// 최후의 대안. 확인과 이동 사이의 틈은 남는다.
fn move_checked(from: &Path, to: &Path) -> io::Result<()> {
    if fs::symlink_metadata(to).is_ok() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    fs::rename(from, to)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::ffi::{c_char, c_int, c_uint, CString};
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn renameat2(
            olddirfd: c_int,
            oldpath: *const c_char,
            newdirfd: c_int,
            newpath: *const c_char,
            flags: c_uint,
        ) -> c_int;
    }
    const AT_FDCWD: c_int = -100;
    const RENAME_NOREPLACE: c_uint = 1;

    let from = CString::new(from.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let to = CString::new(to.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: both pointers come from live NUL-terminated CStrings that outlive
    // the call, and AT_FDCWD makes the kernel resolve them relative to the cwd.
    let result = unsafe {
        renameat2(
            AT_FDCWD,
            from.as_ptr(),
            AT_FDCWD,
            to.as_ptr(),
            RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(target_os = "macos")]
fn platform_move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::ffi::{c_char, c_int, c_uint, CString};
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn renamex_np(from: *const c_char, to: *const c_char, flags: c_uint) -> c_int;
    }
    const RENAME_EXCL: c_uint = 0x0000_0004;

    let from = CString::new(from.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let to = CString::new(to.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    // SAFETY: both pointers come from live NUL-terminated CStrings that outlive
    // the call.
    let result = unsafe { renamex_np(from.as_ptr(), to.as_ptr(), RENAME_EXCL) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn platform_move_no_replace(from: &Path, to: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }

    let wide = |path: &Path| -> Vec<u16> {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let from = wide(from);
    let to = wide(to);
    // `MOVEFILE_REPLACE_EXISTING` is deliberately absent: an existing target
    // fails with ERROR_ALREADY_EXISTS / ERROR_FILE_EXISTS (io AlreadyExists).
    // SAFETY: both buffers are NUL-terminated UTF-16 strings that outlive the call.
    let ok = unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) };
    if ok != 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    windows
)))]
fn platform_move_no_replace(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Whether `dir` keeps names that differ only in case apart.
    pub(crate) fn is_case_sensitive(dir: &Path) -> bool {
        let probe = dir.join("case-probe");
        fs::write(&probe, b"").unwrap();
        let sensitive = !dir.join("CASE-PROBE").exists();
        fs::remove_file(&probe).unwrap();
        sensitive
    }

    #[test]
    fn move_no_replace_refuses_an_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"from").unwrap();
        fs::write(&to, b"to").unwrap();

        let error = move_no_replace(&from, &to).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&from).unwrap(), b"from");
        assert_eq!(fs::read(&to).unwrap(), b"to");
    }

    #[test]
    fn move_no_replace_moves_to_a_free_name() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"from").unwrap();

        move_no_replace(&from, &to).unwrap();

        assert!(!from.exists());
        assert_eq!(fs::read(&to).unwrap(), b"from");
    }

    /// The link fallback keeps the same no-overwrite guarantee.
    #[test]
    fn link_fallback_refuses_an_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"from").unwrap();
        fs::write(&to, b"to").unwrap();

        assert_eq!(
            move_by_link(&from, &to).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(fs::read(&to).unwrap(), b"to");

        let free = dir.path().join("free.txt");
        move_by_link(&from, &free).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read(&free).unwrap(), b"from");
    }

    #[test]
    fn case_only_move_goes_through_a_temp_name() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("page.png");
        let to = dir.path().join("PAGE.png");
        fs::write(&from, b"page").unwrap();

        move_case_only(&from, &to).unwrap();

        assert_eq!(fs::read(&to).unwrap(), b"page");
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("PAGE.png")]);
    }

    /// When the final name is taken the file returns to its original name.
    /// Needs two names that differ only in case, so it runs on case-sensitive
    /// volumes only (Linux CI; macOS and Windows defaults fold case).
    #[test]
    fn case_only_move_restores_the_original_name_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        if !is_case_sensitive(dir.path()) {
            return;
        }
        let from = dir.path().join("page.png");
        let to = dir.path().join("PAGE.png");
        fs::write(&from, b"page").unwrap();
        fs::write(&to, b"other").unwrap();

        let error = move_case_only(&from, &to).unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&from).unwrap(), b"page");
        assert_eq!(fs::read(&to).unwrap(), b"other");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
