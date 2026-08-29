#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -eq 0 ]]; then
  echo "Usage: $0 <command> [argument ...]" >&2
  exit 2
fi

for command_name in setsid pgrep pkill; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    echo "Missing required command: $command_name" >&2
    exit 1
  fi
done

OWNED_SESSION=""

cleanup_owned_session() {
  local _attempt

  if [[ -z "$OWNED_SESSION" ]]; then
    return
  fi

  pkill -TERM -s "$OWNED_SESSION" >/dev/null 2>&1 || true
  for ((_attempt = 0; _attempt < 50; _attempt++)); do
    if ! pgrep -s "$OWNED_SESSION" >/dev/null 2>&1; then
      break
    fi
    sleep 0.1
  done
  pkill -KILL -s "$OWNED_SESSION" >/dev/null 2>&1 || true
  wait "$OWNED_SESSION" >/dev/null 2>&1 || true
  OWNED_SESSION=""
}

terminate() {
  local status="$1"
  trap - INT TERM HUP
  cleanup_owned_session
  exit "$status"
}

trap 'terminate 130' INT
trap 'terminate 143' TERM HUP

setsid "$@" &
OWNED_SESSION=$!

set +e
wait "$OWNED_SESSION"
status=$?
set -e

cleanup_owned_session
exit "$status"
