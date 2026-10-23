#!/usr/bin/env bash
set -euo pipefail
# Touched include! fragments are not traversed by cargo fmt.
fragments=(
 crates/trust-runtime-core/src/program_model/ops/time_ops.rs
 crates/trust-runtime/src/runtime/vm/call/tests/stdlib_binding.rs
 crates/trust-runtime/src/host/eval/expr/call/tests.rs
)
if [[ ${1:-} == --check ]]; then
 cargo fmt --all -- --check
 cargo fmt --manifest-path firmware/trust-nucleo-f401re/Cargo.toml --all -- --check
 rustfmt --check --edition 2021 --config-path rustfmt.toml "${fragments[@]}"
else
 cargo fmt --all
 cargo fmt --manifest-path firmware/trust-nucleo-f401re/Cargo.toml --all
 rustfmt --edition 2021 --config-path rustfmt.toml "${fragments[@]}"
fi
