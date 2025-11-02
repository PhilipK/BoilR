#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../apps/boilr-tauri"
if command -v npm >/dev/null 2>&1; then
  exec npm install
else
  echo "npm is required to bootstrap the Tauri frontend." >&2
  exit 1
fi
