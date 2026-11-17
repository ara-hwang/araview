//! Updater I/O seams, independently implemented. No network, clock or process globals in tests.
//! The production adapter supplies request-wide deadlines; tests inspect those deadlines without
//! sleeping. Signature verification is supplied by the caller and is never bypassed in production.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

pub(super) const DOWNLOAD_CHUNK: usize = 64 * 1024;

#[derive(Default)]
pub(super) struct DownloadProgress {
    pub(super) received: AtomicU64,
    pub(super) total: AtomicU64,
    pub(super) cancelled: AtomicBool,
}

impl DownloadProgress {
    pub(super) fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn percent(&self) -> Option<f32> {
        let total = self.total.load(Ordering::Relaxed);
        let received = self.received.load(Ordering::Relaxed);
        (total > 0).then(|| (received as f32 / total as f32 * 100.).min(100.))
    }
}

/// None means HTTP 404. Other failed statuses and transport failures remain errors.
pub(super) trait Http {
    fn get<T>(
        &self,
        url: &str,
        timeout: Duration,
        consume: impl FnOnce(u64, &mut dyn Read) -> Result<T, String>,
    ) -> Result<Option<T>, String>;
}

pub(super) trait Installer {
    fn save(&self, bytes: &[u8]) -> Result<PathBuf, String>;
    fn read(&self, path: &Path) -> Result<Vec<u8>, String>;
    fn launch(&self, path: &Path) -> Result<(), String>;
    fn discard(&self, path: &Path);
}

pub(super) fn read_with_progress(
    mut reader: impl Read,
    progress: &DownloadProgress,
) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0; DOWNLOAD_CHUNK];
    loop {
        if progress.is_cancelled() {
            return Ok(None);
        }
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Ok(Some(bytes)),
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.to_string()),
        };
        bytes.extend_from_slice(&chunk[..read]);
        progress
            .received
            .store(bytes.len() as u64, Ordering::Relaxed);
    }
}

pub(super) fn download(
    http: &impl Http,
    installer: &impl Installer,
    url: &str,
    timeout: Duration,
    limit: u64,
    progress: &DownloadProgress,
    verify: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<Option<PathBuf>, String> {
    let bytes = http
        .get(url, timeout, |total, reader| {
            progress.total.store(total, Ordering::Relaxed);
            read_with_progress(reader.take(limit.saturating_add(1)), progress)
        })?
        .ok_or("installer was not found")?;
    let Some(bytes) = bytes else { return Ok(None) };
    if bytes.len() as u64 > limit {
        return Err("installer is larger than expected".to_owned());
    }
    verify(&bytes)?;
    if progress.is_cancelled() {
        return Ok(None);
    }
    installer.save(&bytes).map(Some)
}

/// Re-read and authenticate the persisted file immediately before crossing the process boundary.
/// Never invoke launch after a failed read/signature check, and remove failed candidates.
pub(super) fn launch_verified(
    installer: &impl Installer,
    path: &Path,
    verify: impl FnOnce(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let result = installer
        .read(path)
        .and_then(|bytes| verify(&bytes))
        .and_then(|()| installer.launch(path));
    if result.is_err() {
        installer.discard(path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::io::{self, Cursor};

    struct FakeHttp {
        response: RefCell<Option<Result<Option<Vec<u8>>, String>>>,
        seen: RefCell<Vec<(String, Duration)>>,
    }

    impl FakeHttp {
        fn new(response: Result<Option<Vec<u8>>, String>) -> Self {
            Self {
                response: RefCell::new(Some(response)),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    impl Http for FakeHttp {
        fn get<T>(
            &self,
            url: &str,
            timeout: Duration,
            consume: impl FnOnce(u64, &mut dyn Read) -> Result<T, String>,
        ) -> Result<Option<T>, String> {
            self.seen.borrow_mut().push((url.to_owned(), timeout));
            self.response
                .borrow_mut()
                .take()
                .unwrap()?
                .map(|bytes| consume(bytes.len() as u64, &mut Cursor::new(bytes)))
                .transpose()
        }
    }

    #[derive(Default)]
    struct FakeInstaller {
        bytes: RefCell<Vec<u8>>,
        saves: Cell<usize>,
        launches: Cell<usize>,
        discarded: Cell<bool>,
        fail_launch: Cell<bool>,
    }

    impl Installer for FakeInstaller {
        fn save(&self, bytes: &[u8]) -> Result<PathBuf, String> {
            self.saves.set(self.saves.get() + 1);
            self.bytes.replace(bytes.to_vec());
            Ok(PathBuf::from("test-installer.exe"))
        }
        fn read(&self, _: &Path) -> Result<Vec<u8>, String> {
            Ok(self.bytes.borrow().clone())
        }
        fn launch(&self, _: &Path) -> Result<(), String> {
            self.launches.set(self.launches.get() + 1);
            if self.fail_launch.get() {
                Err("launch failed".into())
            } else {
                Ok(())
            }
        }
        fn discard(&self, _: &Path) {
            self.discarded.set(true);
        }
    }

    // This is a pipeline guard, not a substitute for the real minisign tests in update.rs.
    fn verify_fixture(bytes: &[u8]) -> Result<(), String> {
        if bytes == b"signed" {
            Ok(())
        } else {
            Err("signature mismatch".into())
        }
    }

    fn attempt(
        http: &impl Http,
        installer: &impl Installer,
        progress: &DownloadProgress,
    ) -> Result<Option<PathBuf>, String> {
        download(
            http,
            installer,
            "https://example.invalid/setup.exe",
            Duration::from_secs(600),
            64,
            progress,
            verify_fixture,
        )
    }

    #[test]
    fn successful_download_checks_signature_then_launch_rechecks_persisted_bytes() {
        let http = FakeHttp::new(Ok(Some(b"signed".to_vec())));
        let installer = FakeInstaller::default();
        let progress = DownloadProgress::default();
        let path = attempt(&http, &installer, &progress).unwrap().unwrap();
        assert_eq!(progress.percent(), Some(100.));
        assert_eq!(installer.saves.get(), 1);
        assert_eq!(
            installer.launches.get(),
            0,
            "download must not launch an installer"
        );
        assert_eq!(http.seen.borrow()[0].1, Duration::from_secs(600));
        installer.bytes.replace(b"tampered after save".to_vec());
        assert!(launch_verified(&installer, &path, verify_fixture).is_err());
        assert_eq!(installer.launches.get(), 0);
        assert!(installer.discarded.get());
    }

    #[test]
    fn missing_timeout_and_bad_signature_never_persist_or_launch() {
        for response in [
            Ok(None),
            Err("deadline exceeded".into()),
            Ok(Some(b"bad".to_vec())),
        ] {
            let installer = FakeInstaller::default();
            assert!(
                attempt(
                    &FakeHttp::new(response),
                    &installer,
                    &DownloadProgress::default()
                )
                .is_err()
            );
            assert_eq!(installer.saves.get(), 0);
            assert_eq!(installer.launches.get(), 0);
        }
    }

    #[test]
    fn size_limit_and_cancelled_response_never_persist() {
        let installer = FakeInstaller::default();
        assert!(
            attempt(
                &FakeHttp::new(Ok(Some(vec![0; 65]))),
                &installer,
                &DownloadProgress::default()
            )
            .is_err()
        );
        let progress = DownloadProgress::default();
        progress.cancel();
        assert_eq!(
            attempt(
                &FakeHttp::new(Ok(Some(b"signed".to_vec()))),
                &installer,
                &progress
            ),
            Ok(None)
        );
        assert_eq!(installer.saves.get(), 0);
        // A retry has independent progress, not the cancelled request's flag.
        assert!(
            attempt(
                &FakeHttp::new(Ok(Some(b"signed".to_vec()))),
                &installer,
                &DownloadProgress::default()
            )
            .unwrap()
            .is_some()
        );
    }

    #[test]
    fn partial_read_failure_does_not_become_a_complete_download() {
        struct Partial(bool);
        impl Read for Partial {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0 {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "deadline exceeded"));
                }
                self.0 = true;
                buf[..3].copy_from_slice(b"sig");
                Ok(3)
            }
        }
        struct PartialHttp;
        impl Http for PartialHttp {
            fn get<T>(
                &self,
                _: &str,
                _: Duration,
                consume: impl FnOnce(u64, &mut dyn Read) -> Result<T, String>,
            ) -> Result<Option<T>, String> {
                consume(6, &mut Partial(false)).map(Some)
            }
        }
        let installer = FakeInstaller::default();
        let progress = DownloadProgress::default();
        assert!(attempt(&PartialHttp, &installer, &progress).is_err());
        assert_eq!(progress.received.load(Ordering::Relaxed), 3);
        assert_eq!(installer.saves.get(), 0);
    }

    #[test]
    fn cancellation_after_a_partial_chunk_stops_before_verification_or_save() {
        struct CancellingHttp<'a>(&'a DownloadProgress);
        impl Http for CancellingHttp<'_> {
            fn get<T>(
                &self,
                _: &str,
                _: Duration,
                consume: impl FnOnce(u64, &mut dyn Read) -> Result<T, String>,
            ) -> Result<Option<T>, String> {
                struct CancellingReader<'a>(&'a DownloadProgress);
                impl Read for CancellingReader<'_> {
                    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                        self.0.cancel();
                        buffer[..3].copy_from_slice(b"sig");
                        Ok(3)
                    }
                }
                consume(6, &mut CancellingReader(self.0)).map(Some)
            }
        }
        let progress = DownloadProgress::default();
        let installer = FakeInstaller::default();
        let result = download(
            &CancellingHttp(&progress),
            &installer,
            "https://example.invalid",
            Duration::from_secs(600),
            64,
            &progress,
            |_| panic!("cancelled bytes must not be verified"),
        );
        assert_eq!(result, Ok(None));
        assert_eq!(progress.received.load(Ordering::Relaxed), 3);
        assert_eq!(installer.saves.get(), 0);
    }

    #[test]
    fn launch_success_and_failure_cross_the_boundary_only_after_verification() {
        let installer = FakeInstaller::default();
        installer.bytes.replace(b"signed".to_vec());
        let path = Path::new("fake.exe");
        assert_eq!(launch_verified(&installer, path, verify_fixture), Ok(()));
        assert_eq!(installer.launches.get(), 1);
        installer.fail_launch.set(true);
        assert!(launch_verified(&installer, path, verify_fixture).is_err());
        assert_eq!(installer.launches.get(), 2);
        assert!(installer.discarded.get());
    }
}
