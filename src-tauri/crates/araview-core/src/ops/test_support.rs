//! 커맨드 모듈 테스트가 함께 쓰는 임시 폴더/픽스처 헬퍼.

use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn unique_dir(suffix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "tiv-rename-{suffix}-{}-{nanos}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

pub(crate) fn write_sized(dir: &std::path::Path, name: &str, size: usize) -> String {
    let path = dir.join(name);
    fs::write(&path, vec![0u8; size]).expect("write dummy");
    path.to_str().unwrap().to_string()
}

/// 실제 PNG 1장을 담은 CBZ 픽스처로 아카이브 썸네일 왕복을 검증한다.
pub(crate) fn write_cbz_fixture(dir: &Path, file_name: &str) -> std::path::PathBuf {
    use std::io::Write as _;

    let png_path = dir.join("page.png");
    let img = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(200, 100, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8])
    }));
    img.save(&png_path).expect("write png fixture");

    let archive_path = dir.join(file_name);
    let file = fs::File::create(&archive_path).expect("create cbz");
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    writer.start_file("page.png", options).expect("start entry");
    writer
        .write_all(&fs::read(&png_path).expect("read png"))
        .expect("write entry");
    writer.finish().expect("finish cbz");
    archive_path
}
