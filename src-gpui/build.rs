use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    copy_libheif_dlls();
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=../src-tauri/icons/icon.ico");
    // 아이콘이 빠져도 앱은 동작하므로 리소스 컴파일 실패로 빌드를 막지 않는다.
    let _ = embed_resource::compile("resources/app.rc", embed_resource::NONE).manifest_optional();
}

/// vcpkg로 동적 링크한 libheif 런타임 DLL을 실행 파일 옆에 둔다. 없으면 앱이
/// 시작되지 않는다. `src-tauri/build.rs`의 같은 단계와 대상 DLL을 맞춘다.
fn copy_libheif_dlls() {
    println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
    println!("cargo:rerun-if-env-changed=VCPKG_INSTALLATION_ROOT");
    let Some(root) = env::var_os("VCPKG_ROOT").or_else(|| env::var_os("VCPKG_INSTALLATION_ROOT"))
    else {
        return;
    };
    let bin = Path::new(&root)
        .join("installed")
        .join("x64-windows")
        .join("bin");
    let Ok(entries) = fs::read_dir(&bin) else {
        return;
    };
    let Some(profile_dir) = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .and_then(|out| out.ancestors().nth(3).map(Path::to_path_buf))
    else {
        return;
    };
    let dests = [profile_dir.clone(), profile_dir.join("deps")];

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
    matches!(
        name.to_ascii_lowercase().as_str(),
        "heif.dll" | "libde265.dll" | "aom.dll"
    )
}
