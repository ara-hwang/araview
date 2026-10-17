#!/usr/bin/env bash
# AraView 릴리스 빌드와 macOS 앱 번들(.app)과 디스크 이미지(.dmg)를 만든다.
# 서명과 공증은 하지 않고 ad-hoc 서명만 붙인다(첫 실행은 우클릭 > 열기가 필요하다).
#
#   scripts/package-macos.sh [--skip-build]
#
# 결과물: target/AraView-<버전>-macos-<arch>.dmg (arch: arm64 | x86_64)
# libheif는 vcpkg 정적 빌드(`arm64-osx`/`x64-osx`)로 링크하므로 dylib를 동봉하지 않는다.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
app_src="$root/src-gpui"
release="$root/target/release"

if [[ "${1:-}" != "--skip-build" ]]; then
  (cd "$root" && cargo build --release -p araview-gpui)
fi
[[ -x "$release/araview" ]] || { echo "릴리스 실행 파일이 없습니다: $release/araview" >&2; exit 1; }

version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$app_src/Cargo.toml" | head -1)"
arch="$(uname -m)"
work="$root/target/macos-stage"
bundle="$work/AraView.app"
out="$root/target/AraView-$version-macos-$arch.dmg"

rm -rf "$work"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m 755 "$release/araview" "$bundle/Contents/MacOS/araview"
sed "s/@VERSION@/$version/g" "$app_src/packaging/macos/Info.plist" > "$bundle/Contents/Info.plist"

# 1024px 원본에서 iconset을 만들어 icns로 묶는다.
iconset="$work/AraView.iconset"
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" "$app_src/resources/icon.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" "$app_src/resources/icon.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$bundle/Contents/Resources/AraView.icns"

# 앱의 라이선스 창은 실행 파일 옆 `licenses/`를 읽는다.
if compgen -G "$release/licenses/*" >/dev/null; then
  mkdir -p "$bundle/Contents/MacOS/licenses"
  cp "$release"/licenses/* "$bundle/Contents/MacOS/licenses/"
else
  echo "경고: 네이티브 라이브러리 저작권 문서가 없습니다($release/licenses)." >&2
fi

codesign --force --deep --sign - "$bundle"

ln -s /Applications "$work/Applications"
rm -f "$out"
hdiutil create -volname "AraView" -srcfolder "$work" -ov -format UDZO "$out" >/dev/null
echo "macOS 이미지: $out"
