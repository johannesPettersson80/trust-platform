#!/usr/bin/env bash
set -euo pipefail

target=${1:?usage: with_cargo_target_lease.sh TARGET COMMAND...}
shift
if (( $# == 0 )); then
  echo "with_cargo_target_lease.sh requires a command" >&2
  exit 2
fi

case "$target" in
  "$HOME/.cache/codex-targets/"*|/tmp/*) ;;
  *)
    echo "refusing unsafe Cargo target lease path: $target" >&2
    exit 2
    ;;
esac

lease_root="$HOME/.cache/codex-target-leases"
mkdir -p "$target" "$lease_root"
target_digest=$(printf '%s' "$target" | sha256sum | cut -d' ' -f1)
lease="$lease_root/$target_digest.lock"
exec flock --close -x "$lease" "$@"
