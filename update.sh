#!/usr/bin/env bash
set -Eeuo pipefail

REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
COMPONENTS_DIR="${REPO_ROOT}/components"
BACKEND="${1:-auto}"

say() { printf '%s\n' "$*"; }
fail() { printf 'FEHLER: %s\n' "$*" >&2; exit 1; }

if [ ! -d "$COMPONENTS_DIR" ]; then
  mkdir -p "$COMPONENTS_DIR"
fi

cd "$COMPONENTS_DIR"

for repo in 1bitshit_kernel-bkg 1bitshit_driver-bkg 1bitshit_engine-bkg 1bitshit_cli-bkg; do
  if [ -d "$repo/.git" ]; then
    say "[AutoLoader] Updating $repo ..."
    git -C "$repo" fetch origin --prune
    git -C "$repo" switch main
    git -C "$repo" reset --hard origin/main
  else
    say "[AutoLoader] Cloning $repo ..."
    git clone --depth 1 --branch main "https://github.com/eyshoit-commits/$repo.git" "$repo"
  fi
done

say "[AutoLoader] All components up to date."
