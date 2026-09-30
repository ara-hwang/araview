use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    sync_dev_mcp_capability();
    copy_libheif_dlls();
    collect_license_files();
    tauri_build::build();
}

fn sync_dev_mcp_capability() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cap_path = manifest_dir.join("capabilities").join("dev-mcp.json");
    let enabled = env::var_os("CARGO_FEATURE_DEV_MCP").is_some();

    if enabled {
        let content = r#"{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "dev-mcp",
  "description": "MCP bridge for local development",
  "windows": ["main"],
  "permissions": ["mcp-bridge:default"]
}
"#;
        // 내용이 같으면 쓰지 않는다. 매 빌드마다 mtime이 갱신되면
        // `tauri dev` 워처가 무한 재빌드에 빠진다.
        let unchanged = fs::read_to_string(&cap_path).ok().as_deref() == Some(content);
        if !unchanged {
            fs::write(&cap_path, content).expect("write dev-mcp capability");
        }
    } else if cap_path.exists() {
        fs::remove_file(&cap_path).ok();
    }
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
    matches!(
        name.to_ascii_lowercase().as_str(),
        "heif.dll" | "libde265.dll" | "aom.dll"
    )
}

/// 설치 프로그램에 동봉할 라이선스 원문을 `generated/licenses/`로 모은다.
/// vcpkg의 `copyright`(libheif, libde265, aom)와 저장소의 서드파티 고지 문서를 넣는다.
fn collect_license_files() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let dest_dir = manifest_dir.join("generated").join("licenses");
    let _ = fs::create_dir_all(&dest_dir);

    let mut sources = vec![(
        manifest_dir.join("..").join("THIRD_PARTY_LICENSES.md"),
        "THIRD_PARTY_LICENSES.md".to_string(),
    )];
    if let Some(root) = env::var_os("VCPKG_ROOT").or_else(|| env::var_os("VCPKG_INSTALLATION_ROOT"))
    {
        let share = Path::new(&root)
            .join("installed")
            .join("x64-windows")
            .join("share");
        for pkg in ["libheif", "libde265", "aom"] {
            sources.push((
                share.join(pkg).join("copyright"),
                format!("{pkg}-copyright.txt"),
            ));
        }
    }
    for (src, name) in sources {
        let Ok(content) = fs::read(&src) else {
            continue;
        };
        let dest = dest_dir.join(name);
        // 내용이 같으면 쓰지 않는다. mtime 갱신이 `tauri dev` 재빌드를 유발한다.
        if fs::read(&dest).ok().as_deref() != Some(content.as_slice()) {
            let _ = fs::write(&dest, content);
        }
        println!("cargo:rerun-if-changed={}", src.display());
    }
}
