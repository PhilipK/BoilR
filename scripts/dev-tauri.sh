#!/usr/bin/env bash
set -euo pipefail
repo_root="$(dirname "$0")/.."
frontend_dir="$repo_root/apps/boilr-tauri"
if [ ! -d "$frontend_dir/node_modules" ]; then
  echo "Installing frontend dependencies..."
  (cd "$frontend_dir" && npm install)
fi
cd "$frontend_dir"
exec cargo tauri dev
