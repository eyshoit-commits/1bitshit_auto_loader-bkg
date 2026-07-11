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
      printf '%s\n' 'Usage: ./install.sh [--backend auto|cpu|cuda|rocm|bitnet] [--yes] [--no-legacy-alias] [--no-migrate] [--no-launch]'
      exit 0
      ;;
    *) fail "Unknown option: $1" ;;
  esac
done

command -v cargo >/dev/null 2>&1 || fail "cargo wurde nicht gefunden"

cd "$REPO_ROOT"
echo "[Installer] Baue BitShit Component Manager..."
cargo build --release --bin bitshit-manager

echo "[Installer] Installation abgeschlossen."
echo "[Installer] Manager starten mit:"
echo "  cargo run --release --bin bitshit-manager"
echo "[Installer] Grafischen Assistenten starten mit:"
echo "  cargo run --release --bin bitshit-installer"

if [[ "$NO_LAUNCH" -eq 0 && -t 0 && -t 1 ]]; then
  exec cargo run --release --bin bitshit-manager
fi
