# Phase 18 code-spec authority reuse review

Date: 2026-07-29

Accepted metadata commit:
`91710d3b27767df53d8a7c0358fa719837685fec`

## Outcome

The clean source report contained 17,227 mapped code facts and zero
code-spec findings. Of its 15,541 explicit `spec_gap` facts, the
review-only exact-path proposal found:

- 568 facts in singular existing-authority groups;
- 71 facts in ambiguous existing-authority groups;
- 14,902 facts with no exact-path authority candidate.

Exact path equality was treated only as a review lead. Every one of the 57
candidate groups received an explicit semantic decision against the complete
invariant claim and every fact in the group. Thirty-five groups containing
356 facts were accepted. Twenty-two groups containing 283 facts were
withheld: eight for ambiguous authority, eleven for mixed semantics, and
three for a direct claim mismatch.

The accepted transaction reduced `spec_gap` from 15,541 to 15,185 and
increased `specified` from 1,651 to 2,007. It did not create a specification,
invariant, test, catalog relation, execution result, reachability result, or
adequacy claim. The 283 withheld candidate facts remain explicit
`spec_gap`, alongside the 14,902 facts with no reusable exact-path authority.

## Semantic review

Acceptance required one active, oracle-eligible invariant in the same area;
an exact source-path and specification-fragment match; nonempty mapped tests;
live invariant backlinks from those tests; and semantic coverage of every
fact in the group. A singular path candidate was withheld when even one fact
had behavior outside the invariant claim.

Examples of the fail-closed review:

- `crates/trust-runtime/src/runtime/core/lifecycle.rs` remained a gap because
  backend-selection authority does not specify watchdog, fault, retain,
  debug, deadline, telemetry, or safe-I/O lifecycle methods.
- `crates/trust-runtime/src/io/interface.rs` remained a gap because a narrow
  typed bit/word round-trip invariant does not specify resizing, binding,
  snapshots, raw area access, or process-image bounds.
- `crates/trust-runtime/src/io/addressing.rs` remained a gap because the
  proposed direct-address parser claim did not cover the scanner facts, which
  were address, binding, snapshot, target, and safe-state types.
- `crates/trust-hir/src/db/queries/collector/variables.rs` remained a gap
  because the reviewed INT constant-initializer claim does not specify the
  file's string, aggregate, subrange, `VAR_CONFIG`, `VAR_ACCESS`, and general
  declaration behavior.
- every group with two or more candidate invariants was withheld as
  `ambiguous_authority`; path proximity was never used to choose among them.

### Accepted groups

| Source path | Existing invariant | Facts |
| --- | --- | ---: |
| `crates/trust-runtime-core/src/value/string_semantics.rs` | `VM_SEAM_STRING_ELEMENT_001` | 5 |
| `crates/trust-dev/src/ci.rs` | `DEV_CI_EXIT_CLASSIFICATION_001` | 7 |
| `crates/trust-dev/src/test_cmd/output.rs` | `DEV_TEST_REPORTING_001` | 14 |
| `crates/trust-plcopen/src/plcopen/codesys_export_meta.rs` | `PLCO_CODESYS_METADATA_001` | 12 |
| `crates/trust-plcopen/src/plcopen/codesys_structure.rs` | `PLCO_CODESYS_METADATA_001` | 3 |
| `crates/trust-plcopen/src/plcopen/export.rs` | `PLCO_ST_COMPLETE_INTERCHANGE_001` | 4 |
| `crates/trust-plcopen/src/plcopen/export_helpers.rs` | `PLCO_EXPORT_ADAPTERS_001` | 8 |
| `crates/trust-plcopen/src/plcopen/profile.rs` | `PLCO_ST_COMPLETE_PROFILE_001` | 1 |
| `crates/trust-runtime/src/bin/trust-runtime/cli/bench.rs` | `RT_BENCH_TRANSPORT_DISPATCH_001` | 1 |
| `crates/trust-runtime/src/bin/trust-runtime/cli/comm.rs` | `RT_OFFLINE_COMM_AUTHORING_CLI_001` | 6 |
| `crates/trust-runtime/src/bin/trust-runtime/cli/commands.rs` | `RT_RUNTIME_CLI_ENTRYPOINT_001` | 4 |
| `crates/trust-runtime/src/bin/trust-runtime/config_ui.rs` | `RT_STANDALONE_IDE_BOOTSTRAP_001` | 1 |
| `crates/trust-runtime/src/bin/trust-runtime/deploy/bundle_io.rs` | `RT_VERSIONED_DEPLOY_STATE_001` | 15 |
| `crates/trust-runtime/src/bin/trust-runtime/deploy/diffs.rs` | `RT_VERSIONED_DEPLOY_STATE_001` | 4 |
| `crates/trust-runtime/src/host/bundle.rs` | `RT_PLAY_LAUNCH_001` | 2 |
| `crates/trust-runtime/src/host/bundle_template.rs` | `RT_OFFLINE_COMM_AUTHORING_CLI_001` | 4 |
| `crates/trust-runtime/src/host/linux_rt.rs` | `RT_LINUX_REALTIME_PROFILE_001` | 27 |
| `crates/trust-runtime/src/host/realtime/realtime_part_01.rs` | `RT_REALTIME_T0_TRANSPORT_001` | 21 |
| `crates/trust-runtime/src/host/realtime/shm.rs` | `RT_REALTIME_T0_TRANSPORT_001` | 26 |
| `crates/trust-runtime/src/host/realtime/transport.rs` | `RT_REALTIME_T0_TRANSPORT_001` | 21 |
| `scripts/verification/code_spec_authority_reuse.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 21 |
| `scripts/verification/code_spec_authority_reuse_projection.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 7 |
| `scripts/verification/code_spec_authority_reuse_selection.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 5 |
| `scripts/verification/code_spec_refresh_projection.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 13 |
| `scripts/verification/code_spec_refresh_projection_support.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 14 |
| `scripts/verification/code_spec_refresh_selection.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 15 |
| `scripts/verification/code_spec_semantic_projection.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 8 |
| `scripts/verification/code_spec_semantic_selection.py` | `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` | 5 |
| `scripts/verification_assertion_inventory_gate.sh` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 1 |
| `xtask/src/test_assertions.rs` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 36 |
| `xtask/src/test_assertions/external_oracles.rs` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 10 |
| `xtask/src/test_assertions/generated_inputs.rs` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 15 |
| `xtask/src/test_assertions/import_resolution.rs` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 10 |
| `xtask/src/test_assertions/repository_source_tree.rs` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 5 |
| `xtask/src/test_assertions/unit_context.rs` | `VERIFICATION_ASSERTION_INVENTORY_CLOSURE_001` | 5 |

### Withheld groups

| Source path | Facts | Reason | Candidate authority |
| --- | ---: | --- | --- |
| `crates/trust-runtime/src/value/reference.rs` | 1 | `mixed_semantics` | `VM_SEAM_STRING_ELEMENT_001` |
| `crates/trust-hir/src/db/queries/collector/variables.rs` | 21 | `mixed_semantics` | `IEC_HIR_VARIABLE_INITIALIZER_CONSTANT_001` |
| `crates/trust-syntax/src/lexer/mod.rs` | 7 | `ambiguous_authority` | pragma-trivia and token-classification invariants |
| `crates/trust-syntax/src/lexer/tokens/token_kind.rs` | 1 | `ambiguous_authority` | pragma-trivia and token-classification invariants |
| `crates/trust-syntax/src/lexer/tokens/token_kind_impl_classification.rs` | 2 | `claim_mismatch` | `IEC_LEXER_PRAGMA_TRIVIA_001` |
| `crates/trust-syntax/src/parser/parser.rs` | 16 | `ambiguous_authority` | six parser invariants |
| `crates/trust-syntax/src/syntax/mod.rs` | 17 | `mixed_semantics` | `TRUST_SYNTAX_KIND_CLASSIFICATION_001` |
| `crates/trust-plcopen/src/plcopen/contracts.rs` | 21 | `mixed_semantics` | `PLCO_MIGRATION_REPORT_AND_EXTENSION_001` |
| `crates/trust-plcopen/src/plcopen/import.rs` | 2 | `ambiguous_authority` | import-failure, non-ST rejection, and interchange invariants |
| `crates/trust-plcopen/src/plcopen/shims_metrics.rs` | 15 | `mixed_semantics` | `PLCO_VENDOR_LIBRARY_SHIMS_001` |
| `crates/trust-runtime-core/src/watchdog.rs` | 24 | `ambiguous_authority` | fault-record and watchdog-policy invariants |
| `crates/trust-runtime/src/bin/trust-harness.rs` | 1 | `ambiguous_authority` | three harness invariants |
| `crates/trust-runtime/src/bin/trust-runtime/cli/ads.rs` | 3 | `ambiguous_authority` | two ADS CLI invariants |
| `crates/trust-runtime/src/bin/trust-runtime/run/commands.rs` | 4 | `mixed_semantics` | `RT_PLAY_LAUNCH_001` |
| `crates/trust-runtime/src/bin/trust-runtime/run/types.rs` | 1 | `claim_mismatch` | `RT_RUN_STARTUP_POLICY_001` |
| `crates/trust-runtime/src/host/execution_backend.rs` | 15 | `mixed_semantics` | `RT_EXECUTION_BACKEND_SELECTION_001` |
| `crates/trust-runtime/src/host/metrics.rs` | 32 | `mixed_semantics` | `RT_RUNTIME_METRICS_PROFILING_001` |
| `crates/trust-runtime/src/host/realtime/realtime_part_02.rs` | 17 | `ambiguous_authority` | T0 cycle and transport invariants |
| `crates/trust-runtime/src/io/addressing.rs` | 10 | `claim_mismatch` | `RT_DIRECT_ADDRESS_PARSER_001` |
| `crates/trust-runtime/src/io/interface.rs` | 30 | `mixed_semantics` | `RT_IO_TYPED_ROUNDTRIP_001` |
| `crates/trust-runtime/src/runtime/ads_subsystem.rs` | 15 | `mixed_semantics` | `RT_ADS_ACTIVE_DEVICE_ROUTE_MATCHING_001` |
| `crates/trust-runtime/src/runtime/core/lifecycle.rs` | 28 | `mixed_semantics` | `RT_EXECUTION_BACKEND_SELECTION_001` |

The complete group IDs and decisions are frozen in
`verification/p18/review-locks/code-spec-authority-reuse-lock.json`.

## Review-only transaction

The authority-reuse tooling was implemented through focused test-first
slices. The initial nine tests failed because the proposal, projection,
application, and real-gate registration contracts did not exist. Those nine
tests passed after implementation, followed by a 79/79 focused
authority/semantic/refresh/metadata suite on the clean builder source.

The accepted tool:

- requires a clean zero-finding source report;
- proposes only exact-path, exact-fragment, same-area active authority with
  live test backlinks;
- partitions singular, ambiguous, and no-candidate facts without accepting
  any of them automatically;
- requires one decision for every candidate group and rejects acceptance of
  ambiguous groups;
- reconstructs the proposal live before projection and again immediately
  before mutation;
- updates only the selected code-map disposition fields and owning gap
  affected-code lists;
- applies both canonical ledgers and the application manifest through one
  recoverable compare-and-swap transaction.

The source transaction is bound by:

- source commit:
  `c295f7a8b07ffb232a6907a2130a18dc2fb04710`;
- source report:
  `sha256:553b156eda5a9446282fb7b7e68a1f301ebe26e1ff0aa931f4ce48d4d3bc8b11`;
- source input digest:
  `sha256:9e16079b113719f07cbfa6374714edc8a1dd7bb11bc3e16d4b6fc1d5234509d8`;
- source proposal:
  `sha256:d206443345177ce90d49a33542fb5fbee7802f91786b7ac04b4a2ff2969db878`;
- operation-set digest:
  `sha256:e4878dc4a500fd91e07cea220adc6ddadbed3695c084317b6cb91a821271fd04`;
- reviewed selection:
  `sha256:1baf694916cb784ba340d05485143d234a9fd36700c4fb59710077081744c824`;
- application:
  `sha256:38027e8c744a864c7694f4086df398e4e792050140b858f93c7ce73a6cdc706f`.

The canonical code map moved from
`sha256:220e67b1ea014ad7dcb2cb373021a075868444e45067d6cb8cee54b33b917991`
to
`sha256:17cea30baed1115d78700723d6cb8a97b2739c24a5cec693caaa780e668f70ee`.
The spec-gap ledger moved from
`sha256:d288a353c4356d323b840bc1e2e4c3ee82a180f3c4aeedb3f5ff84a0619d62fe`
to
`sha256:401e26a43a5d2c6572f7799ca6d602d40c088066385d82dbbb7d1579d9909c93`.

## Accepted remote proof

Canonical checkout:
`/home/johannes/projects/trust-platform`

Clean validation checkout:
`trust-builder:/home/johannes/projects/trust-platform-p18-marker-0d31ff12`

Bootstrap:

- remote host: `scena-rust-builder`;
- destination HEAD:
  `91710d3b27767df53d8a7c0358fa719837685fec`;
- destination branch: detached;
- root `AGENTS.md`: exact match to the remote primary checkout;
- `.codex/skills/**`: 25/25 files, exact recursive match.

The focused post-application authority, semantic, and refresh suite passed
62/62. The clean code-spec report is
`sha256:6041178a6bea440fc4573b283b2c91246c393c2895db834b66c919e43f926f72`
with input digest
`sha256:5ced5137a756306de65a289697b612c6786894a36624926561735b0b583e3647`.
It contains 17,227 live = 17,227 mapped facts, zero findings, 15,185
`spec_gap`, 2,007 `specified`, 30 `inherits_spec`, two `generated`, two
`non_behavioral`, and one `vendored_external`.

The post-application identity/semantic refresh proposal is empty:
`sha256:c8ceca45c145d9c22f149625b2aa3f8bbbb40c28c0cb5db9d9451081968008d2`
with operation-set digest
`sha256:9c029f348397cd6f8be5c8b9c925de27b5584f9039f2d3e6a67327505d84fdd4`.
The next authority-reuse proposal is
`sha256:04ed56be1a7b016439042320017e32df9548e1b1c01954ba38624f4d2d77a054`
with operation-set digest
`sha256:f07c048fc7b08ed6b761df1c121a2f8c36253f05f4b51eb42c13cef2e791154f`.
It retains exactly the 22 withheld groups: 212 singular-candidate facts and
71 ambiguous-candidate facts.

The changed-path planner exited 6 on the same known repository-wide Phase 18
metadata debt: 1,877 findings. The authority transaction itself introduced
no code-spec finding or identity/semantic refresh item.

No `just fmt`, `just clippy`, `just test`, `just test-all`, coverage campaign,
mutation campaign, or final broad gate ran.

## Boundary

This batch closes reusable existing specification authority for 356 exact
code facts only. It does not close `VERIF-P18-003` globally. The remaining
15,185 `spec_gap` facts, 173 gap-open coverage cells, eleven historical
placeholder invariants, missing-test review, execution, bidirectional-chain,
reachability, mutation, adequacy, environment, candidate, release, and
independent-review rows remain open.
