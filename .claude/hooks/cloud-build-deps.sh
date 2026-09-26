#!/bin/bash
# Installs BoilR's build dependencies in Claude Code cloud sessions (no-op locally).
[ "$CLAUDE_CODE_REMOTE" = "true" ] || exit 0

if ! command -v pkg-config >/dev/null || ! pkg-config --exists openssl; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq && apt-get install -y -qq pkg-config libssl-dev >/dev/null
fi

# CI builds with the latest stable Rust; match it so new lints show up here first.
rustup toolchain install stable --profile minimal -c clippy -c rustfmt >/dev/null 2>&1 && rustup default stable >/dev/null 2>&1

# Warm the dependency cache in the background so the first build is faster.
(cd "$CLAUDE_PROJECT_DIR" && cargo fetch -q >/dev/null 2>&1 &)
exit 0
