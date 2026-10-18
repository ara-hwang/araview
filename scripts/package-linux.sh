#!/usr/bin/env bash
# AraView 릴리스 빌드와 Linux 배포 묶음(tar.gz)을 만든다. 서명과 업로드는 하지 않는다.
#
#   scripts/package-linux.sh [--skip-build]
#
# 결과물: target/AraView-<버전>-linux-x86_64.tar.gz
# 안에는 실행 파일, 라이선스 자료, `.desktop` 파일, 아이콘, 설치 스크립트가 들어 있다.
# libheif는 vcpkg 정적 빌드(`x64-linux`)로 링크하므로 따로 동봉하지 않는다.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app="$root/src-gpui"
release="$root/target/release"

if [[ "${1:-}" != "--skip-build" ]]; then
  (cd "$root" && cargo build --release -p araview-gpui)
fi
[[ -x "$release/araview" ]] || { echo "릴리스 실행 파일이 없습니다: $release/araview" >&2; exit 1; }

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$app/Cargo.toml" | head -1)"
arch="$(uname -m)"
name="AraView-$version-linux-$arch"
stage="$root/target/linux-stage/$name"
out="$root/target/$name.tar.gz"

rm -rf "$root/target/linux-stage"
mkdir -p "$stage/bin" "$stage/share/applications" "$stage/share/icons/hicolor/512x512/apps"
install -m 755 "$release/araview" "$stage/bin/araview"
install -m 644 "$app/packaging/linux/com.araview.viewer.desktop" "$stage/share/applications/"
install -m 644 "$app/resources/icon.png" "$stage/share/icons/hicolor/512x512/apps/com.araview.viewer.png"
install -m 755 "$app/packaging/linux/install.sh" "$stage/install.sh"
# 앱의 라이선스 창은 실행 파일 옆 `licenses/`를 읽는다.
if compgen -G "$release/licenses/*" >/dev/null; then
  mkdir -p "$stage/bin/licenses"
  cp "$release"/licenses/* "$stage/bin/licenses/"
else
  echo "경고: 네이티브 라이브러리 저작권 문서가 없습니다($release/licenses)." >&2
fi
install -m 644 "$root/LICENSE" "$stage/LICENSE"

mkdir -p "$(dirname "$out")"
tar -C "$root/target/linux-stage" -czf "$out" "$name"
echo "Linux 묶음: $out"
