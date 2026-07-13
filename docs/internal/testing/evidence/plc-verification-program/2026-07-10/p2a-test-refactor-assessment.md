# Existing-Test Refactor Assessment

Generator: `test-refactor-assessment v1`
Source revision: `b32dc62ee71dd66b2985f94342d5c19bd70ae559`
Generated: `2026-07-13T16:48:52+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `b03a62e982d1f238255f95adc2579de7f101464ae86246b4f242e71f15615af6`
Input SHA-256: `sha256:f899e8276fab91d960f6a5d8a04ee6b492cb26bc7f02cc8948255a4d5baf6790`

Size is a review signal, not a refactor decision.
Mechanical similarity is candidate evidence only; it never authorizes
a move, split, rename, fixture merge, or behavior change.

## Summary

- Scanner facts: 3823
- Fact-bearing files: 675
- Large-file candidates: 24
- Reviewed mapping-diversity candidates: 0
- Broad multi-invariant claim candidates: 0
- Exact fact-file duplicate groups: 0
- Whitespace-normalized fact-file duplicate groups: 0
- Exact case-input duplicate groups: 0
- Same-table structural case-input peer groups: 9
- Shared case-file reference groups: 1
- Malformed-class overlap groups: 0
- VS Code facts: 456
- VS Code files: 38
- VS Code registrations: 38
- Large registered VS Code files: 5
- Catalog records: 27
- Scanner facts with reviewed duration: 22
- Scanner facts without reviewed duration: 3801
- Catalog rows explicitly classified slow: 1
- Reviewed proposal decisions: 1
- Assessment-supported decisions: 1

## Large Or Mixed-Purpose Signals

| Path | Lines | Facts | Reviewed mappings | Signals |
| --- | ---: | ---: | ---: | --- |
| `crates/trust-ads-server/src/commands/tests.rs` | 1287 | 43 | 0 | `large_file` |
| `crates/trust-ads-server/src/listener.rs` | 1374 | 16 | 0 | `large_file` |
| `crates/trust-debug/src/adapter/tests_part_02.rs` | 1175 | 13 | 0 | `large_file` |
| `crates/trust-hir/src/openot_authoring.rs` | 2868 | 22 | 0 | `large_file` |
| `crates/trust-hir/src/symbols/table.rs` | 1136 | 3 | 0 | `large_file` |
| `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs` | 1295 | 69 | 0 | `large_file` |
| `crates/trust-hir/tests/semantic_type_checking/hir_mutation_hardening.rs` | 1316 | 23 | 0 | `large_file` |
| `crates/trust-runtime/src/bin/trust-runtime/ads.rs` | 1562 | 2 | 0 | `large_file` |
| `crates/trust-runtime/src/bin/trust-runtime/cli/tests.rs` | 1543 | 49 | 0 | `large_file` |
| `crates/trust-runtime/src/config/tests.rs` | 1154 | 71 | 0 | `large_file` |
| `crates/trust-runtime/src/control/comm_handlers/browse_symbols.rs` | 1123 | 7 | 0 | `large_file` |
| `crates/trust-runtime/src/control/tests/core.rs` | 5785 | 71 | 0 | `large_file` |
| `crates/trust-runtime/src/host/ads/onboarding/tests.rs` | 1291 | 29 | 0 | `large_file` |
| `crates/trust-runtime/src/host/ads/tests.rs` | 1282 | 35 | 0 | `large_file` |
| `crates/trust-runtime/src/io/mqtt/tests.rs` | 1232 | 30 | 0 | `large_file` |
| `crates/trust-runtime/tests/fixtures/plcopen_motion/coordinated_motion/src/tests.st` | 1748 | 19 | 0 | `large_file` |
| `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st` | 2869 | 50 | 0 | `large_file` |
| `crates/trust-runtime/tests/openot_telemetry.rs` | 3148 | 37 | 0 | `large_file` |
| `crates/trust-runtime/tests/phase11_seam_contract.rs` | 1344 | 23 | 9 | `large_file` |
| `editors/vscode/src/test/suite/hmi.integration.test.ts` | 1445 | 14 | 0 | `large_file` |
| `editors/vscode/src/test/suite/ladder-engine.test.ts` | 1093 | 14 | 0 | `large_file` |
| `editors/vscode/src/test/suite/network-canvas.test.ts` | 2109 | 56 | 0 | `large_file` |
| `editors/vscode/src/test/suite/runtime-controls-contract.test.ts` | 1245 | 47 | 0 | `large_file` |
| `editors/vscode/src/test/suite/ux-shell-contract.test.ts` | 4378 | 158 | 0 | `large_file` |

## Broad Invariant Claims

- `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_INITIALIZER_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_COERCION_RETURN_WIDENING_001` claims 1 invariants; result `single_invariant`.
- `TEST_IEC_TIMER_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_LITERAL_CONTEXT_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_PARAMETER_COPY_IN_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_RESTART_TRACE_001` claims 1 invariants; result `single_invariant`.
- `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001` claims 1 invariants; result `single_invariant`.
- `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` claims 1 invariants; result `single_invariant`.
- `TEST_CASE_TABLE_VM_SEAM_VALID_001` claims 1 invariants; result `single_invariant`.

## Duplicate And Structural Signals

- Exact fact-file groups: 0
- Whitespace-normalized fact-file groups: 0
- Exact case-input groups: 0
- Same-table structural case-input peer groups: 9
- Shared case-file reference groups: 1
- Explicit malformed-class overlap groups: 0
- Free-form source-body similarity: `not_assessed`
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: `VM_SEAM_DECLARED_TYPE_001_COMPATIBLE_LITERAL_INITIALIZER_WIDENING_D8DF321B`, `VM_SEAM_DECLARED_TYPE_001_COMPATIBLE_TYPED_ASSIGNMENT_WIDENING_A6856BB4`, `VM_SEAM_DECLARED_TYPE_001_FUNCTION_RESULT_WIDENING_E52C15B0`, `VM_SEAM_DECLARED_TYPE_001_INT_EXPRESSION_TO_DINT_SLOT_9BF228AA`, `VM_SEAM_DECLARED_TYPE_001_INT_LITERAL_TO_REAL_SLOT_04979927`, `VM_SEAM_DECLARED_TYPE_001_INT_VARIABLE_TO_REAL_SLOT_C3821866`, `VM_SEAM_DECLARED_TYPE_001_POU_OUTPUT_WIDENING_8D957490`; shape `sha256:1be9655b754ca9d884c27bd43460558fb86cb55d995475a1d19b276b5de60ecc`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`: `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_1A1BA936`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_77B93993`, `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_D8E1DB83`; shape `sha256:0d790842b7e2d96191704c9f07776e47e75c71b2d7dec4c06ff4ba9f0187a606`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`: `VM_SEAM_STRING_BOUND_001_MAX_D165B4CE`, `VM_SEAM_STRING_BOUND_001_MIN_D165B4CE`; shape `sha256:3940b6625c5f575bfcae24332dd917448996f46e736d442993370d110b01778a`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`: `VM_SEAM_SUBRANGE_001_MAX_AE132E71`, `VM_SEAM_SUBRANGE_001_MIN_AE132E71`; shape `sha256:74118ebe11ebf1204131d61e0250805ebee19ed88606029843da8d8741cc79ad`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND_100_6DD115EE`, `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND__100_09FC189F`; shape `sha256:4c2dbca3a9792afb543b0301d33ce1a8bd127b78dee5e254a7c0f3350539b70f`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_TRUNCATE_BEFORE_POU_BODIES_D6833A8D`, `VM_SEAM_VALID_001_TRUNCATE_BEFORE_SECTION_TABLE_58B11C2B`; shape `sha256:e0bd21a1e4c5110f2132f018441b9faa042eff4a8587ab2ce394077f907edf8d`.
- Structural peers in `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_80_CA909A71`, `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_FF_32935955`; shape `sha256:7afbe67384583995479cd4b26ae4dfb1e78bf262fb626d55f38a7b05699ab8e3`.
- Structural peers in `verification/cases/compiler_iec/IEC_TIMER_001.toml`: `IEC_TIMER_001_TOF_LTIME_POST_EXPIRY_HOLD`, `IEC_TIMER_001_TOF_TIME_POST_EXPIRY_HOLD`, `IEC_TIMER_001_TON_TIME_BASIC_DELAY`, `IEC_TIMER_001_TP_TIME_BASIC_PULSE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Structural peers in `verification/cases/runtime_safety/RT_SAFE_RESTART_TIME_002.toml`: `RT_SAFE_RESTART_TIME_002_COLD_PRESERVES_TIME_AND_REINITIALIZES_STATE`, `RT_SAFE_RESTART_TIME_002_WARM_PRESERVES_TIME_AND_REINITIALIZES_STATE`; shape `sha256:5583f769954ed3a1265abd03edaee615985ac4cd8dffbd88b1b246bdc76df8e2`.
- Shared case file `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`: tests `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001`, `TEST_CASE_TABLE_VM_SEAM_VALID_001`; record paths `scripts/bytecode_validator_mutation.py`, `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`.

## VS Code Registration

- Discovered facts: 456
- Test files: 38
- Literal registrations: 38
- Diagnostics: 0
- `editors/vscode/src/test/suite/hmi.integration.test.ts`: 1445 lines, 14 facts.
- `editors/vscode/src/test/suite/ladder-engine.test.ts`: 1093 lines, 14 facts.
- `editors/vscode/src/test/suite/network-canvas.test.ts`: 2109 lines, 56 facts.
- `editors/vscode/src/test/suite/runtime-controls-contract.test.ts`: 1245 lines, 47 facts.
- `editors/vscode/src/test/suite/ux-shell-contract.test.ts`: 4378 lines, 158 facts.

## Duration Classification

- Scanner facts listed: 3823
- Artifact catalog rows listed separately: 5
- Ignored, nightly, hardware, and name signals never infer duration.
- Scanner `DISC_043E03287D0BD498DBFE` / `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_093B7EAE0DCB979D4540` / `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001`: `fast` at `crates/trust-runtime/tests/retain_integrity.rs`.
- Scanner `DISC_1257BED418738BD81458` / `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001`: `fast` at `crates/trust-runtime/src/control/tests/hmi_values_write.rs`.
- Scanner `DISC_1D261CB173DC0CF72297` / `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_21561AEEDB07941017B8` / `TEST_RUNTIME_RESTART_TRACE_001`: `fast` at `crates/trust-runtime/tests/runtime_restart_trace_cases.rs`.
- Scanner `DISC_265444021E11E0C2B452` / `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_266A60F3E75C685188EB` / `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_278EEBF061FD97B539D0` / `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_34DA98F67689CD13B551` / `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_475B227B49C510855C19` / `TEST_VM_DECLARED_LITERAL_CONTEXT_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_4E0EFE2F6CE2913C5C99` / `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001`: `fast` at `crates/trust-runtime/tests/runtime_watchdog_output_case.rs`.
- Scanner `DISC_547AC119BF2B3BEFB960` / `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_56F608E0FA7014D18845` / `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_88F921D24D3708CEF3E1` / `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`: `fast` at `crates/trust-runtime/tests/bytecode_container.rs`.
- Scanner `DISC_8E59F13125886D732DA2` / `TEST_VM_DECLARED_PARAMETER_COPY_IN_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_934C37AAF7B29C216166` / `TEST_VM_COERCION_INITIALIZER_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_B7E97F9BAA11D0743107` / `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_C1C1116CE99213C96040` / `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_C5A04B37E39DDBE237C5` / `TEST_IEC_TIMER_TRACE_001`: `fast` at `crates/trust-runtime/tests/iec_timer_trace_cases.rs`.
- Scanner `DISC_D76D0BCB14E615250E0B` / `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001`: `fast` at `crates/trust-runtime/tests/phase11_seam_contract.rs`.
- Scanner `DISC_F103626E494B610C2A79` / `TEST_VM_COERCION_RETURN_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Scanner `DISC_F85382D0B4505D5984DB` / `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001`: `fast` at `crates/trust-runtime/tests/coercion_proof.rs`.
- Artifact `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001`: `slow` `mutation_shard_runner` at `scripts/bytecode_validator_mutation.py`; suites `nightly`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_DECLARED_TYPE_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_STRING_BOUND_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_SUBRANGE_001.toml`; suites `veryquick`.
- Artifact `TEST_CASE_TABLE_VM_SEAM_VALID_001`: `fast` `case_table_artifact` at `verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml`; suites `veryquick`.
- Commandless suites: `supporting_local`
- Placeholder suites: none
- Catalog rows without suite tiers: `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`
- Unknown assigned suites: none

## Reviewed Proposal Decisions

- `TEST_REFACTOR_BYTECODE_CONTAINER_INVALID_MAGIC_001`: disposition `no_refactor_needed`, supported `yes`, sources `crates/trust-runtime/tests/bytecode_container.rs`, observed signals none.

## Limitations

- Large-file findings are mechanical line counts at the reviewed inclusive threshold.
- Mixed-purpose findings require multiple reviewed catalog areas or test classes; names and source text never establish purpose.
- Broad-claim findings require multiple catalog invariants; catalog v2 has no authorized coverage-dimension field.
- Duplicate findings compare committed whole-file bytes and whitespace-normalized whole-file text; they do not infer semantic similarity.
- Fixture helper functions and helper-only files are not assessed as duplicate fixtures in this slice.
- Malformed-input overlap comes only from explicit malformed_input_class_ids in reviewed catalog rows.
- Duration classifications come only from hand-owned catalog metadata; unclassified scanner facts receive no inferred duration.
- A supported proposal means its disposition agrees with visible assessment signals; it does not authorize a move, split, or rename.
- Mechanical signals never authorize a move, rename, or split; change dispositions remain unsupported in this v1 assessment.
- The single-identity proposal model refuses split rather than under-modeling multiple targets.
- Completed moves and renames require case-file-bound lock proof; catalog rows without that binding remain blocked.
- The mutable evidence index is globally validated but excluded from the report digest closure to avoid self-reference.
- Platform is historical generation provenance; at-rest validation cannot rederive a prior host platform.
