#!/usr/bin/env bash
set -euo pipefail

policy="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/cargo_target_path.sh"
source "$policy"
target=$(validated_cargo_target "${1:?usage: with_cargo_target_lease.sh TARGET COMMAND...}")
shift
if (( $# == 0 )); then
  echo "with_cargo_target_lease.sh requires a command" >&2
  exit 2
fi

lease_root="$HOME/.cache/codex-target-leases"
mkdir -p "$lease_root"
target_digest=$(printf '%s' "$target" | sha256sum | cut -d' ' -f1)
lease="$lease_root/$target_digest.lock"
# Validate again after taking the lease, before creating or using the target.
# --close keeps detached command children from retaining the outer lease.
exec flock --close -x "$lease" bash -c '
  set -euo pipefail
  source "$1"
  target=$(validated_cargo_target "$2")
  shift 2
  mkdir -p "$target"
  exec "$@"
' bash "$policy" "$target" "$@"
