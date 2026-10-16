#!/usr/bin/env bash
# Opens the resource manager as a herdr popup (bound to a key in ~/.config/herdr/config.toml).
# Without python3 >= 3.9 the popup would flash and vanish, so say why instead.
set -euo pipefail
herdr_bin="${HERDR_BIN_PATH:-herdr}"

if ! command -v python3 >/dev/null || ! python3 -c 'import sys; sys.exit(sys.version_info < (3, 9))'; then
  "$herdr_bin" notification show "Popup Resource Manager" \
    --body "Needs python3 3.9+ (install the Xcode Command Line Tools: xcode-select --install)"
  exit 1
fi

exec "$herdr_bin" plugin pane open \
  --plugin herdr-popup-resource-manager \
  --entrypoint resource-manager \
  --placement popup
