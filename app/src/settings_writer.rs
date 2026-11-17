//! Single-owner settings persistence. No GPUI state crosses the worker boundary.
//! This is an AraView implementation; no Zed settings source is incorporated.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender},
    thread::{self, JoinHandle},
};

enum Message {
    Save(Vec<u8>),
    Flush(Sender<io::Result<()>>),
}

pub(super) struct SettingsWriter {
    sender: Option<Sender<Message>>,
    worker: Option<JoinHandle<()>>,
}

impl SettingsWriter {
    pub(super) fn new(
        path: PathBuf,
        mut damaged: Option<Vec<u8>>,
        report_error: fn(&Path, &io::Error),
    ) -> io::Result<Self> {
        Self::start(move |bytes| {
            let result = write_snapshot(&path, bytes, &mut damaged);
            if let Err(error) = &result {
                // The worker cannot display UI; retain the error for the flush barrier too.
                report_error(&path, error);
            }
            result
        })
    }

    fn start(write: impl FnMut(&[u8]) -> io::Result<()> + Send + 'static) -> io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("settings-writer".into())
            .spawn(move || run(receiver, write))?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
        })
    }

    pub(super) fn save(&self, bytes: Vec<u8>) -> io::Result<()> {
        self.sender
            .as_ref()
            .ok_or_else(disconnected)?
            .send(Message::Save(bytes))
            .map_err(|_| disconnected())
    }

    /// A barrier after every earlier snapshot, including errors from the latest write.
    pub(super) fn flush(&self) -> io::Result<()> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .as_ref()
            .ok_or_else(disconnected)?
            .send(Message::Flush(sender))
            .map_err(|_| disconnected())?;
        receiver.recv().map_err(|_| disconnected())?
    }
}

impl Drop for SettingsWriter {
    fn drop(&mut self) {
        // Disconnect only our producer. The worker drains queued snapshots before exiting.
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn disconnected() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "settings writer stopped")
}

fn run(receiver: Receiver<Message>, mut write: impl FnMut(&[u8]) -> io::Result<()>) {
    let mut last_error = None;
    for message in receiver {
        match message {
            Message::Save(bytes) => last_error = write(&bytes).err(),
            Message::Flush(sender) => {
                let result = match &last_error {
                    Some(error) => Err(io::Error::new(error.kind(), error.to_string())),
                    None => Ok(()),
                };
                let _ = sender.send(result);
            }
        }
    }
}

fn write_snapshot(path: &Path, bytes: &[u8], damaged: &mut Option<Vec<u8>>) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    if let Some(original) = damaged.as_ref() {
        // Never replace an unreadable settings document before preserving its exact bytes.
        // create_new also prevents an earlier recovery backup from being overwritten.
        let backup = backup_path(path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)?;
        file.write_all(original)?;
        file.sync_all()?;
        *damaged = None;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

fn backup_path(path: &Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    path.with_extension(format!(
        "json.corrupt-{timestamp}-{}",
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let dir = backup_path(&std::env::temp_dir().join("araview-writer"));
            fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn settings(&self) -> PathBuf {
            self.0.join("settings.json")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn slow_disk_does_not_block_enqueue_and_flush_preserves_order() {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let observed = writes.clone();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let writer = SettingsWriter::start(move |bytes| {
            if bytes == b"first" {
                entered_tx.send(()).unwrap();
                release_rx.recv().unwrap();
            }
            observed.lock().unwrap().push(bytes.to_vec());
            Ok(())
        })
        .unwrap();
        writer.save(b"first".to_vec()).unwrap();
        entered_rx.recv().unwrap();
        // The first disk operation is still blocked. A second UI save must return now.
        writer.save(b"last".to_vec()).unwrap();
        assert!(writes.lock().unwrap().is_empty());
        release_tx.send(()).unwrap();
        writer.flush().unwrap();
        assert_eq!(
            *writes.lock().unwrap(),
            [b"first".to_vec(), b"last".to_vec()]
        );
    }

    #[test]
    fn flush_reports_failure_and_successful_retry_clears_it() {
        let writer = SettingsWriter::start(|bytes| {
            if bytes == b"fail" {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "test failure",
                ))
            } else {
                Ok(())
            }
        })
        .unwrap();
        writer.save(b"fail".to_vec()).unwrap();
        assert_eq!(
            writer.flush().unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        writer.save(b"retry".to_vec()).unwrap();
        writer.flush().unwrap();
    }

    #[test]
    fn shutdown_drains_pending_snapshots_and_replaces_existing_file() {
        let dir = Directory::new();
        let path = dir.settings();
        fs::write(&path, b"old").unwrap();
        let writer = SettingsWriter::new(path.clone(), None, |_, _| {}).unwrap();
        for index in 0..100 {
            writer.save(index.to_string().into_bytes()).unwrap();
        }
        drop(writer);
        assert_eq!(fs::read(&path).unwrap(), b"99");
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn damaged_bytes_survive_next_save_and_backup_is_not_repeated() {
        let dir = Directory::new();
        let path = dir.settings();
        let original = vec![0xff, b'{', 0];
        fs::write(&path, &original).unwrap();
        let writer = SettingsWriter::new(path.clone(), Some(original.clone()), |_, _| {}).unwrap();
        writer.save(b"recovered".to_vec()).unwrap();
        writer.save(b"latest".to_vec()).unwrap();
        writer.flush().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"latest");
        let backups: Vec<_> = fs::read_dir(&dir.0)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|entry| entry != &path)
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(&backups[0]).unwrap(), original);
    }

    #[test]
    fn failed_backup_does_not_replace_damaged_settings() {
        let dir = Directory::new();
        // The original filename fits; adding the backup suffix exceeds the component limit.
        // This works without relying on Unix permission semantics (tests can run as root).
        let path = dir.0.join(format!("{}.json", "x".repeat(240)));
        fs::write(&path, b"broken").unwrap();
        let mut damaged = Some(b"broken".to_vec());
        assert!(write_snapshot(&path, b"new", &mut damaged).is_err());
        assert!(damaged.is_some());
        assert_eq!(fs::read(path).unwrap(), b"broken");
    }
}
