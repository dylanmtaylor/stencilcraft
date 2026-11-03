#!/usr/bin/env bash
# Build StencilCraft from this checkout and install it as a user Flatpak in one step:
# release tarball (package.sh), then Flatpak bundle + install + smoke checks (flatpak-bundle.sh).
#
# Usage: packaging/linux/install-flatpak.sh
# Run it with `flatpak run ai.storyteller.stencilcraft` afterwards.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

"$HERE/package.sh" --formats tar
"$HERE/flatpak-bundle.sh"
