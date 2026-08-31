#!/usr/bin/env bash
set -euo pipefail

state_dir=${OPENOT_DATABASE_RUNNER_STATE_DIR:-${RUNNER_TEMP:-/tmp}/trust-openot-databases}
prefix=${OPENOT_DATABASE_RUNNER_PREFIX:-trust-openot-${GITHUB_RUN_ID:-local}}

validate_prefix() {
  [[ $1 =~ ^trust-openot-[A-Za-z0-9_.-]+$ ]] || {
    echo "refusing unsafe OpenOT runner prefix" >&2
    exit 2
  }
}
validate_prefix "$prefix"

marker="$state_dir/.trust-openot-runner-state"
if [[ ! -e $state_dir ]]; then
  exit 0
fi
if [[ -L $state_dir || -L $marker ]]; then
  echo "refusing symlinked OpenOT runner state" >&2
  exit 2
fi
if [[ $(basename "$state_dir") != trust-openot-* || ! -f $marker ]] \
  || [[ $(<"$marker") != "$prefix" ]]; then
  echo "refusing unowned OpenOT runner state" >&2
  exit 2
fi

containers=(
  "$prefix-postgres" "$prefix-timescale" "$prefix-mysql"
  "$prefix-mariadb" "$prefix-sqlserver" "$prefix-influx"
  "$prefix-influx-tls"
)
docker rm -f "${containers[@]}" >/dev/null 2>&1 || true
docker network rm "$prefix-network" >/dev/null 2>&1 || true

if [[ -d $state_dir/sqlserver ]]; then
  sudo chown -R "$(id -u):$(id -g)" "$state_dir/sqlserver"
fi
rm -rf -- "$state_dir"
