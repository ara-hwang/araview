use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // 메인 스레드가 큰 이미지를 다루므로 이 크레이트의 실행 파일과 테스트의 스택을 넉넉히 둔다.
    // (워크스페이스의 썸네일 DLL에는 적용하지 않으려고 링크 인자를 이 크레이트에만 건다.)
    println!("cargo:rustc-link-arg=/STACK:8000000");
    copy_libheif_dlls();
    collect_license_files();
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/icon.ico");
    // 아이콘이 빠져도 앱은 동작하므로 리소스 컴파일 실패로 빌드를 막지 않는다.
    let _ = embed_resource::compile("resources/app.rc", embed_resource::NONE).manifest_optional();
}

/// vcpkg로 동적 링크한 libheif 런타임 DLL을 실행 파일 옆에 둔다. 없으면 앱이
/// 시작되지 않는다.
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

/// vcpkg 네이티브 라이브러리(libheif, libde265, aom)의 저작권 문서를 실행 파일 옆
/// `licenses/`로 모은다. 설치 프로그램이 이 폴더를 그대로 동봉하고, 앱의 라이선스 창이 읽는다.
fn collect_license_files() {
    let Some(root) = env::var_os("VCPKG_ROOT").or_else(|| env::var_os("VCPKG_INSTALLATION_ROOT"))
    else {
        return;
    };
    let Some(profile_dir) = env::var_os("OUT_DIR")
        .map(PathBuf::from)
        .and_then(|out| out.ancestors().nth(3).map(Path::to_path_buf))
    else {
        return;
    };
    let dest_dir = profile_dir.join("licenses");
    let _ = fs::create_dir_all(&dest_dir);
    let share = Path::new(&root)
        .join("installed")
        .join("x64-windows")
        .join("share");
    for package in ["libheif", "libde265", "aom"] {
        let source = share.join(package).join("copyright");
        let Ok(content) = fs::read(&source) else {
            continue;
        };
        let dest = dest_dir.join(format!("{package}-copyright.txt"));
        if fs::read(&dest).ok().as_deref() != Some(content.as_slice()) {
            let _ = fs::write(&dest, content);
        }
        println!("cargo:rerun-if-changed={}", source.display());
    }
}
