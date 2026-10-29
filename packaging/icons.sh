#!/usr/bin/env bash
# Regenerate every app icon from assets/app-icon/stencilcraft.svg (the canonical artwork).
#
# Needs: rsvg-convert (librsvg) and python3 with Pillow (for the .ico and .icns). The outputs
# are committed, so building and packaging never need these tools.
#
#   packaging/icons.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/assets/app-icon"
SVG="$DIR/stencilcraft.svg"
ID="ai.storyteller.stencilcraft"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

command -v rsvg-convert >/dev/null || { echo "error: rsvg-convert not found (librsvg)" >&2; exit 1; }
python3 -c 'import PIL' 2>/dev/null || { echo "error: python3 Pillow not found" >&2; exit 1; }

# The SVG is a full-bleed 512 tile, used as-is on Windows and Linux. macOS icons follow Apple's grid:
# the tile is 824/1024 of the canvas with a transparent margin, so widen the viewBox around it.
MAC="$TMP/mac.svg"
sed 's/viewBox="0 0 512 512"/viewBox="-62.136 -62.136 636.272 636.272"/' "$SVG" >"$MAC"
grep -q 'viewBox="-62.136' "$MAC" || { echo "error: $SVG has no 512 viewBox" >&2; exit 1; }

render() { rsvg-convert -w "$2" -h "$2" "$1" -o "$3"; }

render "$SVG" 1024 "$DIR/stencilcraft-1024.png"
render "$MAC" 512 "$DIR/stencilcraft-macos-512.png"

# Linux hicolor theme (the 256 one is also the runtime icon on Windows and Linux).
for s in 16 24 32 48 64 128 256 512; do
  mkdir -p "$DIR/hicolor/${s}x${s}/apps"
  render "$SVG" "$s" "$DIR/hicolor/${s}x${s}/apps/$ID.png"
done
mkdir -p "$DIR/hicolor/scalable/apps"
cp "$SVG" "$DIR/hicolor/scalable/apps/$ID.svg"

# Windows .ico (16 to 256) and macOS .icns (16 to 1024, on Apple's grid).
render "$SVG" 256 "$TMP/ico.png"
render "$MAC" 1024 "$TMP/icns.png"
python3 -I - "$TMP/ico.png" "$DIR/stencilcraft.ico" "$TMP/icns.png" "$DIR/stencilcraft.icns" <<'EOF'
import sys
from PIL import Image
ico, ico_out, icns, icns_out = sys.argv[1:5]
Image.open(ico).save(ico_out, sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
Image.open(icns).save(icns_out)
EOF
echo "icons written to $DIR"
