//! 업데이트 흐름을 로컬에서 끝까지 돌려 보기 위한 개발 도구.
//!
//! 주어진 설치 프로그램을 임시 minisign 키로 서명하고, 출력 폴더에 설치 프로그램
//! 사본, `latest-gpui.json`, 공개키(`pubkey.txt`)를 쓴다. 출력 폴더를 HTTP로
//! 띄운 뒤 디버그 빌드를 `ARAVIEW_UPDATE_FEED`, `ARAVIEW_UPDATE_PUBKEY`와 함께
//! 실행한다(`docs/development.md`). 릴리스 키와는 무관하다.
//!
//! ```text
//! cargo run --example local_update_feed -- <setup.exe> <out-dir> <base-url> [version]
//! ```

use std::io::Cursor;
use std::path::Path;

use base64::Engine as _;

fn encode(text: String) -> String {
    base64::engine::general_purpose::STANDARD.encode(text)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [installer, out_dir, base_url, rest @ ..] = args.as_slice() else {
        return Err("usage: local_update_feed <setup.exe> <out-dir> <base-url> [version]".into());
    };
    let version = rest.first().map(String::as_str).unwrap_or("99.0.0");
    let bytes = std::fs::read(installer)?;
    let name = Path::new(installer)
        .file_name()
        .ok_or("installer has no file name")?
        .to_string_lossy()
        .into_owned();

    let keys = minisign::KeyPair::generate_unencrypted_keypair()?;
    let signature = minisign::sign(Some(&keys.pk), &keys.sk, Cursor::new(&bytes), None, None)?;

    let out = Path::new(out_dir);
    std::fs::create_dir_all(out)?;
    std::fs::write(out.join(&name), &bytes)?;
    let feed = serde_json::json!({
        "version": version,
        "notes": "Local update test feed.",
        "platforms": {
            "windows-x86_64": {
                "url": format!("{}/{name}", base_url.trim_end_matches('/')),
                "signature": encode(signature.to_string()),
            }
        }
    });
    std::fs::write(
        out.join("latest-gpui.json"),
        serde_json::to_vec_pretty(&feed)?,
    )?;
    std::fs::write(
        out.join("pubkey.txt"),
        encode(keys.pk.to_box()?.to_string()),
    )?;
    println!("feed: {}", out.join("latest-gpui.json").display());
    Ok(())
}
