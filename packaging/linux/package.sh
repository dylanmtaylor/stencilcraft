#!/usr/bin/env bash
# Build and package StencilCraft for Linux (<arch> is x86_64 or aarch64):
#
#   $DIST/stencilcraft-<version>-linux-<arch>.AppImage
#   $DIST/stencilcraft-<version>-linux-<arch>.deb
#   $DIST/stencilcraft-<version>-linux-<arch>.rpm
#   $DIST/stencilcraft-<version>-linux-<arch>.tar.gz
#
# Usage: packaging/linux/package.sh [--skip-build] [--formats "appimage deb rpm tar"]
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
APP_ID=ai.storyteller.stencilcraft

SKIP_BUILD=0
FORMATS="appimage deb rpm tar"
while [ $# -gt 0 ]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --formats) FORMATS="$2"; shift 2 ;;
    -h | --help) sed -n '2,10p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

ARCH="${CROSS_ARCH:-$(uname -m)}"
case "$ARCH" in
  riscv64) DEB_ARCH=riscv64 ;;
  x86_64) DEB_ARCH=amd64 ;;
  aarch64 | arm64) ARCH=aarch64; DEB_ARCH=arm64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac
export STENCILCRAFT_MAINTAINER="${STENCILCRAFT_MAINTAINER:-StencilCraft maintainers <stencilcraft@storyteller.ai>}"
BASENAME="stencilcraft-$VERSION-linux-$ARCH"

echo "==> StencilCraft $VERSION for Linux $ARCH ($FORMATS)"

if [ "$SKIP_BUILD" = 0 ]; then
  if [ -n "${CROSS_TARGET:-}" ]; then
    (cd "$ROOT" && CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release -p stencilcraft -p stencilcraft-cli --target "$CROSS_TARGET")
  else
    (cd "$ROOT" && CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build --release -p stencilcraft -p stencilcraft-cli)
  fi
fi
BIN="$CARGO_TARGET_DIR/${CROSS_TARGET:+$CROSS_TARGET/}release"
WORK="$CARGO_TARGET_DIR/linux-package"
STAGE="$WORK/root"
rm -rf "$WORK"

# ---- stage an FHS tree (shared by every format) -------------------------------------------------
install -Dm755 "$BIN/stencilcraft" "$STAGE/usr/bin/stencilcraft"
install -Dm755 "$BIN/stencilcraft-cli" "$STAGE/usr/bin/stencilcraft-cli"
"${CROSS_COMPILE:-}strip" "$STAGE/usr/bin/stencilcraft" "$STAGE/usr/bin/stencilcraft-cli" 2>/dev/null || true
install -Dm644 "$HERE/$APP_ID.desktop" "$STAGE/usr/share/applications/$APP_ID.desktop"
install -Dm644 "$HERE/$APP_ID.mime.xml" "$STAGE/usr/share/mime/packages/$APP_ID.xml"
mkdir -p "$STAGE/usr/share/metainfo"
sed -e "s/@VERSION@/$VERSION/g" -e "s/@DATE@/$STENCILCRAFT_BUILD_DATE/g" \
  "$HERE/$APP_ID.metainfo.xml.in" >"$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml"
mkdir -p "$STAGE/usr/share/icons"
cp -R "$ROOT/assets/app-icon/hicolor" "$STAGE/usr/share/icons/"
mkdir -p "$STAGE/usr/share/doc/stencilcraft"
copy_docs "$STAGE/usr/share/doc/stencilcraft"

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$STAGE/usr/share/applications/$APP_ID.desktop"
fi
if command -v appstreamcli >/dev/null; then
  appstreamcli validate --no-net --explain "$STAGE/usr/share/metainfo/$APP_ID.metainfo.xml" || true
fi

has() { case " $FORMATS " in *" $1 "*) return 0 ;; *) return 1 ;; esac; }

# ---- .tar.gz ------------------------------------------------------------------------------------
if has tar; then
  mkdir -p "$WORK/tar"
  cp -R "$STAGE/usr" "$WORK/tar/$BASENAME"
  tar -C "$WORK/tar" -czf "$DIST/$BASENAME.tar.gz" "$BASENAME"
  echo "wrote $DIST/$BASENAME.tar.gz"
fi

if [ -z "${CROSS_TARGET:-}" ]; then
  "$STAGE/usr/bin/stencilcraft-cli" help >/dev/null
fi
echo "==> done"
ls -lh "$DIST"
