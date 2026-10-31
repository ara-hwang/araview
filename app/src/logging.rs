//! 파일 로그와 회전(SPEC §17.1).
//!
//! 콘솔 필터(`RUST_LOG`, 기본 `warn`)는 env_logger가 그대로 판단하고, 같은 기록을
//! 캐시 폴더 아래 `logs/araview.log`에도 남긴다. 한 파일이 상한을 넘으면 이전 파일을
//! `araview.log.1`로 옮기고 새로 시작한다(회전 1개). 최근 경고·오류는 진단 정보
//! (`diag.rs`)가 읽도록 메모리에 마지막 몇 건만 남긴다.
//!
//! 파일 기록은 지원 요청에 붙일 자료라 조용히 실패하지 않되, 로그를 남기지 못해도
//! 앱은 그대로 동작해야 한다. 그래서 폴더를 만들지 못하거나 쓰기가 실패하면 파일
//! 기록만 포기하고 콘솔 기록은 유지한다.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// 지금 쓰는 로그 파일 이름.
pub(crate) const FILE_NAME: &str = "araview.log";
/// 회전해 남기는 이전 로그 파일 이름. 이 파일은 하나만 유지한다.
pub(crate) const ROTATED_NAME: &str = "araview.log.1";
/// 한 파일의 상한(바이트). 넘으면 회전한다.
pub(crate) const MAX_BYTES: u64 = 1024 * 1024;
/// 진단 정보에 담는 최근 경고·오류 기록 수.
pub(crate) const RECENT_LIMIT: usize = 8;

/// 로그 폴더. 캐시 폴더 아래 `logs/`다(SPEC §17.1).
pub(crate) fn log_dir() -> Option<PathBuf> {
    crate::cache_dir().map(|dir| dir.join("logs"))
}

/// 지금 쓰는 로그 파일 경로.
pub(crate) fn log_path() -> Option<PathBuf> {
    log_dir().map(|dir| dir.join(FILE_NAME))
}

/// 최근에 남긴 경고·오류(오래된 것부터). 진단 정보가 이 목록을 싣는다.
pub(crate) fn recent_problems() -> Vec<String> {
    let Ok(recent) = recent().lock() else {
        return Vec::new();
    };
    recent.iter().cloned().collect()
}

fn recent() -> &'static Mutex<VecDeque<String>> {
    static RECENT: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
    RECENT.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// 상한을 넘으면 `.1`로 회전하는 append 전용 파일.
struct RotatingFile {
    path: PathBuf,
    file: Option<File>,
    written: u64,
    limit: u64,
}

impl RotatingFile {
    /// `path`를 덧붙이기로 열고, 이미 있는 크기를 상한 계산에 넣는다.
    fn open(path: PathBuf, limit: u64) -> std::io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let written = file.metadata().map(|meta| meta.len()).unwrap_or(0);
        Ok(Self {
            path,
            file: Some(file),
            written,
            limit,
        })
    }

    /// 한 줄을 남긴다. 상한을 넘으면 먼저 회전한다.
    fn write_line(&mut self, line: &str) {
        let bytes = line.len() as u64;
        if self.written > 0 && self.written + bytes > self.limit {
            self.rotate();
        }
        let Some(file) = self.file.as_mut() else {
            return;
        };
        if file.write_all(line.as_bytes()).is_err() {
            // 쓰지 못하는 파일을 계속 붙들고 실패를 반복하지 않는다.
            self.file = None;
            return;
        }
        self.written += bytes;
    }

    /// 현재 파일을 `.1`로 옮기고 새 파일을 연다. 옮기지 못해도 새 파일로 이어 쓴다.
    fn rotate(&mut self) {
        let rotated = self.path.with_file_name(ROTATED_NAME);
        // Windows는 대상 파일이 있으면 rename이 실패하므로 먼저 지운다.
        let _ = std::fs::remove_file(&rotated);
        // 열려 있는 파일은 Windows에서 옮길 수 없어 먼저 닫는다.
        self.file = None;
        let _ = std::fs::rename(&self.path, &rotated);
        self.file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)
            .ok();
        self.written = 0;
    }

    /// 실행마다 한 줄 머리말을 남겨 어느 빌드의 기록인지 알 수 있게 한다.
    fn write_header(&mut self, version: &str, profile: &str) {
        let line = format!("=== AraView {version} ({profile}) ===\n");
        self.write_line(&line);
    }
}

/// 콘솔과 파일에 같은 기록을 남기는 로거.
struct TeeLogger {
    console: Box<dyn log::Log>,
    file: Mutex<Option<RotatingFile>>,
}

impl log::Log for TeeLogger {
    /// 표시 여부는 콘솔 필터(`RUST_LOG`)가 정한다. 파일도 같은 범위를 남긴다.
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.console.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        self.console.log(record);
        let line = format_line(record, now_millis());
        if let Ok(mut file) = self.file.lock()
            && let Some(file) = file.as_mut()
        {
            file.write_line(&line);
        }
        if record.level() <= log::Level::Warn {
            let Ok(mut recent) = recent().lock() else {
                return;
            };
            if recent.len() == RECENT_LIMIT {
                recent.pop_front();
            }
            recent.push_back(line);
        }
    }

    fn flush(&self) {
        self.console.flush();
        if let Ok(mut file) = self.file.lock()
            && let Some(file) = file.as_mut()
            && let Some(handle) = file.file.as_mut()
        {
            let _ = handle.flush();
        }
    }
}

/// 로깅을 시작한다. `main`에서 설정을 읽기 전에 한 번 부른다.
pub(crate) fn init() {
    let console =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).build();
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let file = log_dir().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        let mut file = RotatingFile::open(dir.join(FILE_NAME), MAX_BYTES).ok()?;
        file.write_header(env!("CARGO_PKG_VERSION"), profile);
        Some(file)
    });
    // `enabled()`가 실제 판단을 하므로 매크로 단계에서는 모두 통과시킨다.
    log::set_max_level(log::LevelFilter::Trace);
    if log::set_boxed_logger(Box::new(TeeLogger {
        console: Box::new(console),
        file: Mutex::new(file),
    }))
    .is_err()
    {
        eprintln!("[log] logger already initialized");
    }
}

/// 로그 한 줄. 예: `2026-10-29T03:09:00.123Z WARN  araview::app: message`
fn format_line(record: &log::Record, millis: u128) -> String {
    format!(
        "{} {:<5} {}: {}\n",
        timestamp(millis),
        record.level(),
        record.target(),
        record.args()
    )
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0)
}

/// 유닉스 밀리초를 UTC RFC 3339(밀리초 포함)로 쓴다.
fn timestamp(millis: u128) -> String {
    let seconds = (millis / 1000) as i64;
    let millis = (millis % 1000) as u32;
    let (year, month, day, hour, minute, second) = civil_from_unix(seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// 유닉스 초를 UTC 달력 값으로 바꾼다(하워드 힌넌트의 civil_from_days).
fn civil_from_unix(seconds: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    (
        year + i64::from(month <= 2),
        month as u32,
        day as u32,
        (rest / 3600) as u32,
        ((rest % 3600) / 60) as u32,
        (rest % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn timestamp_reads_utc() {
        assert_eq!(timestamp(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(timestamp(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(timestamp(1_767_225_600_000), "2026-01-01T00:00:00.000Z");
        assert_eq!(timestamp(1_767_225_600_123), "2026-01-01T00:00:00.123Z");
    }

    #[test]
    fn line_carries_level_and_target() {
        let record = log::Record::builder()
            .args(format_args!("cache miss"))
            .level(log::Level::Warn)
            .target("araview::cache")
            .build();
        assert_eq!(
            format_line(&record, 0),
            "1970-01-01T00:00:00.000Z WARN  araview::cache: cache miss\n"
        );
    }

    #[test]
    fn rotation_keeps_one_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let mut file = RotatingFile::open(path.clone(), 16).unwrap();
        for line in ["aaaaa\n", "bbbbb\n", "ccccc\n"] {
            file.write_line(line);
        }
        let rotated = dir.path().join(ROTATED_NAME);
        assert_eq!(fs::read_to_string(&rotated).unwrap(), "aaaaa\nbbbbb\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "ccccc\n");
    }

    #[test]
    fn big_existing_file_rotates_before_the_next_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        fs::write(&path, "0123456789").unwrap();
        let mut file = RotatingFile::open(path.clone(), 10).unwrap();
        file.write_line("next\n");
        assert_eq!(fs::read_to_string(&path).unwrap(), "next\n");
        assert_eq!(
            fs::read_to_string(dir.path().join(ROTATED_NAME)).unwrap(),
            "0123456789"
        );
    }

    #[test]
    fn header_names_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let mut file = RotatingFile::open(path.clone(), MAX_BYTES).unwrap();
        file.write_header("2.7.0", "debug");
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .starts_with("=== AraView 2.7.0 (debug) ===")
        );
    }
}
