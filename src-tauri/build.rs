use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    sync_dev_mcp_capability();
    copy_libheif_dlls();
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
        if (!unchanged) {
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
        "heif.dll" | "libde265.dll"
    )
}
