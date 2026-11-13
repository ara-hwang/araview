//! 디코드 경로 리팩터링 전후의 픽셀 출력을 검증하는 다이제스트 도구.
//!
//! yosh의 `decode_digest`와 같은 역할이다. 각 입력 이미지에 대해 표시 디코드,
//! 저해상 프리뷰, 해상도 상한 sidecar 축소의 결과 픽셀을 SHA-256으로 기록한다.
//! 리팩터링 전에 한 번, 후에 한 번 실행해 출력을 diff한다. 라인이 하나라도
//! 다르면 디코드·축소 경로의 출력이 바뀐 것이므로 의도한 변경인지 확인해야 한다.
//! JPEG/PNG 인코더의 바이트 출력 변화는 잡지 않는다(픽셀만 본다).
//!
//! ```sh
//! cargo run -p araview-core --example digest -- samples > before.txt
//! # ... 리팩터링 ...
//! cargo run -p araview-core --example digest -- samples > after.txt
//! diff before.txt after.txt
//! ```
//!
//! libheif(AVIF/HEIC) 샘플은 `aom.dll`·`libde265.dll`·`heif.dll`이 실행 파일
//! 옆에 있어야 디코드된다(개발 문서 "요구 사항"). DLL이 없으면 그 샘플 라인은
//! `ERR ...`로 기록되고 나머지는 그대로 검증된다.
//!
//! 출력 라인 형식: `<라벨>\t<파일명>\t<폭>x<높이>\t<sha256>`. 라인은 라벨 뒤에
//! 파일명이 온다. `-`는 해당 경로가 대상이 아님(`ERR`는 디코드 실패)을 뜻한다.

use std::path::{Path, PathBuf};

const SVG_MAX_SIDE: u32 = 8192;
const PREVIEW_MAX_SIDE: u32 = 2048;
const SCALED_MAX_SIDE: u32 = 2048;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let roots: Vec<PathBuf> = if args.is_empty() {
        vec![PathBuf::from("samples")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };
    let mut files = Vec::new();
    for root in &roots {
        collect_files(root, &mut files);
    }
    files.sort();
    if files.is_empty() {
        eprintln!("입력 이미지가 없다: {args:?}");
        std::process::exit(2);
    }

    // sidecar가 사용자의 실제 캐시에 쌓이지 않게 임시 루트를 쓴다.
    let cache = tempfile::tempdir().expect("임시 캐시 폴더");
    araview_core::process_temp::initialize(
        cache.path(),
        araview_core::process_temp::CacheStorageMode::Temporary,
    )
    .expect("캐시 루트 초기화");

    let mut exit = 0;
    for path in &files {
        for (label, result) in [
            ("full", run_full(path)),
            ("preview", run_preview(path)),
            ("scaled", run_scaled(path)),
        ] {
            match result {
                Ok(Ok(digest)) => println!("{label}\t{digest}"),
                Ok(Err(none_reason)) => {
                    println!("{label}\t{}\t-\t{none_reason}", path.display());
                }
                Err(message) => {
                    println!("{label}\t{}\t-\tERR {message}", path.display());
                    exit = 1;
                }
            }
        }
    }
    if exit != 0 {
        eprintln!("디코드 실패가 있다. ERR 라인을 확인한다.");
    }
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>) {
    if root.is_file() {
        out.push(root.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        collect_files(&entry.path(), out);
    }
}

fn run_full(path: &Path) -> Result<Result<String, String>, String> {
    let image = araview_core::display::decode_for_display(path, SVG_MAX_SIDE)
        .map_err(|e| format!("{e}"))?;
    digest_frame(path, &image)
}

fn run_preview(path: &Path) -> Result<Result<String, String>, String> {
    match araview_core::display::decode_preview_for_display(path, PREVIEW_MAX_SIDE) {
        Some(image) => digest_frame(path, &image),
        None => Ok(Err("none (preview path does not apply)".to_string())),
    }
}

fn run_scaled(path: &Path) -> Result<Result<String, String>, String> {
    let sidecar = araview_core::scaled::ensure_scaled_sidecar(path, SCALED_MAX_SIDE)
        .map_err(|e| format!("{e}"))?;
    let Some(sidecar) = sidecar else {
        return Ok(Err("original (not over the cap)".to_string()));
    };
    let img = image::ImageReader::open(&sidecar.path)
        .map_err(|e| format!("{e}"))?
        .with_guessed_format()
        .map_err(|e| format!("{e}"))?
        .decode()
        .map_err(|e| format!("{e}"))?;
    let dims = (img.width(), img.height());
    let hash = sha256_hex(&img.into_rgba8().into_raw());
    Ok(Ok(format!(
        "{}\t{}x{}\t{hash}",
        path.display(),
        dims.0,
        dims.1
    )))
}

fn digest_frame(
    path: &Path,
    image: &araview_core::display::DisplayImage,
) -> Result<Result<String, String>, String> {
    let frame = image.frames.first().ok_or("프레임이 없다")?;
    let dims = (frame.image.width(), frame.image.height());
    let hash = sha256_hex(frame.image.as_raw());
    Ok(Ok(format!(
        "{}\t{}x{}\t{hash}",
        path.display(),
        dims.0,
        dims.1
    )))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
