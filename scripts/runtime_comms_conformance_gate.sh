#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

OUT_DIR="${OUT_DIR:-target/gate-artifacts/runtime-comms-conformance}"
mkdir -p "${OUT_DIR}"

run_observed() {
  local phase="$1"
  local target="$2"
  local timeout="$3"
  local log_path="$4"
  shift 4
  python3 ./scripts/run_with_progress.py \
    --phase "${phase}" \
    --target "${target}" \
    --timeout-seconds "${timeout}" \
    --progress-interval-seconds "${GATE_PROGRESS_INTERVAL_SECONDS:-30}" \
    --log "${log_path}" \
    -- "$@"
}

run_lib_case() {
  local case_id="$1"
  local test_filter="$2"
  local log_path="${OUT_DIR}/${case_id}.log"
  echo "[conformance-gate] running lib case ${case_id}"
  run_observed "runtime-comms-conformance" "${case_id}" "${GATE_TEST_TIMEOUT_SECONDS:-900}" "${log_path}" \
    cargo test -p trust-runtime --lib "${test_filter}" -- --nocapture
}

run_it_case() {
  local case_id="$1"
  local test_target="$2"
  local test_filter="$3"
  local log_path="${OUT_DIR}/${case_id}.log"
  echo "[conformance-gate] running integration case ${case_id}"
  run_observed "runtime-comms-conformance" "${case_id}" "${GATE_TEST_TIMEOUT_SECONDS:-900}" "${log_path}" \
    cargo test -p trust-runtime --test "${test_target}" "${test_filter}" -- --nocapture
}

echo "[conformance-gate] suite: t0-shm"
run_lib_case "t0_shm_bind_contract_mismatch" "realtime::tests::t0_shm_contract_mismatch_is_rejected_before_run"
run_lib_case "t0_shm_stale_retry" "realtime::tests::t0_read_surfaces_stale_after_bounded_misses_and_spin_limit"
run_lib_case "t0_shm_overrun_accounting" "realtime::tests::t0_publish_and_read_track_overrun_and_latest_payload"
run_it_case "t0_shm_process_restart_rebind" "realtime_t0_integration" "realtime_t0_multi_process_shm_exchange_succeeds"

echo "[conformance-gate] suite: zenoh-mesh"
run_lib_case "mesh_presence_transitions" "runtime_cloud::projection::tests::presence_projection_transitions_stale_before_partitioned"
run_lib_case "mesh_catalog_epoch_refresh" "runtime_cloud::contracts::tests::catalog_epoch_cache_requests_refresh_only_on_monotonic_increase"
run_it_case "mesh_query_budget_timeout" "web_io_config_integration" "runtime_cloud_dispatch_cancels_fanout_when_query_budget_is_exhausted"
run_it_case "mesh_secure_transport_advertisement" "web_io_config_integration" "runtime_cloud_discovery_endpoint_exposes_secure_transport_metadata"

echo "[conformance-gate] suite: gateway-bridge"
run_it_case "gateway_default_deny_cross_site" "web_io_config_integration" "runtime_cloud_preflight_denies_cross_site_cfg_apply_without_allowlist"
run_it_case "gateway_allowlist_cross_site" "web_io_config_integration" "runtime_cloud_preflight_allows_cross_site_cfg_apply_with_allowlist"
run_it_case "gateway_degraded_partial_partition" "web_io_config_integration" "runtime_cloud_preflight_marks_partial_partition_target_as_stale"
run_it_case "gateway_witness_loss_demotion" "web_io_config_integration" "runtime_cloud_ha_lease_expiry_demotes_active_runtime_preflight"

echo "[conformance-gate] suite: config-rollout"
run_it_case "config_reconcile_desired_reported_status" "web_io_config_integration" "runtime_cloud_config_agent_reconciles_desired_reported_meta_and_status"
run_it_case "config_revision_etag_conflict" "web_io_config_integration" "runtime_cloud_config_desired_write_enforces_revision_and_etag_conflict"
run_it_case "config_conflict_rebase_retry" "web_io_config_integration" "runtime_cloud_config_conflict_rebase_retry_applies_latest_desired"
run_it_case "config_retained_desired_late_join" "web_io_config_integration" "runtime_cloud_config_agent_recovers_pending_state_after_restart"
run_it_case "rollout_state_machine" "web_io_config_integration" "runtime_cloud_rollout_state_machine_covers_happy_failed_and_aborted_paths"

echo "[conformance-gate] suite: audit-ha"
run_it_case "ha_split_brain_partition" "web_io_config_integration" "runtime_cloud_ha_split_brain_preflight_denies_dual_active_candidates"
run_it_case "ha_dual_output_prevention" "web_io_config_integration" "runtime_cloud_ha_dual_output_prevention_blocks_standby_dispatch"
run_it_case "audit_allowlist_change" "web_io_config_integration" "runtime_cloud_wan_allowlist_policy_change_is_audited"
run_it_case "audit_dispatch_correlation" "web_io_config_integration" "runtime_cloud_dispatch_reaches_remote_runtime_and_propagates_audit_correlation_id"
run_it_case "audit_success_failure_emission" "web_io_config_integration" "runtime_cloud_remote_dispatch_emits_audit_for_success_and_failure_paths"
run_it_case "audit_dedup_event_id" "web_io_config_integration" "runtime_cloud_ha_replay_guard_deduplicates_and_rejects_stale_seq"

cat > "${OUT_DIR}/summary.md" <<'MD'
# Runtime Comms Conformance Gate

Suites:

- t0-shm
- zenoh-mesh
- gateway-bridge
- config-rollout
- audit-ha

Result: PASS
MD

jq -n '
{
  suites: [
    {
      id: "t0-shm",
      cases: [
        "t0_shm_bind_contract_mismatch",
        "t0_shm_stale_retry",
        "t0_shm_overrun_accounting",
        "t0_shm_process_restart_rebind"
      ]
    },
    {
      id: "zenoh-mesh",
      cases: [
        "mesh_presence_transitions",
        "mesh_catalog_epoch_refresh",
        "mesh_query_budget_timeout",
        "mesh_secure_transport_advertisement"
      ]
    },
    {
      id: "gateway-bridge",
      cases: [
        "gateway_default_deny_cross_site",
        "gateway_allowlist_cross_site",
        "gateway_degraded_partial_partition",
        "gateway_witness_loss_demotion"
      ]
    },
    {
      id: "config-rollout",
      cases: [
        "config_reconcile_desired_reported_status",
        "config_revision_etag_conflict",
        "config_conflict_rebase_retry",
        "config_retained_desired_late_join",
        "rollout_state_machine"
      ]
    },
    {
      id: "audit-ha",
      cases: [
        "ha_split_brain_partition",
        "ha_dual_output_prevention",
        "audit_allowlist_change",
        "audit_dispatch_correlation",
        "audit_success_failure_emission",
        "audit_dedup_event_id"
      ]
    }
  ],
  result: "pass"
}
' > "${OUT_DIR}/summary.json"

echo "[conformance-gate] PASS"
