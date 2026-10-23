#!/usr/bin/env bash
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/cargo_target_path.sh"
target=$(validated_cargo_target "${1:?usage: remove_cargo_target_if_idle.sh TARGET}")
lease_root="$HOME/.cache/codex-target-leases"
mkdir -p "$lease_root"
target_digest=$(printf '%s' "$target" | sha256sum | cut -d' ' -f1)
lease="$lease_root/$target_digest.lock"
exec 9>"$lease"
if ! flock -n 9; then
  echo "Cargo target is active; cleanup skipped: $target" >&2
  exit 75
fi
target=$(validated_cargo_target "$target")
rm -rf -- "$target"
