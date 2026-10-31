#!/usr/bin/env bash
# Repackage a StencilCraft Linux tarball as a single-file Flatpak bundle:
#
#   $DIST/stencilcraft-<version>-linux-<arch>.flatpak
#
# Usage: packaging/linux/flatpak-bundle.sh [--no-test] [TARBALL]
#
# Uses flatpak-builder when available, or falls back to native `flatpak build-finish` +
# `flatpak build-export` + `flatpak build-bundle` when flatpak-builder is not installed on the host.
set -euo pipefail
# shellcheck source=../env.sh
. "$(dirname "${BASH_SOURCE[0]}")/../env.sh"
HERE="$ROOT/packaging/linux"
APP_ID=ai.storyteller.stencilcraft
FLATHUB=https://dl.flathub.org/repo/flathub.flatpakrepo

TEST=1
OCI=0
TARBALL=""
while [ $# -gt 0 ]; do
  case "$1" in
    --no-test) TEST=0; shift ;;
    --oci) OCI=1; shift ;;
    -h | --help) sed -n '2,10p' "$0"; exit 0 ;;
    -*) echo "unknown argument: $1" >&2; exit 2 ;;
    *) TARBALL="$1"; shift ;;
  esac
done

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) ;;
  aarch64 | arm64) ARCH=aarch64 ;;
  *) echo "unsupported architecture $ARCH" >&2; exit 2 ;;
esac
BASENAME="stencilcraft-$VERSION-linux-$ARCH"
TARBALL="${TARBALL:-$DIST/$BASENAME.tar.gz}"
[ -f "$TARBALL" ] || { echo "error: $TARBALL not found (run packaging/linux/package.sh --formats tar first)" >&2; exit 1; }
command -v flatpak >/dev/null || { echo "error: flatpak not found" >&2; exit 1; }

echo "==> StencilCraft $VERSION Flatpak bundle for $ARCH from $(basename "$TARBALL")"

WORK="${WORK:-/tmp/stencilcraft-flatpak-bundle}"
rm -rf "$WORK"
mkdir -p "$WORK/stage"
tar -xzf "$TARBALL" -C "$WORK/stage" --strip-components=1
[ -x "$WORK/stage/bin/stencilcraft" ] || { echo "error: $TARBALL has no bin/stencilcraft" >&2; exit 1; }
cp "$HERE/flatpak/$APP_ID.bundle.yml" "$WORK/$APP_ID.yml"

flatpak remote-add --user --if-not-exists flathub "$FLATHUB" 2>/dev/null || true

if command -v flatpak-builder >/dev/null; then
  flatpak-builder --user --install-deps-from=flathub --disable-rofiles-fuse --force-clean \
    --default-branch=stable --repo="$WORK/repo" --state-dir="$WORK/state" "$WORK/build" "$WORK/$APP_ID.yml"
else
  echo "==> flatpak-builder not found; building Flatpak directory & OSTree repo natively via flatpak"
  BUILD_DIR="$WORK/build"
  rm -rf "$BUILD_DIR"
  flatpak build-init "$BUILD_DIR" "$APP_ID" org.freedesktop.Platform org.freedesktop.Platform 26.08
  install -Dm755 "$WORK/stage/bin/stencilcraft" "$BUILD_DIR/files/bin/stencilcraft"
  install -Dm755 "$WORK/stage/bin/stencilcraft-cli" "$BUILD_DIR/files/bin/stencilcraft-cli"
  install -Dm644 "$WORK/stage/share/applications/$APP_ID.desktop" "$BUILD_DIR/files/share/applications/$APP_ID.desktop"
  install -Dm644 "$WORK/stage/share/mime/packages/$APP_ID.xml" "$BUILD_DIR/files/share/mime/packages/$APP_ID.xml"
  install -Dm644 "$WORK/stage/share/metainfo/$APP_ID.metainfo.xml" "$BUILD_DIR/files/share/metainfo/$APP_ID.metainfo.xml"
  mkdir -p "$BUILD_DIR/files/share/icons" "$BUILD_DIR/files/share/doc"
  cp -R "$WORK/stage/share/icons/hicolor" "$BUILD_DIR/files/share/icons/"
  cp -R "$WORK/stage/share/doc/stencilcraft" "$BUILD_DIR/files/share/doc/"

  flatpak build-finish "$BUILD_DIR" \
    --command=stencilcraft \
    --socket=wayland \
    --socket=fallback-x11 \
    --share=ipc \
    --device=dri \
    --filesystem=xdg-documents \
    --filesystem=xdg-pictures

  ostree init --mode=archive --repo="$WORK/repo"
  ostree config --repo="$WORK/repo" set core.min-free-space-percent 0
  flatpak build-export "$WORK/repo" "$BUILD_DIR" stable
fi

OUT="$DIST/$BASENAME.flatpak"
flatpak build-bundle --runtime-repo="$FLATHUB" "$WORK/repo" "$OUT" "$APP_ID" stable
echo "wrote $OUT"

if [ "$OCI" = 1 ]; then
  OUT_OCI="$DIST/$BASENAME-oci"
  rm -rf "$OUT_OCI"
  flatpak build-bundle --oci "$WORK/repo" "$OUT_OCI" "$APP_ID" stable
  echo "wrote OCI layout to $OUT_OCI"
fi

if [ "$TEST" = 1 ]; then
  flatpak install --user -y --noninteractive --reinstall "$OUT"
  flatpak info --user "$APP_ID"
  flatpak run --command=stencilcraft-cli "$APP_ID" help >/dev/null
  flatpak run --command=sh "$APP_ID" -c 'ls /app/share/applications /app/share/metainfo /app/share/mime/packages /app/share/icons/hicolor/scalable/apps'
fi
echo "==> done"
ls -lh "$OUT"
