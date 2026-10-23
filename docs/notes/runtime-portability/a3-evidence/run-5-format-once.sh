#!/usr/bin/env bash
set -euo pipefail
fragments=(
  crates/trust-runtime/src/bytecode/encoder/codegen/jumps_consts.rs
  crates/trust-runtime/src/host/harness/compiler/config/entry.rs
  crates/trust-runtime/src/bytecode/encoder/pou/build.rs
  crates/trust-runtime/src/bytecode/encoder/pou/entries.rs
  crates/trust-runtime/src/bytecode/encoder/pou/class_meta.rs
  crates/trust-runtime/src/bytecode/encoder/codegen/expr.rs
  crates/trust-runtime/src/bytecode/encoder/codegen/call_expr.rs
  crates/trust-runtime/src/bytecode/encoder/codegen/dynamic_access.rs
  crates/trust-runtime/src/host/harness/compiler/config/globals_access.rs
  crates/trust-runtime/src/host/harness/compiler/pou/program_vars.rs
  crates/trust-runtime/src/host/harness/io/types.rs
  crates/trust-runtime/src/host/harness/io/direct_field_bindings.rs
  crates/trust-runtime/src/io/addressing.rs
  crates/trust-runtime/src/io/interface.rs
  crates/trust-runtime/src/runtime/core/accessors.rs
)
if [[ ${1:-} == --check ]]; then
  cargo fmt --all -- --check
  rustfmt --check --edition 2021 --config-path rustfmt.toml "${fragments[@]}"
else
  cargo fmt --all
  rustfmt --edition 2021 --config-path rustfmt.toml "${fragments[@]}"
fi
