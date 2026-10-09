#!/usr/bin/env bash
# Shared filesystem policy for generated Cargo targets, leases and reclamation.
set -euo pipefail

validated_cargo_target() {
  local candidate=${1%/}
  local volume=/mnt/HC_Volume_107089260
  local volume_targets="$volume/builder-storage/cargo-targets"
  case "$candidate" in
    "$HOME/.cache/codex-targets/"*|/tmp/*) ;;
    "$volume_targets/"*)
      if ! mountpoint -q -- "$volume"; then
        echo "Cargo target volume is not mounted: $volume" >&2
        return 2
      fi
      ;;
    *)
      echo "refusing unsafe Cargo target path: $candidate" >&2
      return 2
      ;;
  esac
  case "/$candidate/" in
    */../*|*/./*)
      echo "refusing noncanonical Cargo target path: $candidate" >&2
      return 2
      ;;
  esac
  local resolved
  resolved=$(realpath -m -- "$candidate") || return 2
  if [[ "$resolved" != "$candidate" ]]; then
    echo "refusing aliased Cargo target path: $candidate" >&2
    return 2
  fi
  local existing=$candidate
  while [[ ! -e "$existing" && ! -L "$existing" ]]; do
    existing=$(dirname -- "$existing")
  done
  if [[ "$existing" != /tmp && ! -O "$existing" ]]; then
    echo "Cargo target or its existing parent is not owned by the current user: $existing" >&2
    return 2
  fi
  printf '%s\n' "$candidate"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  validated_cargo_target "${1:?usage: cargo_target_path.sh TARGET}"
fi
