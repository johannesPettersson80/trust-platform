#!/usr/bin/env bash
set -euo pipefail
fragments=(
 crates/trust-runtime/src/io/addressing.rs
 crates/trust-runtime/src/io/coercion.rs
 crates/trust-runtime/src/io/interface.rs
 crates/trust-runtime/src/runtime/vm/call/tests/support.rs
 crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs
 crates/trust-runtime/src/runtime/vm/call/tests/function_blocks.rs
 crates/trust-runtime/src/runtime/vm/call/tests/write_targets.rs
 crates/trust-runtime/src/runtime/vm/register_ir/tests/support.rs
 crates/trust-runtime/src/runtime/vm/register_ir/tests/profile.rs
 crates/trust-runtime/src/runtime/vm/register_ir/tests/function_blocks.rs
 crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/calls_failures_cache.rs
 crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/state_deadline_buffers.rs
 crates/trust-runtime/src/runtime/vm/register_ir/tests/tier1/load_ref_super_bool.rs
)
if [[ ${1:-} == --check ]]; then
 cargo fmt --all -- --check
 rustfmt --check --edition 2021 --config-path rustfmt.toml "${fragments[@]}"
else
 cargo fmt --all
 rustfmt --edition 2021 --config-path rustfmt.toml "${fragments[@]}"
fi
