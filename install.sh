#!/usr/bin/env bash
set -Eeuo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
BACKEND=""
YES=0
NO_LEGACY_ALIAS=0
NO_MIGRATE=0
NO_LAUNCH=0

fail() { printf 'FEHLER: %s\n' "$*" >&2; exit 1; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --backend) [[ $# -ge 2 ]] || fail "--backend requires a value"; BACKEND="$2"; shift 2 ;;
    --yes|-y) YES=1; shift ;;
    --no-legacy-alias) NO_LEGACY_ALIAS=1; shift ;;
    --no-migrate) NO_MIGRATE=1; shift ;;
    --no-launch) NO_LAUNCH=1; shift ;;
    --help|-h)
      printf '%s\n' 'Usage: ./install.sh [--backend auto|cpu|cuda|rocm] [--yes] [--no-legacy-alias] [--no-migrate] [--no-launch]'
      exit 0
      ;;
    *) fail "Unknown option: $1" ;;
  esac
done

OS_NAME="$(uname -s 2>/dev/null || true)"
case "$OS_NAME" in
  Linux|Darwin)
    if command -v cargo >/dev/null 2>&1; then
      echo "[AutoLoader] Bootstrapping modular components..."
      cargo run --bin bitshit-auto-loader 2>/dev/null || true
    else
      echo "[AutoLoader] cargo not found; skipping component bootstrap."
    fi
    ;;
esac

echo "[AutoLoader] Installation complete."
echo "[AutoLoader] Run 'cargo run --bin bitshit-auto-loader' to manage components."
