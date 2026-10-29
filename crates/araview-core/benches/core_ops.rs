//! 코어 연산 벤치(이슈 #125). `cargo bench -p araview-core`로 돌린다.
//!
//! 측정 대상은 `PRODUCT.md`가 약속하는 "즉시 열림"에 걸리는 네 갈래다.
//! - 큰 JPEG/PNG/TIFF의 full 디코드
//! - 썸네일 생성(캐시 미스)
//! - 해상도 상한 sidecar 생성(캐시 미스)
//! - 폴더 목록 스캔과 자연 정렬
//!
//! 샘플 파일은 저장소에 넣지 않고 실행할 때 임시 폴더에 만든다. 캐시도 임시
//! 폴더로 돌리므로(`process_temp::initialize`) 사용자의 실제 캐시를 건드리지
//! 않는다. 캐시 미스 경로를 재려면 회차마다 원본의 수정 시각을 바꿔 캐시 키를
//! 흔든다. 기준선 기록과 비교는 `scripts/bench.mjs`가 맡는다(`docs/releasing.md`).

use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use araview_core::process_temp::CacheStorageMode;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use image::{ExtendedColorType, RgbImage};

/// 샘플 크기. 8K(7680x4320)보다 작지만 흔한 휴대폰 사진(12MP)보다 큰 축이라
/// 디코드 비용이 충분히 드러나고, 세 OS 러너에서 벤치 전체가 몇 분 안에 끝난다.
const SAMPLE_W: u32 = 4000;
const SAMPLE_H: u32 = 3000;
/// 썸네일 스트립이 쓰는 크기(`SPEC.md` §9).
const THUMB_SIDE: u32 = 512;
/// 해상도 상한 sidecar의 상한(`SPEC.md` §9). 샘플보다 작아 sidecar가 만들어진다.
const SCALED_CAP: u32 = 2048;
/// 자연 정렬 벤치가 다루는 항목 수.
const SORT_ITEMS: usize = 2_000;
/// 폴더 목록 벤치가 만드는 파일 수.
const LISTING_ITEMS: usize = 300;

fn benches(c: &mut Criterion) {
    let fixtures = Fixtures::new();
    decode(c, &fixtures);
    thumbnails(c, &fixtures);
    scaled_sidecar(c, &fixtures);
    natural_sort(c);
    directory_listing(c, &fixtures);
}

/// 벤치가 쓰는 샘플 폴더와 캐시 폴더. 둘 다 끝나면 지워진다.
struct Fixtures {
    /// 캐시 루트. 필드로 들고 있어야 벤치가 도는 동안 폴더가 남는다.
    _cache: tempfile::TempDir,
    dir: tempfile::TempDir,
    jpeg: PathBuf,
    png: PathBuf,
    tiff: PathBuf,
}

impl Fixtures {
    fn new() -> Self {
        let cache = tempfile::tempdir().expect("임시 캐시 폴더");
        // 벤치가 만드는 썸네일·sidecar가 사용자의 실제 캐시에 쌓이지 않게 한다.
        araview_core::process_temp::initialize(cache.path(), CacheStorageMode::Temporary)
            .expect("캐시 루트 초기화");
        let dir = tempfile::tempdir().expect("임시 샘플 폴더");
        let pixels = sample_image(SAMPLE_W, SAMPLE_H);
        let jpeg = dir.path().join("sample.jpg");
        let png = dir.path().join("sample.png");
        let tiff = dir.path().join("sample.tiff");
        for path in [&jpeg, &png, &tiff] {
            save(path, &pixels);
        }
        Self {
            _cache: cache,
            dir,
            jpeg,
            png,
            tiff,
        }
    }
}

/// 확장자로 포맷을 고르게 저장한다(`image::save_buffer`의 기본 인코더).
fn save(path: &Path, pixels: &RgbImage) {
    image::save_buffer(
        path,
        pixels.as_raw(),
        pixels.width(),
        pixels.height(),
        ExtendedColorType::Rgb8,
    )
    .unwrap_or_else(|e| panic!("샘플 저장 실패 {}: {e}", path.display()));
}

/// 사진처럼 보이는 8비트 RGB 픽셀. 단색이면 JPEG가 과하게 줄어들어 실제
/// 사진보다 디코드가 싸진다. 경계와 완만한 그라디언트를 섞는다.
fn sample_image(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        let checker = ((x / 16) ^ (y / 16)) & 1;
        let shade = ((u64::from(x) * 255) / u64::from(width.max(1))) as u8;
        let line = ((u64::from(y) * 255) / u64::from(height.max(1))) as u8;
        if checker == 0 {
            image::Rgb([shade, line, 128])
        } else {
            image::Rgb([line, 128, shade])
        }
    })
}

/// full 디코드. `display::decode_for_display`는 표시 경로가 실제로 쓰는 함수다.
fn decode(c: &mut Criterion, f: &Fixtures) {
    let bytes = u64::from(SAMPLE_W) * u64::from(SAMPLE_H) * 4;
    let mut group = c.benchmark_group("decode");
    group.throughput(Throughput::Bytes(bytes));
    for (name, path) in [("jpeg", &f.jpeg), ("png", &f.png), ("tiff", &f.tiff)] {
        group.bench_function(name, |b| {
            b.iter(|| {
                let image =
                    araview_core::display::decode_for_display(path, 8192).expect("디코드 실패");
                std::hint::black_box(image)
            });
        });
    }
    group.finish();
}

/// 썸네일 생성. 회차마다 원본의 수정 시각을 바꿔 캐시 미스를 만든다.
fn thumbnails(c: &mut Criterion, f: &Fixtures) {
    let mut group = c.benchmark_group("thumbnail");
    // 캐시 미스 한 번에 썸네일 파일 하나가 생기므로 표본을 줄여 임시 캐시를 작게 유지한다.
    group.sample_size(20);
    group.bench_function("cold_jpeg_512", |b| {
        b.iter_batched(
            || bump_mtime(&f.jpeg),
            |()| {
                let thumb = araview_core::thumbnail::generate_thumbnail(&f.jpeg, THUMB_SIDE)
                    .expect("썸네일 실패");
                std::hint::black_box(thumb)
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

/// 해상도 상한 sidecar 생성. 썸네일과 같은 방식으로 캐시 미스를 만든다.
fn scaled_sidecar(c: &mut Criterion, f: &Fixtures) {
    let mut group = c.benchmark_group("scaled_sidecar");
    group.sample_size(20);
    group.bench_function("cold_jpeg_2048", |b| {
        b.iter_batched(
            || bump_mtime(&f.jpeg),
            |()| {
                let sidecar = araview_core::scaled::ensure_scaled_sidecar(&f.jpeg, SCALED_CAP)
                    .expect("sidecar 실패");
                std::hint::black_box(sidecar)
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

/// 자연 정렬. 키 계산과 정렬을 나눠 잰다.
fn natural_sort(c: &mut Criterion) {
    let names: Vec<String> = (0..SORT_ITEMS)
        .map(|index| format!("IMG_{index:05}_photo_2x{}.jpg", index % 7))
        .collect();
    let mut group = c.benchmark_group("natural_sort");
    group.bench_function("keys_2000", |b| {
        b.iter(|| {
            let keys: Vec<_> = names
                .iter()
                .map(|name| araview_core::natural_sort::natural_key(name))
                .collect();
            std::hint::black_box(keys)
        });
    });
    group.bench_function("sort_2000", |b| {
        b.iter(|| {
            let mut sorted = names.clone();
            sorted.sort_by_cached_key(|name| araview_core::natural_sort::natural_key(name));
            std::hint::black_box(sorted)
        });
    });
    group.finish();
}

/// 폴더 목록. 폴더 수정 시각이 바뀌어야 캐시를 건너뛰고 다시 스캔하므로
/// 회차마다 목록에 없는 확장자의 표식 파일을 새로 만든다.
fn directory_listing(c: &mut Criterion, f: &Fixtures) {
    let listing = f.dir.path().join("listing");
    fs::create_dir_all(&listing).expect("목록 폴더");
    for index in 0..LISTING_ITEMS {
        let path = listing.join(format!("IMG_{index:05}.jpg"));
        fs::write(&path, b"x").expect("목록 샘플");
    }
    // `get_directory_images_impl`은 파일 경로를 받아 그 상위 폴더를 나열한다.
    let current = listing.join("IMG_00000.jpg");
    let current = current.to_string_lossy().into_owned();
    let mut group = c.benchmark_group("directory");
    group.sample_size(20);
    group.bench_function("scan_300", |b| {
        b.iter_batched(
            || touch_dir(&listing),
            |()| {
                let images = araview_core::ops::get_directory_images_impl(&current, None)
                    .expect("목록 실패");
                std::hint::black_box(images)
            },
            BatchSize::PerIteration,
        );
    });
    group.finish();
}

/// 원본의 수정 시각을 앞당겨 캐시 키(`sidecar::file_identity_hash`는 경로·수정
/// 시각·크기를 섞는다)를 바꾼다. 내용은 건드리지 않으므로 복사 없이 캐시
/// 미스를 만들 수 있다.
fn bump_mtime(path: &Path) {
    static TICKS: AtomicU64 = AtomicU64::new(1);
    let tick = TICKS.fetch_add(1, Ordering::Relaxed);
    let times = FileTimes::new().set_modified(UNIX_EPOCH + Duration::from_secs(tick));
    File::options()
        .append(true)
        .open(path)
        .and_then(|file| file.set_times(times))
        .unwrap_or_else(|e| panic!("수정 시각 변경 실패 {}: {e}", path.display()));
}

/// 폴더 수정 시각을 바꾼다. 폴더 핸들은 OS마다 열 수 있는 방법이 달라서
/// (Windows는 `File::open`으로 폴더를 열 수 없다) 폴더 안에 표식 파일을 만든다.
fn touch_dir(dir: &Path) {
    static TICKS: AtomicU64 = AtomicU64::new(1);
    let tick = TICKS.fetch_add(1, Ordering::Relaxed);
    // 지원하지 않는 확장자라 목록 내용은 그대로다.
    fs::write(dir.join(format!("marker-{tick:06}.tmp")), b"")
        .unwrap_or_else(|e| panic!("폴더 수정 시각 변경 실패 {}: {e}", dir.display()));
}

criterion_group!(core_ops, benches);
criterion_main!(core_ops);
