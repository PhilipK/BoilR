#!/usr/bin/env bash
# Regenerates the offline cargo sources the Flatpak builds from.
# Run after any change to Cargo.lock.
# Needs curl and python3 (with venv).
set -Eeuo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
tools="$(mktemp -d)"
trap 'rm -rf "$tools"' EXIT

curl -fsSL https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py \
  --output "$tools/flatpak-cargo-generator.py"

python3 -m venv "$tools/venv"
"$tools/venv/bin/pip" install --quiet aiohttp tomlkit

"$tools/venv/bin/python" "$tools/flatpak-cargo-generator.py" \
  "$here/../Cargo.lock" -o "$here/cargo-lock.json"

echo "Updated cargo-lock.json"
