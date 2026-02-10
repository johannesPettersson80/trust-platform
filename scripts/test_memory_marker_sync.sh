#!/usr/bin/env bash
set -euo pipefail

echo "[memory-marker] parse coverage (%MX/%MB/%MW/%MD/%ML)"
cargo test -p trust-runtime --test io_address parse_addresses

echo "[memory-marker] wildcard mismatch coverage (%M* with wrong area)"
cargo test -p trust-runtime --test io_wildcard wildcard_memory_area_mismatch

echo "[memory-marker] %MW cycle sync regression"
cargo test -p trust-runtime --test vars_access var_config_memory_binding_syncs_with_program_storage

echo "[memory-marker] full marker variant sync via VAR_CONFIG"
cargo test -p trust-runtime --test vars_access memory_variants_sync_via_var_config_wildcards

echo "[memory-marker] PASS"
