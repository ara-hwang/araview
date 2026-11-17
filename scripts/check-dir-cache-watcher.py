#!/usr/bin/env python3
"""Dependency-free regression/ballpark check of production watcher invalidation.

Extracts the production functions, not a second implementation. The standalone
Rust binary owns its globals and runs serially, so it cannot invalidate entries
belonging to parallel Cargo tests. Only filesystem canonicalize is instrumented.
Run with RUSTUP_TOOLCHAIN=stable python3 scripts/check-dir-cache-watcher.py.
"""
from pathlib import Path
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "crates/araview-core/src/dir_cache.rs").read_text()


def function(name):
    start = SOURCE.index(f"fn {name}(")
    opening = SOURCE.index("{", start)
    depth = 1
    end = opening + 1
    while depth:
        depth += (SOURCE[end] == "{") - (SOURCE[end] == "}")
        end += 1
    return SOURCE[start:end]


HEADER = r'''
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
struct CachedListing { parent_key: String }
static DIR_CACHE: LazyLock<Mutex<HashMap<String, CachedListing>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static SCANS: LazyLock<Mutex<HashMap<u64, (String, bool)>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static LAST_EVENT_PARENT: Mutex<Option<(PathBuf, Instant, String)>> = Mutex::new(None);
const EVENT_PARENT_TTL: Duration = Duration::from_secs(1);
static CANONICALIZES: AtomicUsize = AtomicUsize::new(0);
mod fs {
    pub fn canonicalize(path: impl AsRef<std::path::Path>) -> std::io::Result<std::path::PathBuf> {
        super::CANONICALIZES.fetch_add(1, super::Ordering::Relaxed);
        std::fs::canonicalize(path)
    }
}
'''
TESTS = r'''
fn reset() {
    DIR_CACHE.lock().unwrap().clear();
    SCANS.lock().unwrap().clear();
    *LAST_EVENT_PARENT.lock().unwrap() = None;
    CANONICALIZES.store(0, Ordering::Relaxed);
}
fn insert(key: &str) {
    DIR_CACHE.lock().unwrap().insert(key.into(), CachedListing { parent_key: key.into() });
}
fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("araview-watcher-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    root
}
#[test]
fn empty_cache_and_scans_skip_path_io_during_100k_path_event() {
    reset();
    let path = Path::new("missing-root/not-present.jpg");
    let start = Instant::now();
    // Same path loop as notify Event.paths: event construction is not timed.
    for _ in 0..100_000 { invalidate_for_path(path); }
    eprintln!("empty 100000 paths: {:?}, canonicalize calls: {}", start.elapsed(), CANONICALIZES.load(Ordering::Relaxed));
    assert_eq!(CANONICALIZES.load(Ordering::Relaxed), 0);
}
#[test]
fn burst_drops_listing_but_dirties_inflight_scan_without_touching_sibling() {
    reset();
    let root = temp_root();
    let key = parent_key(&std::fs::canonicalize(&root).unwrap());
    let sibling = format!("{}sibling{}", key.trim_end_matches(std::path::MAIN_SEPARATOR), std::path::MAIN_SEPARATOR);
    insert(&key);
    insert(&sibling);
    SCANS.lock().unwrap().insert(1, (key.clone(), false));
    SCANS.lock().unwrap().insert(2, (sibling.clone(), false));
    let path = root.join("deleted.jpg");
    let start = Instant::now();
    for _ in 0..100_000 { invalidate_for_path(&path); }
    eprintln!("active scan 100000 paths: {:?}, canonicalize calls: {}", start.elapsed(), CANONICALIZES.load(Ordering::Relaxed));
    let cache = DIR_CACHE.lock().unwrap();
    assert!(!cache.contains_key(&key));
    assert!(cache.contains_key(&sibling));
    let scans = SCANS.lock().unwrap();
    assert!(scans[&1].1);
    assert!(!scans[&2].1);
    drop(scans);
    drop(cache);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn watch_error_clears_all_listings_and_dirties_all_scans() {
    reset();
    insert("one/"); insert("two/");
    SCANS.lock().unwrap().insert(1, ("one/".into(), false));
    SCANS.lock().unwrap().insert(2, ("two/".into(), false));
    invalidate_after_watch_error();
    assert!(DIR_CACHE.lock().unwrap().is_empty());
    assert!(SCANS.lock().unwrap().values().all(|(_, dirty)| *dirty));
}
#[test]
fn first_path_invalidates_then_remainder_skips_path_io() {
    reset();
    let root = temp_root();
    let key = parent_key(&std::fs::canonicalize(&root).unwrap());
    insert(&key);
    let path = root.join("deleted.jpg");
    invalidate_for_path(&path);
    assert!(DIR_CACHE.lock().unwrap().is_empty());
    let after_first = CANONICALIZES.load(Ordering::Relaxed);
    assert!(after_first > 0);
    for _ in 1..100_000 { invalidate_for_path(&path); }
    assert_eq!(CANONICALIZES.load(Ordering::Relaxed), after_first);
    std::fs::remove_dir_all(root).unwrap();
}
'''
functions = "\n".join(function(name) for name in (
    "fold_case", "with_trailing_sep", "parent_key", "mark_scans_dirty",
    "canonical_event_parent", "invalidate_after_watch_error", "invalidate_for_path",
))
# Confirm the exercised error helper is wired to the actual notify callback.
assert "invalidate_after_watch_error();" in function("ensure_watched")
with tempfile.TemporaryDirectory(prefix="araview-watcher-check-") as tmp:
    source = Path(tmp) / "watcher.rs"
    binary = Path(tmp) / ("watcher.exe" if os.name == "nt" else "watcher")
    source.write_text(HEADER + functions + TESTS)
    subprocess.run(["rustc", "--edition=2024", "-O", "--test", str(source), "-o", str(binary)], check=True)
    subprocess.run([str(binary), "--test-threads=1", "--nocapture"], check=True)
