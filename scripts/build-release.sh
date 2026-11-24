#!/bin/sh
# Release archives of ja1front with SDL2 linked in (no SDL2 needed on the player's machine).
#   scripts/build-release.sh linux     -> dist/ja1-port-VERSION-linux-x86_64.tar.gz
#   scripts/build-release.sh windows   -> dist/ja1-port-VERSION-windows-x86_64.zip   (cross build from Linux)
# linux: needs cmake, a C compiler and the X11/Wayland/ALSA/PulseAudio headers (SDL2 is built from source;
#        see .github/workflows/release.yml for the Debian/Ubuntu package list). Build on an old distro for a low
#        glibc baseline (the release builds use Ubuntu 22.04: glibc 2.35).
# windows: needs the x86_64-pc-windows-gnu Rust target and mingw-w64; downloads the official SDL2 MinGW
#        development release (static libSDL2.a) into build/.
# VERSION = $VERSION, else `git describe`, else "dev".
set -e
cd "$(dirname "$0")/.."
ROOT=$PWD
WHAT=${1:?usage: scripts/build-release.sh linux|windows}
VERSION=${VERSION:-$(git describe --tags --always 2>/dev/null || echo dev)}
SDL_VERSION=${SDL_VERSION:-2.32.10}
CARGO=${CARGO:-cargo}
mkdir -p dist build

stage() {   # stage DIR BINARY
  rm -rf "$1"; mkdir -p "$1"
  cp "$2" "$1/"
  cp README.md LICENSE THIRD_PARTY.md ja1.cfg.example "$1/"
  cp docs/playing.md docs/keys.md docs/original_bugs.md "$1/"
  mkdir -p "$1/game" && cp game/README.txt "$1/game/"
}

case $WHAT in
linux)
  (cd port/ja1front && CARGO_TARGET_DIR="$ROOT/build/target-linux" $CARGO build --release --locked \
     --features sdl2/bundled,sdl2/static-link)
  B=build/target-linux/release/ja1front
  N=ja1-port-$VERSION-linux-x86_64
  stage "build/$N" "$B"
  cp scripts/install-from-gog.sh "build/$N/"
  tar -C build -czf "dist/$N.tar.gz" "$N"
  echo "dist/$N.tar.gz"
  ;;
windows)
  S=build/SDL2-$SDL_VERSION
  if [ ! -d "$S" ]; then
    curl -sSLf -o build/sdl2-mingw.tar.gz \
      "https://github.com/libsdl-org/SDL/releases/download/release-$SDL_VERSION/SDL2-devel-$SDL_VERSION-mingw.tar.gz"
    tar -C build -xzf build/sdl2-mingw.tar.gz
  fi
  CC=${MINGW_CC:-x86_64-w64-mingw32-gcc}
  # SDL 2.32's registry calls need advapi32, which sdl2-sys 0.37's static link list leaves out.
  (cd port/ja1front && CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=$CC \
     RUSTFLAGS="-L $ROOT/$S/x86_64-w64-mingw32/lib -l advapi32" CARGO_TARGET_DIR="$ROOT/build/target-windows" \
     $CARGO build --release --locked --target x86_64-pc-windows-gnu --features sdl2/static-link)
  B=build/target-windows/x86_64-pc-windows-gnu/release/ja1front.exe
  N=ja1-port-$VERSION-windows-x86_64
  stage "build/$N" "$B"
  rm -f "dist/$N.zip"
  if command -v zip >/dev/null; then (cd build && zip -qr "../dist/$N.zip" "$N")
  else (cd build && python3 -m zipfile -c "../dist/$N.zip" "$N"); fi
  echo "dist/$N.zip"
  ;;
*)
  echo "usage: scripts/build-release.sh linux|windows" >&2; exit 2;;
esac
