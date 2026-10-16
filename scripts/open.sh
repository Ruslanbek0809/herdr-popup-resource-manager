#!/usr/bin/env bash
# Opens the resource manager as a herdr popup (bound to a key in ~/.config/herdr/config.toml).
set -euo pipefail
exec "${HERDR_BIN_PATH:-herdr}" plugin pane open \
  --plugin herdr-resource-manager \
  --entrypoint resource-manager \
  --placement popup
