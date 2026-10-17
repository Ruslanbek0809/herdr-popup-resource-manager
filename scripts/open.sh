#!/bin/sh
# Opens the resource manager as a herdr popup (bound to a key in ~/.config/herdr/config.toml).
# Without the binary the popup would flash and vanish, so say why instead.
set -eu
herdr_bin="${HERDR_BIN_PATH:-herdr}"
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)

if [ ! -x "$root/target/release/herdr-popup-resource-manager" ]; then
  "$herdr_bin" notification show "Popup Resource Manager" \
    --body "Not built yet: run 'cargo build --release' in $root (herdr plugin link doesn't build)"
  exit 1
fi

exec "$herdr_bin" plugin pane open \
  --plugin herdr-popup-resource-manager \
  --entrypoint resource-manager \
  --placement popup
