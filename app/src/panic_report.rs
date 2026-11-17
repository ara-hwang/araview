//! Minimal, local-only panic records. Independently implemented, no crash server or telemetry.
//! Prepare the file before a panic, never lock the normal logger, and never record payloads,
//! arbitrary thread names, environment values or backtraces containing private machine paths.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;

pub(crate) const FILE_NAME: &str = "araview-panic.log";
const MAX_BYTES: u64 = 64 * 1024;

struct ReportFile {
    file: File,
    remaining: u64,
}

impl Write for ReportFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = bytes.len().min(self.remaining as usize);
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "panic report limit reached",
            ));
        }
        let written = self.file.write(&bytes[..count])?;
        self.remaining -= written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

fn open(path: &Path) -> io::Result<ReportFile> {
    if std::fs::metadata(path).is_ok_and(|meta| meta.len() >= MAX_BYTES) {
        let rotated = path.with_file_name("araview-panic.log.1");
        // Windows rename does not replace a destination. Keep at most one previous file.
        match std::fs::remove_file(&rotated) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        std::fs::rename(path, rotated)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(path)?;
    let remaining = MAX_BYTES.saturating_sub(file.metadata()?.len());
    Ok(ReportFile { file, remaining })
}

/// Install once at app startup. Failure disables only this optional local report.
pub(crate) fn install(path: &Path, version: &'static str) -> io::Result<()> {
    let output = Mutex::new(open(path)?);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // try_lock also handles poisoned/re-entrant logging without another panic.
        if let Ok(mut output) = output.try_lock() {
            let thread = std::thread::current();
            let _ = writeln!(output, "AraView {version} panic thread={:?}", thread.id());
            if let Some(location) = info.location() {
                let _ = write!(output, "source=");
                // Only a compiler-supplied source basename, never the absolute build path.
                let source = location
                    .file()
                    .rsplit(['/', '\\'])
                    .next()
                    .unwrap_or("unknown");
                for byte in source.bytes().take(96) {
                    let byte = if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
                    {
                        byte
                    } else {
                        b'_'
                    };
                    let _ = output.write_all(&[byte]);
                }
                let _ = writeln!(output, ":{}:{}", location.line(), location.column());
            }
            let _ = writeln!(
                output,
                "payload, thread name and backtrace omitted for privacy\n"
            );
            let _ = output.flush();
        }
        // Preserve Rust's existing stderr/debugger behavior, without sending anything remotely.
        previous(info);
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn panic_child() {
        let Some(path) = std::env::var_os("ARAVIEW_PANIC_REPORT_TEST") else {
            return;
        };
        install(Path::new(&path), "test-build").unwrap();
        let _ = std::thread::Builder::new()
            .name("private-thread-photo.png".into())
            .spawn(|| panic!("private payload C:\\Users\\Alice\\secret.jpg"))
            .unwrap()
            .join();
        std::process::exit(73);
    }

    #[test]
    fn separate_process_records_metadata_but_not_sensitive_strings() {
        let path =
            std::env::temp_dir().join(format!("araview-panic-test-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let result = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "panic_report::tests::panic_child", "--nocapture"])
            .env("ARAVIEW_PANIC_REPORT_TEST", &path)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(73));
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(text.contains("AraView test-build panic thread="));
        assert!(text.contains("source=panic_report.rs:"));
        for secret in [
            "private-thread",
            "private payload",
            "Alice",
            "secret.jpg",
            "C:\\Users",
        ] {
            assert!(!text.contains(secret), "private data in panic report");
        }
    }

    #[test]
    fn unwritable_location_is_optional_and_does_not_install_a_hook() {
        let missing_parent =
            std::env::temp_dir().join(format!("araview-no-parent-{}", std::process::id()));
        assert!(install(&missing_parent.join(FILE_NAME), "test-build").is_err());
    }

    #[test]
    fn full_report_rotates_once_at_startup() {
        let dir =
            std::env::temp_dir().join(format!("araview-panic-rotation-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        std::fs::write(&path, vec![b'x'; MAX_BYTES as usize]).unwrap();
        std::fs::write(dir.join("araview-panic.log.1"), b"older").unwrap();
        let report = open(&path).unwrap();
        assert_eq!(report.remaining, MAX_BYTES);
        assert_eq!(
            std::fs::metadata(dir.join("araview-panic.log.1"))
                .unwrap()
                .len(),
            MAX_BYTES
        );
        drop(report);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn report_writer_enforces_a_fixed_size_budget() {
        let path =
            std::env::temp_dir().join(format!("araview-panic-budget-{}.log", std::process::id()));
        let file = File::create(&path).unwrap();
        let mut writer = ReportFile { file, remaining: 3 };
        assert!(writer.write_all(b"too long").is_err());
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 3);
        drop(writer);
        std::fs::remove_file(path).unwrap();
    }
}
