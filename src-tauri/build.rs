use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    copy_libheif_dlls();
    tauri_build::build();
}

fn copy_libheif_dlls() {
    let Some(root) = env::var_os("VCPKG_ROOT").or_else(|| env::var_os("VCPKG_INSTALLATION_ROOT"))
    else {
        return;
    };
    let bin = Path::new(&root)
        .join("installed")
        .join("x64-windows")
        .join("bin");
    if !bin.is_dir() {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let bundled = manifest_dir.join("generated").join("libheif-dlls");
    let _ = fs::create_dir_all(&bundled);

    let mut dests = vec![bundled];
    if let Ok(out_dir) = env::var("OUT_DIR") {
        let out = PathBuf::from(out_dir);
        if let Some(profile_dir) = out.ancestors().nth(3) {
            dests.push(profile_dir.to_path_buf());
            dests.push(profile_dir.join("deps"));
        }
    }

    let Ok(entries) = fs::read_dir(&bin) else {
        return;
    };
    for entry in entries.flatten() {
        let src = entry.path();
        let Some(name) = src.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_libheif_runtime_dll(name) {
            continue;
        }
        for dest_dir in &dests {
            let _ = fs::create_dir_all(dest_dir);
            let _ = fs::copy(&src, dest_dir.join(name));
        }
    }
}

fn is_libheif_runtime_dll(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".dll")
        && [
            "heif",
            "libde265",
            "libx265",
            "x265",
            "aom",
            "avif",
            "dav1d",
            "jpeg",
            "turbojpeg",
            "libpng",
            "zlib",
            "lzma",
            "brotli",
            "sharpyuv",
            "libwebp",
            "libyuv",
            "libxml2",
            "iconv",
            "charset",
        ]
        .iter()
        .any(|needle| lower.contains(needle))
}
