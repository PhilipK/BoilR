#!/usr/bin/env bash
# Regenerates the offline cargo and npm sources the Flatpak builds from. Run after any change to
# apps/boilr-tauri/Cargo.lock or package-lock.json; CI (flatpak_lock_sync) fails when they drift.
# Needs git and python3 (with venv).
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
tools="$(mktemp -d)"
trap 'rm -rf "$tools"' EXIT

git clone --quiet --depth 1 https://github.com/flatpak/flatpak-builder-tools "$tools/fbt"
python3 -m venv "$tools/venv"
"$tools/venv/bin/pip" install --quiet aiohttp tomlkit "$tools/fbt/node"

"$tools/venv/bin/python" "$tools/fbt/cargo/flatpak-cargo-generator.py" \
  "$here/../Cargo.lock" -o "$here/cargo-sources.json"
"$tools/venv/bin/flatpak-node-generator" \
  npm "$here/../package-lock.json" -o "$here/node-sources.json" > /dev/null

echo "Updated cargo-sources.json and node-sources.json"
