#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-$HOME/trust-cloud-topology-demo}"
runtime_ids=(a b c d e)

for id in "${runtime_ids[@]}"; do
  pid_file="/tmp/trust-topology-demo-${id}.pid"
  if [[ -f "${pid_file}" ]]; then
    pid="$(cat "${pid_file}" || true)"
    if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
      kill "${pid}" 2>/dev/null || true
      echo "[stop] runtime-${id}: stopped pid ${pid}"
    else
      echo "[stop] runtime-${id}: no live process for pid ${pid}"
    fi
    rm -f "${pid_file}"
  fi
done

# Fallback: kill any runtime started from this demo root path.

echo "[stop] done."
