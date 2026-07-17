#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GROK_ROOT="$ROOT"
PI_BIN="${PI_BIN:-pi}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: Rust/Cargo is required" >&2
  exit 1
fi
if ! command -v "$PI_BIN" >/dev/null 2>&1; then
  echo "error: system Pi executable not found: $PI_BIN" >&2
  exit 1
fi

(cd "$GROK_ROOT" && cargo build -p xai-grok-pager-bin --bin grok-pi)

echo "Built: $GROK_ROOT/target/debug/grok-pi"
