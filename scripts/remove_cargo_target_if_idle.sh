#!/usr/bin/env bash
set -euo pipefail

target=${1:?usage: remove_cargo_target_if_idle.sh TARGET}
case "$target" in
  "$HOME/.cache/codex-targets/"*|/tmp/*) ;;
  *)
    echo "refusing unsafe Cargo target cleanup path: $target" >&2
    exit 2
    ;;
esac

lease_root="$HOME/.cache/codex-target-leases"
mkdir -p "$lease_root"
target_digest=$(printf '%s' "$target" | sha256sum | cut -d' ' -f1)
lease="$lease_root/$target_digest.lock"
exec 9>"$lease"
if ! flock -n 9; then
  echo "Cargo target is active; cleanup skipped: $target" >&2
  exit 75
fi
rm -rf -- "$target"
