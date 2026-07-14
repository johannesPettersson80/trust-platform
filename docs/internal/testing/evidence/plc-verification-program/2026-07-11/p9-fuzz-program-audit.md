# Phase 9 Fuzz Program Audit

Generator: `fuzz-program-audit v1`
Source revision: `27694b329a62206b51cb8392378d6eb9ee0fd8e2`
Generated: `2026-07-14T23:30:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `0c20bd678d395e994b4208c0c645a37501ec0bbc4eec92cc48481ab71f94cd94`
Input SHA-256: `sha256:65c92be8fc2fee3c989bb1bc76b521c0988fbce54b301f83903b7bf07fbf0c08`

This is a report-only inventory of existing fuzz targets, deterministic
fuzz-like smokes, required surfaces, execution profiles, and target gaps.
It runs no campaign and creates no proof or invariant coverage.

## Summary

- Inventory targets: 11
- Cargo-fuzz targets: 5
- Bounded Rust smokes: 6
- Required surfaces: 8
- Gap surfaces: 6

## Required Surfaces

| Surface | Area | Association state | Direct targets | Partial targets |
| --- | --- | --- | --- | --- |
| `st_lexer_parser` | `compiler_iec` | `cargo_fuzz_target` | `FUZZ_SMOKE_PARSER_INITIALIZER_RECOVERY_PROPERTY`, `FUZZ_TARGET_SYNTAX_PARSE` | none |
| `hir_lowering_input` | `compiler_iec` | `partial_only` | none | `FUZZ_TARGET_HIR_SEMANTIC` |
| `plcopen_xml` | `plcopen_devtools` | `unmapped` | none | none |
| `bytecode_container_instructions` | `bytecode_vm` | `smoke_only` | `FUZZ_SMOKE_VM_MALFORMED_BYTECODE` | none |
| `protocol_payloads` | `protocols` | `cargo_fuzz_target` | `FUZZ_SMOKE_MESH_PAYLOAD`, `FUZZ_SMOKE_RUNTIME_CLOUD_API`, `FUZZ_SMOKE_SHM_HEADER`, `FUZZ_TARGET_ADS_AMS_FRAME`, `FUZZ_TARGET_ADS_BOUNDARY_NOOP`, `FUZZ_TARGET_ADS_COMMAND_DISPATCH` | `FUZZ_SMOKE_WAN_ALLOWLIST` |
| `config_files` | `runtime_safety` | `unmapped` | none | none |
| `lsp_incremental_edits` | `editor_safety` | `partial_only` | none | `FUZZ_TARGET_HIR_SEMANTIC` |
| `hmi_schema_payloads` | `hmi_ui` | `unmapped` | none | none |

## Target Inventory

| Target | Kind | Ignore state | Primary tier | Additional tiers | Enforcement | Source |
| --- | --- | --- | --- | --- | --- | --- |
| `FUZZ_TARGET_SYNTAX_PARSE` | `cargo_fuzz` | `not_applicable` | `pr_smoke` | `nightly` | `wired` | `fuzz/fuzz_targets/syntax_parse.rs` |
| `FUZZ_TARGET_HIR_SEMANTIC` | `cargo_fuzz` | `not_applicable` | `pr_smoke` | `nightly` | `wired` | `fuzz/fuzz_targets/hir_semantic.rs` |
| `FUZZ_TARGET_ADS_AMS_FRAME` | `cargo_fuzz` | `not_applicable` | `manual_extended` | none | `manual_only` | `crates/trust-ads-server/fuzz/fuzz_targets/ams_frame.rs` |
| `FUZZ_TARGET_ADS_BOUNDARY_NOOP` | `cargo_fuzz` | `not_applicable` | `manual_extended` | none | `manual_only` | `crates/trust-ads-server/fuzz/fuzz_targets/boundary_noop.rs` |
| `FUZZ_TARGET_ADS_COMMAND_DISPATCH` | `cargo_fuzz` | `not_applicable` | `manual_extended` | none | `manual_only` | `crates/trust-ads-server/fuzz/fuzz_targets/command_dispatch.rs` |
| `FUZZ_SMOKE_VM_MALFORMED_BYTECODE` | `bounded_rust_smoke` | `not_ignored` | `nightly` | none | `planned` | `crates/trust-runtime/tests/bytecode_vm_core/fuzz_stack_call.rs` |
| `FUZZ_SMOKE_MESH_PAYLOAD` | `bounded_rust_smoke` | `not_ignored` | `pr_smoke` | none | `wired` | `crates/trust-runtime/src/host/mesh/tests.rs` |
| `FUZZ_SMOKE_SHM_HEADER` | `bounded_rust_smoke` | `not_ignored` | `pr_smoke` | none | `wired` | `crates/trust-runtime/src/host/realtime/realtime_tests_part_01.rs` |
| `FUZZ_SMOKE_RUNTIME_CLOUD_API` | `bounded_rust_smoke` | `not_ignored` | `pr_smoke` | none | `wired` | `crates/trust-runtime/src/runtime_cloud/routing.rs` |
| `FUZZ_SMOKE_WAN_ALLOWLIST` | `bounded_rust_smoke` | `not_ignored` | `pr_smoke` | none | `wired` | `crates/trust-runtime/src/runtime_cloud/profile_policy.rs` |
| `FUZZ_SMOKE_PARSER_INITIALIZER_RECOVERY_PROPERTY` | `bounded_rust_smoke` | `not_ignored` | `pr_smoke` | none | `wired` | `crates/trust-syntax/tests/parser_variables.rs` |

## Surface Gaps

| Surface | Current state | Gap reason | Associated targets |
| --- | --- | --- | --- |
| `hir_lowering_input` | `partial_only` | `no_direct_surface_target` | `FUZZ_TARGET_HIR_SEMANTIC` |
| `plcopen_xml` | `unmapped` | `no_associated_target` | none |
| `bytecode_container_instructions` | `smoke_only` | `no_cargo_fuzz_target` | `FUZZ_SMOKE_VM_MALFORMED_BYTECODE` |
| `config_files` | `unmapped` | `no_associated_target` | none |
| `lsp_incremental_edits` | `partial_only` | `no_direct_surface_target` | `FUZZ_TARGET_HIR_SEMANTIC` |
| `hmi_schema_payloads` | `unmapped` | `no_associated_target` | none |

## Primary Tier Counts

- `pr_smoke`: 7
- `nightly`: 1
- `manual_extended`: 3

## Additional Tier Counts

- `pr_smoke`: 0
- `nightly`: 2
- `manual_extended`: 0

## Surface State Counts

- `cargo_fuzz_target`: 2
- `smoke_only`: 1
- `partial_only`: 2
- `unmapped`: 3

## Corpus And Crash Handoff

- Working corpus storage: `machine_local_ignored`
- Raw crash storage: `machine_local_ignored`
- Corpus contents assessed: `false`
- Crash-to-regression enforcement: `not_enforced`

## Boundaries

- `report_creates_proof`: `false`
- `report_creates_invariant_coverage`: `false`
- `report_closes_spec_gaps`: `false`
- `semantic_oracles_assessed`: `false`
- `fuzz_campaign_executed`: `false`
- `corpus_contents_assessed`: `false`
- `crash_freedom_claimed`: `false`
- `p9_005_crash_regression_row_remains_open`: `true`
- `phase2_scanner_scope_changed`: `false`
- `runtime_or_product_behavior_changed`: `false`
- `ci_enforcement_changed`: `false`

## Limitations

- Cargo-fuzz facts come from every tracked root or crate-local fuzz/Cargo.toml; the historical Phase 2 scanner remains unchanged.
- Fuzz-like Rust candidates are production-scanner facts selected by the closed fuzz/property_smoke, constrained randomized/arbitrary smoke, or property-framework name vocabulary; names create candidates, never surface associations.
- Unmodeled proptest, quickcheck, or bolero source markers fail visibly, and the reviewed fuzz-gate command parsers reject extra filtered tests even when their names use no fuzz token.
- Direct and partial surface associations are reviewed planning metadata. They are not invariant coverage, an assessed oracle, or passing proof.
- A smoke_only surface has deterministic generated breadth but still appears as a gap because it has no cargo-fuzz target.
- Working corpus and raw crash contents are ignored machine-local or transient CI state and are deliberately not read, counted, digested, or treated as durable evidence.
- The inventory records existing and planned execution paths but executes no fuzz campaign and changes no suite or CI wiring.
- Every bounded Rust smoke is live-joined as not_ignored; ignored or conditional facts cannot retain a runnable tier claim.
- The Rust candidate census is lexical and does not prove ordinary cfg evaluation or parent-module reachability; wired means a reviewed required command path, not observed test execution.
- VERIF-P9-005 remains open because no exhaustive machine registry joins every minimized crash to a committed deterministic regression.
- The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.
