# Phase 18 supply-chain and platform authority closeout

Date: 2026-08-12

Branch: `plc-verification-program`

Reviewed production baseline: `a5213de8774ba1aa16aef9a71d3ae0e668e2a8ef`

## Scope

This batch closes the final 11 code facts in the historical
`supply_chain_platform` specification gap:

- ten runtime bundle-builder functions are bound to the existing
  `SPEC_RUNTIME_ENGINE_001#runtime-bundle-builder-source-and-dependency-resolution`
  contract, `RT_BUILD_COMMAND_001`, and the exact existing builder tests;
- `crates/trust-runtime/build.rs` is bound to the new
  `SPEC_RUNTIME_ENGINE_001#windows-runtime-executable-stack-link-contract`,
  `RT_WINDOWS_STACK_LINK_001`, and
  `TEST_RUNTIME_WINDOWS_STACK_LINK_001`.

No production source was changed. The Windows contract records the already
shipped target/toolchain decision and explicitly leaves native Windows linking
and execution for the later platform-proof row.

## Focused behavior lock

The initial Cargo invocation was stopped after it exposed a cold local target;
dependency compilation is not accepted as proof. The same std-only integration
test was then compiled and executed directly:

```text
CARGO_MANIFEST_DIR=/home/johannes/projects/trust-platform/crates/trust-runtime \
  rustc --edition=2021 --test \
  crates/trust-runtime/tests/windows_stack_build_script.rs \
  -o <temporary>/windows_stack_build_script_test
<temporary>/windows_stack_build_script_test \
  windows_stack_link_arguments_match_toolchain_and_other_targets_emit_none \
  --exact --nocapture
```

Result: `1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` in 0.24 s.
This is local Linux/aarch64 behavior-lock proof of the build-script output
matrix, not native Windows artifact proof.

## Atomic mapping validation

The reviewed projection selected exactly 11 facts with selected-set digest
`sha256:7a0ad159ff67b8083f3c942db5753708e3bf3faf77da10bee467d41be66c6405`.
`build_projection.py --apply` replaced the code map, gap ledger, test catalog,
and denominator in one recoverable transaction and ran
`validate_refresh_post_state` inside the rollback boundary. It passed with:

- 11 selected facts now `specified`;
- 0 remaining `supply_chain_platform/spec_gap` code facts;
- one exact scanner identity,
  `DISC_1F40E9F04D6C110FEB7F`, for the new behavior lock;
- no transaction journal left behind; the zero-byte lock file remains as the
  normal reusable serialization lock.

Projection manifest SHA-256:
`1c87439b8cb06dd14ae923cc36fd582dfe91cf1d5f1d3c428bb1e620cbb67e23`.
Generator SHA-256:
`45c31f8bc776eb18c006980df319d246af6e858bdf29b68f6b03af87d7f8bfad`.

No broad, full, or hour-long test suite was run. Dynamic reachability,
mutation, native Windows, and final broad validation remain assigned to their
later Phase 18 rows.
