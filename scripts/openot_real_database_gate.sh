#!/usr/bin/env bash
set -euo pipefail

# Required release gate for the OpenOT persistence support matrix. Database
# products and TLS credentials are provisioned by the isolated runner; this
# script deliberately never prints connection strings, passwords, or tokens.

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
artifact_root=${TRUST_OPENOT_EVIDENCE_DIR:-"$repo_root/target/openot-real-database-evidence"}
mkdir -p "$artifact_root"

required_variables=(
  TRUST_TEST_OPENOT_POSTGRES_URL TRUST_TEST_OPENOT_POSTGRES_CA
  TRUST_TEST_OPENOT_TIMESCALE_URL TRUST_TEST_OPENOT_TIMESCALE_CA
  TRUST_TEST_OPENOT_MYSQL_URL TRUST_TEST_OPENOT_MYSQL_CA
  TRUST_TEST_OPENOT_MARIADB_URL TRUST_TEST_OPENOT_MARIADB_CA
  TRUST_TEST_OPENOT_SQLSERVER_URL TRUST_TEST_OPENOT_SQLSERVER_CA
  TRUST_TEST_OPENOT_INFLUX_HOST TRUST_TEST_OPENOT_INFLUX_TOKEN
  TRUST_TEST_OPENOT_INFLUX_CA
)

missing=()
for variable in "${required_variables[@]}"; do
  if [[ -z ${!variable:-} ]]; then
    missing+=("$variable")
  fi
done
if (( ${#missing[@]} != 0 )); then
  echo "OpenOT real-database gate is missing required secret/environment inputs:" >&2
  printf '  %s\n' "${missing[@]}" >&2
  exit 2
fi

for ca_variable in \
  TRUST_TEST_OPENOT_POSTGRES_CA TRUST_TEST_OPENOT_TIMESCALE_CA \
  TRUST_TEST_OPENOT_MYSQL_CA TRUST_TEST_OPENOT_MARIADB_CA \
  TRUST_TEST_OPENOT_SQLSERVER_CA TRUST_TEST_OPENOT_INFLUX_CA; do
  if [[ ! -r ${!ca_variable} ]]; then
    echo "$ca_variable does not name a readable CA certificate" >&2
    exit 2
  fi
done

cd "$repo_root"
if candidate_sha=$(git rev-parse HEAD 2>/dev/null); then
  candidate_branch=$(git branch --show-current)
  provenance=git-checkout
else
  candidate_sha=${TRUST_OPENOT_CANDIDATE_SHA:?set TRUST_OPENOT_CANDIDATE_SHA for a non-Git validation copy}
  candidate_branch=${TRUST_OPENOT_CANDIDATE_BRANCH:-detached-validation-copy}
  provenance=declared-validation-copy
fi
{
  echo "candidate_sha=$candidate_sha"
  echo "candidate_branch=$candidate_branch"
  echo "candidate_provenance=$provenance"
  echo "runner_arch=$(uname -m)"
  echo "runner_kernel=$(uname -sr)"
  echo "rustc=$(rustc --version)"
  echo "cargo=$(cargo --version)"
  echo "secret_values=redacted"
  printf 'configured_input=%s\n' "${required_variables[@]}"
} >"$artifact_root/candidate.txt"

container_variables=(
  TRUST_TEST_OPENOT_POSTGRES_CONTAINER TRUST_TEST_OPENOT_TIMESCALE_CONTAINER
  TRUST_TEST_OPENOT_MYSQL_CONTAINER TRUST_TEST_OPENOT_MARIADB_CONTAINER
  TRUST_TEST_OPENOT_SQLSERVER_CONTAINER TRUST_TEST_OPENOT_INFLUX_CONTAINER
)
if command -v docker >/dev/null 2>&1; then
  : >"$artifact_root/products.txt"
  for variable in "${container_variables[@]}"; do
    container=${!variable:-}
    if [[ -n $container ]]; then
      docker inspect --format \
        "$variable name={{.Name}} image={{.Config.Image}} digest={{.Image}} status={{.State.Status}}" \
        "$container" >>"$artifact_root/products.txt"
    fi
  done
fi

run_gate() {
  local name=$1
  shift
  echo "running $name"
  "$@" 2>&1 | tee "$artifact_root/$name.log"
}

run_gate adapter-contracts cargo test -p trust-runtime \
  --features openot-real-database-tests --lib \
  openot_persistence::contract_tests -- --test-threads=1
run_gate service-lifecycle cargo test -p trust-runtime \
  --features openot-real-database-tests --lib \
  openot_persistence::service::tests -- --test-threads=1
run_gate authored-workload cargo test -p trust-runtime \
  --features openot-real-database-tests --test openot_telemetry \
  openot_database_example_persists_same_real_st_workload_to_every_network_backend \
  -- --exact
run_gate system-loss-placeholder cargo test -p trust-runtime \
  --features openot-real-database-tests --test openot_database_system_documents \
  runtime_system_loss_and_placeholder_documents_round_trip_through_every_real_product \
  -- --exact

sha256sum "$artifact_root"/*.log >"$artifact_root/log-sha256.txt"
echo "OpenOT real-database gate passed for $candidate_sha"
