#!/usr/bin/env bash
# AraView를 사용자 폴더(기본 ~/.local)에 설치한다. 관리자 권한이 필요 없다.
#
#   ./install.sh                  # ~/.local 에 설치
#   PREFIX=/usr/local ./install.sh  # 다른 위치(쓰기 권한 필요)
#   ./install.sh --uninstall
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
prefix="${PREFIX:-$HOME/.local}"
bin_dir="$prefix/lib/araview"
desktop="$prefix/share/applications/com.araview.viewer.desktop"
icon="$prefix/share/icons/hicolor/512x512/apps/com.araview.viewer.png"

refresh_caches() {
  command -v update-desktop-database >/dev/null && update-desktop-database "$prefix/share/applications" || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" || true
}

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -rf "$bin_dir" "$prefix/bin/araview" "$desktop" "$icon"
  refresh_caches
  echo "AraView를 제거했습니다."
  exit 0
fi

mkdir -p "$bin_dir" "$prefix/bin" "$prefix/share/applications" "$(dirname "$icon")"
rm -rf "$bin_dir"
cp -R "$here/bin" "$bin_dir"
ln -sf "$bin_dir/araview" "$prefix/bin/araview"
install -m 644 "$here/share/applications/com.araview.viewer.desktop" "$desktop"
install -m 644 "$here/share/icons/hicolor/512x512/apps/com.araview.viewer.png" "$icon"
# 바로가기가 PATH에 의존하지 않도록 실행 파일의 절대 경로를 적는다.
sed -i "s|^Exec=araview |Exec=$prefix/bin/araview |" "$desktop"
refresh_caches
echo "AraView를 설치했습니다: $prefix/bin/araview"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "참고: $prefix/bin 이 PATH에 없습니다." ;; esac
