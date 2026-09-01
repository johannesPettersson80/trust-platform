#!/usr/bin/env bash
set -euo pipefail

state_dir=${OPENOT_DATABASE_RUNNER_STATE_DIR:-${RUNNER_TEMP:-/tmp}/trust-openot-databases}
prefix=${OPENOT_DATABASE_RUNNER_PREFIX:-trust-openot-${GITHUB_RUN_ID:-local}}
ownership_label="com.trust.openot.runner=$prefix"

validate_prefix() {
  [[ $1 =~ ^trust-openot-[A-Za-z0-9_.-]+$ ]] || {
    echo "refusing unsafe OpenOT runner prefix" >&2
    exit 2
  }
}
validate_prefix "$prefix"

marker="$state_dir/.trust-openot-runner-state"
if [[ -L $state_dir || -L $marker ]]; then
  echo "refusing symlinked OpenOT runner state" >&2
  exit 2
fi
state_present=false
if [[ -e $state_dir ]]; then
  if [[ $(basename "$state_dir") != trust-openot-* || ! -f $marker ]] \
    || [[ $(<"$marker") != "$prefix" ]]; then
    echo "refusing unowned OpenOT runner state" >&2
    exit 2
  fi
  state_present=true
fi

containers=()
networks=()
while IFS= read -r container; do
  [[ -n $container ]] && containers+=("$container")
done < <(docker ps -aq --filter "label=$ownership_label")
while IFS= read -r network; do
  [[ -n $network ]] && networks+=("$network")
done < <(docker network ls -q --filter "label=$ownership_label")
if [[ $state_present == true && ${#containers[@]} == 0 ]]; then
  containers=(
    "$prefix-postgres" "$prefix-timescale" "$prefix-mysql"
    "$prefix-mariadb" "$prefix-sqlserver" "$prefix-influx"
    "$prefix-influx-tls"
  )
fi
if [[ $state_present == true && ${#networks[@]} == 0 ]]; then
  networks=("$prefix-network")
fi
volumes=()
if (( ${#containers[@]} != 0 )); then
  mapfile -t volumes < <(
    for container in "${containers[@]}"; do
      docker inspect --format \
        '{{range .Mounts}}{{if eq .Type "volume"}}{{println .Name}}{{end}}{{end}}' \
        "$container" 2>/dev/null || true
    done | sed '/^$/d' | sort -u
  )
fi
if (( ${#containers[@]} != 0 )); then
  docker rm -f "${containers[@]}" >/dev/null 2>&1 || true
fi
if (( ${#volumes[@]} != 0 )); then
  docker volume rm "${volumes[@]}" >/dev/null 2>&1 || true
fi
if (( ${#networks[@]} != 0 )); then
  docker network rm "${networks[@]}" >/dev/null 2>&1 || true
fi

if [[ $state_present == true && -d $state_dir/sqlserver ]]; then
  sudo chown -R "$(id -u):$(id -g)" "$state_dir/sqlserver"
fi
if [[ $state_present == true ]]; then
  rm -rf -- "$state_dir"
fi
