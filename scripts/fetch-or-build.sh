#!/bin/sh
# fetch-or-build.sh — herdr [[build]] step for herdr-popup-resource-manager.
#
# Adapted from herdr-file-viewer's scripts/fetch-or-build.sh (MIT, https://github.com/smarzban/herdr-file-viewer).
#
# Fast path: download the prebuilt binary for THIS source's version and platform from the GitHub release,
# verify its SHA-256 against the release's SHA256SUMS, and install it at target/release/. On any miss (no
# asset for this version or platform, download error, checksum mismatch, no curl/wget/sha tool) say why and
# build from source with cargo instead.
#
# PRM_REPO_ROOT / PRM_OUT / PRM_BASE_URL override the paths and release URL (used by the hermetic tests).
set -u

repo="Ruslanbek0809/herdr-popup-resource-manager"
bin="herdr-popup-resource-manager"

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root="${PRM_REPO_ROOT:-$script_dir/..}"
out="${PRM_OUT:-$repo_root/target/release/$bin}"
base_url="${PRM_BASE_URL:-https://github.com/$repo/releases/download}"

have() { command -v "$1" >/dev/null 2>&1; }

build_from_source() {
  # herdr may run without ~/.cargo/bin on PATH (e.g. started from a GUI).
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
  if ! have cargo; then
    echo "$bin: no prebuilt binary could be installed and cargo was not found. Building from source needs Rust" \
      "1.95+ (https://rustup.rs) and network access to crates.io. Then re-run: herdr plugin install $repo" >&2
    exit 1
  fi
  cd "$repo_root" && exec cargo build --release --locked
}

fallback() {
  echo "$bin: $1 — building from source instead." >&2
  [ -n "${tmpdir:-}" ] && rm -rf "$tmpdir"
  build_from_source
}

download() { # download <url> <dest>
  if have curl; then
    curl -fsSL -o "$2" "$1"
  elif have wget; then
    wget -q -O "$2" "$1"
  else
    return 127
  fi
}

sha256_of() {
  if have sha256sum; then
    sha256sum "$1" | awk '{print $1}'
  elif have shasum; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    return 127
  fi
}

os=$(uname -s 2>/dev/null || echo unknown)
arch=$(uname -m 2>/dev/null || echo unknown)
triple=""
case "$os/$arch" in
  Darwin/arm64 | Darwin/aarch64) triple="aarch64-apple-darwin" ;;
  Darwin/x86_64) triple="x86_64-apple-darwin" ;;
  Linux/x86_64 | Linux/amd64) triple="x86_64-unknown-linux-musl" ;;
  Linux/aarch64 | Linux/arm64) triple="aarch64-unknown-linux-musl" ;;
esac
[ -n "$triple" ] || fallback "no prebuilt binary for $os/$arch"

version=$(grep -E '^version *= *"' "$repo_root/Cargo.toml" 2>/dev/null | head -n 1 | sed -E 's/^version *= *"([^"]+)".*/\1/')
[ -n "$version" ] || fallback "could not read the version from Cargo.toml"

asset="$bin-$triple"
tmpdir=$(mktemp -d 2>/dev/null) || fallback "could not create a temp dir"
trap 'rm -rf "$tmpdir"' EXIT

download "$base_url/v$version/$asset" "$tmpdir/$asset" || fallback "prebuilt binary not available for v$version ($asset)"
download "$base_url/v$version/SHA256SUMS" "$tmpdir/SHA256SUMS" || fallback "checksums not available for v$version"

# `  name` (text mode) or ` *name` (binary mode) both verify.
expected=$(grep -E "^[0-9a-f]{64} [ *]$asset\$" "$tmpdir/SHA256SUMS" 2>/dev/null | awk '{print $1}' | head -n 1)
[ -n "$expected" ] || fallback "no checksum listed for $asset"
actual=$(sha256_of "$tmpdir/$asset") || fallback "no sha-256 tool (sha256sum/shasum) available"
[ "$actual" = "$expected" ] || fallback "checksum mismatch for $asset (expected $expected, got $actual)"

chmod +x "$tmpdir/$asset"
mkdir -p "$(dirname "$out")"
mv -f "$tmpdir/$asset" "$out" || fallback "could not install the verified binary to $out"
echo "$bin: installed prebuilt v$version ($triple), SHA-256 verified."
exit 0
