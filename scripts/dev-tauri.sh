#!/usr/bin/env bash
set -euo pipefail
repo_root="$(dirname "$0")/.."
frontend_dir="$repo_root/apps/boilr-tauri"
dev_config_root="$repo_root/.dev-config"
mkdir -p "$dev_config_root"
export BOILR_CONFIG_HOME="$dev_config_root"
export XDG_CONFIG_HOME="$dev_config_root"
if [ ! -d "$frontend_dir/node_modules" ]; then
  echo "Installing frontend dependencies..."
  (cd "$frontend_dir" && npm install)
fi
cd "$frontend_dir"
exec cargo tauri dev
