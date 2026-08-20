# Phase 18 Post-Closure Behavior Delta

Status: corrective behavior audit closed; exact-SHA candidate validated.

Current row: none.

## Accepted Baseline

The accepted source baseline is `1f3134524e86ceed2b8ba1369084dfa83d0fb7de`.
The final July 19 closure record is historical evidence for that source tree;
it is not a denominator for new product work.

A later campaign indexed production functions and converted implementation and
test inventory into specification, mapping, and proof debt. That campaign is
retired. The corrective audit considers only surviving observable product
behavior changes after the accepted baseline.

## Audit Rule

The substantive product chain is:

```text
written specification -> native executable test
```

One audit row represents one coherent observable contract group owned by one
written specification section and one directly relevant native suite. Closely
coupled inputs, errors, and state transitions stay together; the audit does not
recreate a per-function or per-assertion inventory. A row may have exactly one
of these decisions:

- `already_covered`: the current written specification and native assertion
  directly agree;
- `missing_spec`: expected behavior is missing or ambiguous in the owning
  product specification;
- `missing_test`: the specification is precise but no native executable
  assertion reaches it;
- `behavior_defect`: a focused native test observes behavior that contradicts
  the written specification; or
- `external_manual`: the written contract intrinsically requires hardware,
  browser rendering, an external implementation, or another real environment.

Invariant status, catalog linkage, denominator disposition, evidence freshness,
proof level, mutation result, scanner fact, file count, and function count
cannot create a behavior-ledger row or change its decision. They may be used
only as historical search aids. In particular, an empty metadata `tests` array
does not establish that a native product test is absent.

## Behavior Ledger

These rows have been checked directly against current specification text and
native assertions:

The accepted-baseline deletion audit is complete for product/spec/test roots:
26 removed web/HMI JavaScript parts are content-preserving consolidations, six
removed lexer enum fragments were unwired duplicates fully represented in the
canonical token enum, the removed old-path LSP golden is represented by the
live module-qualified replacement snapshot, and the one genuinely lost native
ST fixture has been restored verbatim. No other deleted product specification,
native test, or test fixture remains.

| Behavior | Written specification | Native executable test | Decision |
|---|---|---|---|
| CTU/CTD/CTUD edge, priority, saturation, and value-family behavior | `docs/specs/08-standard-function-blocks.md`, Counter Runtime Conformance Contract | `crates/trust-runtime/tests/stdlib_fb_contract.rs` and the existing counter suites | `already_covered` |
| TON/TOF/TP scan, rearm, preset, clock, TIME/LTIME, and type boundaries | `docs/specs/08-standard-function-blocks.md`, Timer Scan-Step and Runtime Boundary contracts | `crates/trust-runtime/tests/stdlib_fb_contract.rs` and the existing timer suites | `already_covered` |
| Connector schema, policy, quality, and protocol projection | `docs/specs/23-connector-status.md` | `crates/trust-runtime/tests/connectors_status.rs` and connector conformance cases | `already_covered` |
| PLCopen Motion rejects the absent `MC_Stop.Active` surface in both program and native ST test-program fixtures | `docs/PLCOPEN_DECISIONS.md`, initial-profile `MC_Stop` signature decision; `docs/guides/PLCOPEN_MOTION_LIBRARY_GUIDE.md`, `MC_Stop` | `crates/trust-runtime/tests/plcopen_motion_oop_library.rs`; `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_negative_stop_active/src/tests.st` | `already_covered`; the accidentally deleted baseline `TEST_PROGRAM` is restored byte-for-byte and its exact-candidate remote native harness remains a final gate |
| Conformance manifest, source containment, and compile-error rejection | `conformance/contract.md`, Case Manifest Contract | `crates/trust-runtime/src/bin/trust-runtime/conformance/tests.rs` | `already_covered` |
| Benchmark jitter requires two latency observations | `docs/specs/11-runtime-engine.md`, benchmark statistics contract | `crates/trust-runtime/src/bin/trust-runtime/bench/tests.rs::jitter_samples_require_two_latency_observations` | `already_covered` |
| ADS sum-up results preserve exact requested identity and cardinality | `docs/specs/11-runtime-engine.md`, ADS onboarding contract | `crates/trust-runtime/src/host/ads/onboarding/wire/tests.rs::sumup_read_projection_rejects_incomplete_or_untrusted_results` | `already_covered` |
| Shipped HMI routes and standalone export use the complete consolidated process/layout/widget/trend/alarm module set instead of a test-only split-file resolver | `docs/guides/HMI_OPERATOR_FIRST_SPECIFICATION.md`, Process rendering, controls, layout, live quality, and exported module set | `crates/trust-runtime/tests/hmi_readonly_integration/hmi_readonly_integration_part_01.rs::hmi_dashboard_routes_render_without_manual_layout`; `hmi_readonly_integration_part_02.rs::hmi_standalone_export_bundle_contains_assets_routes_and_config` | `already_covered`; all 26 removed JS parts are byte-exact consolidated content except blank-line-only widget boundaries, and the native tests inspect the actual served/exported assets |
| Every shipped `runtime.toml` example remains loadable; communication examples retain the complete reviewed project/config/source shape and build/validate successfully | `docs/specs/22-developer-workflows.md`, Shipped project example corpus | `crates/trust-runtime/tests/example_runtime_configs.rs::shipped_example_runtime_configs_are_loadable`; `crates/trust-runtime/tests/communication_examples_cli.rs::communication_examples_build_and_validate` | `already_covered` |
| The shipped tutorial corpus compiles through the runtime and bytecode paths, while the blinker, traffic-light, and motor-starter fixtures retain their exact documented scan behavior | `docs/specs/22-developer-workflows.md`, Executable tutorial examples | `crates/trust-runtime/tests/tutorial_examples.rs::tutorial_examples_parse_typecheck_and_compile_to_bytecode`; `tutorial_blinker_ton_timing_behavior`; `tutorial_traffic_light_state_sequence`; `tutorial_motor_starter_latch_and_unlatch` | `already_covered` |
| Deployment summaries fail closed, describe the successful deployment rather than acting as pointers, and leave the `current` symlink as the authoritative active bundle across rollback | `docs/specs/11-runtime-engine.md`, Versioned deployment and rollback | `crates/trust-runtime/tests/deploy_cli_command.rs::deployment_summary_reports_current_bundle_and_any_runtime_file_change`; current/previous, rollback, containment, and failure-atomic tests in the same native suite | `already_covered` |
| OSCAT compatibility adaptations preserve explicit numeric typing, classic/OOP behavior, and the reviewed paired runnable example layout | `docs/specs/31-oscat-library-profile.md`, Compatibility adaptations, component parity, and shipped OOP example projects | OSCAT native ST fixtures; `crates/trust-runtime/tests/oscat_oop_examples.rs::oscat_oop_example_st_unit_tests_pass`; `oscat_airport_baggage_namespace_aggregate_trigger_passes`; grouped-layout/pattern assertions | `already_covered` |
| Textual `ACTION` bodies do not terminate or swallow surrounding owner statements | `docs/specs/04-pou-declarations.md`, Action declarations | `crates/trust-syntax/tests/parser_action_contract.rs::action_parser_accepts_owner_statements_around_analyzed_actions` | `already_covered` |
| Action bodies share the owning POU variable/receiver scope and retain independent diagnostics | `docs/specs/09-semantic-rules.md`, Action semantic context | `crates/trust-hir/tests/semantic_actions.rs::action_semantics_share_program_variable_scope` | `already_covered` |
| Temporal arithmetic accepts and rejects exactly the documented operand/result matrix | `docs/specs/05-expressions.md`, Temporal arithmetic | `crates/trust-hir/tests/semantic_type_checking/expression_operator_contract_acceptance.rs::expression_operator_accepts_complete_temporal_arithmetic_matrix`; rejection counterpart in `expression_operator_contract_rejection.rs` | `already_covered` |
| Explicit mixed numeric operations require an accuracy-preserving common type | `docs/specs/05-expressions.md`, Mixed numeric accuracy | `crates/trust-hir/tests/semantic_type_checking/expression_operator_contract_rejection.rs::expression_operator_rejects_ulint_real_without_common_type` | `already_covered` |
| Unique integer-valued enumeration constants are accepted as array bounds | `docs/specs/02-data-types.md`, Array bounds | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs::test_array_bounds_enum_values` | `already_covered` |
| Computed `ANY_INT` and subrange indexes are runtime-checked rather than rejected from their declared domain alone | `docs/specs/09-semantic-rules.md`, Array indexing | `crates/trust-hir/tests/semantic_type_checking/assignments_and_var_access.rs::test_array_index_subrange_out_of_bounds` | `already_covered` |
| `CURRENT_DT()` uses UTC Unix milliseconds, preserves the nonnegative `i64` range, and rejects invalid host time | `docs/specs/07-standard-functions.md`, Current date/time | `crates/trust-runtime/src/stdlib/time.rs::current_dt_preserves_the_full_nonnegative_dt_tick_range`; `current_dt_rejects_pre_epoch_and_out_of_range_host_time` | `already_covered` |
| Portable civil-date and `DateTimeProfile` conversion preserve checked Gregorian boundaries, valid day-resolution divisibility/range, and signed tick division modes | `docs/specs/10-runtime-semantics.md`, Portable Runtime Value and Program-Model Contracts | datetime conversion and profile tests in `crates/trust-runtime-core/src/datetime.rs` | `already_covered` |
| Bytecode `FOR` comparisons materialize the evaluated step using the control variable's integer type | `docs/specs/10-runtime-semantics.md`, `FOR` execution | `crates/trust-runtime/tests/bounded_value_semantics.rs::for_loop_bounds_materialize_the_control_variable_type` | `already_covered` |
| File-backed `PROGRAM VAR RETAIN` uses qualified keys; warm restart reloads saved state and cold restart restores initialization | `docs/specs/11-runtime-engine.md`, Retain snapshot identity and order | `crates/trust-runtime/src/runtime/retain_snapshot.rs::program_retain_key_round_trips_qualified_program_path`; `crates/trust-runtime/tests/runtime_reliability.rs::e2e_retain_roundtrip_restart` | `already_covered` |
| Indexed string writes enforce destination `CHAR`/`WCHAR` width and reject invalid scalar or shape | `docs/specs/10-runtime-semantics.md`, Indexed string access | `crates/trust-runtime-core/src/value/string_semantics.rs::narrow_string_index_rejects_out_of_range_chars`; `wide_string_index_reads_and_writes_unicode_scalar_elements` | `already_covered` |
| Direct addresses accept only exact wildcards and unsigned ASCII-decimal components | `docs/specs/10-runtime-semantics.md`, Direct-address grammar | `crates/trust-runtime/tests/io_address.rs`, malformed wildcard and nondecimal/out-of-range cases | `already_covered` |
| DAP `stackTrace` rejects an unknown projected thread without synthesizing a frame | `docs/specs/13-debug-adapter.md`, Stack trace | `crates/trust-debug/src/adapter/tests_part_05.rs::stack_trace_rejects_unknown_thread_id` | `already_covered` |
| At zero velocity, active `MC_Stop` reports `Done`, not `Busy`, and holds `Stopping` | `docs/internal/references/PLCopenMotion/plcopen_motion_library_spec_for_truST_v0_1.md`, single-axis stop behavior | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st::plcopen_motion_single_axis_home_halt_stop_and_continuous_end_velocity_behaviors` | `already_covered` |
| PLCopen standard parameters are writable only during disabled initialization, while vendor BOOL parameter 1000 remains independent | `docs/internal/references/PLCopenMotion/plcopen_motion_library_spec_for_truST_v0_1.md`, Parameter model | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st`, parameter constants, BOOL roundtrip, and rejection test programs | `already_covered` |
| Buffered PLCopen commands flushed by `ErrorStop` retain the axis fault and report `Error`, not `CommandAborted` | `docs/internal/references/PLCopenMotion/plcopen_motion_library_spec_for_truST_v0_1.md`, Queue cleanup and ErrorStop | `crates/trust-runtime/tests/fixtures/plcopen_motion/single_axis_core/src/tests.st::plcopen_motion_single_axis_buffered_commands_report_error_after_active_fault` | `already_covered` |
| A PLCopen POU containing both ST and non-ST executable bodies fails closed | `docs/specs/22-developer-workflows.md`, PLCopen semantic import | `crates/trust-plcopen/src/plcopen/tests/semantic_import_contract.rs::semantic_supported_st_plus_non_st_body_fails_closed` | `already_covered` |
| One malformed structured PLCopen GVL entry rejects the entire list | `docs/specs/22-developer-workflows.md`, PLCopen global-variable import | `crates/trust-plcopen/src/plcopen/tests/global_var_contract.rs::global_vars_one_malformed_structured_entry_rejects_whole_list` | `already_covered` |
| PLCopen export rejects parser errors before publication and rolls back atomically when source-map publication fails | `docs/specs/22-developer-workflows.md`, PLCopen export transaction | `crates/trust-plcopen/src/plcopen/tests/semantic_export_contract.rs::semantic_export_rejects_parser_errors_before_publication`; rollback contract in `transaction_contract.rs` | `already_covered` |
| Malformed lexical candidates remain one rejected candidate instead of splitting into misleading valid tokens | `docs/specs/01-lexical-elements.md`, Lexical rejection and value-validation boundary | `crates/trust-syntax/tests/lexer_lexical_contract.rs`; `parser_lexical_contract.rs` | `already_covered` |
| User-defined type formation is fail-closed and transactional while valid sibling declarations remain discoverable | `docs/specs/02-data-types.md`, User-defined types; `docs/specs/09-semantic-rules.md`, Type formation | `crates/trust-hir/tests/semantic_type_checking/user_type_contract_acceptance.rs`; `user_type_contract_rejection.rs`; parser user-type contracts | `already_covered` |
| Runtime user-defined-type materialization preserves enum and named-value identity/defaults, subrange and array failure atomicity, and recursive struct/union defaults and copies | `docs/specs/10-runtime-semantics.md`, Initializer and reference architecture; portable default rules | `crates/trust-runtime/tests/user_type_runtime_contract.rs` | `already_covered` |
| Call binding preserves positional/formal order, directions, defaults, EN/ENO, writable actuals, and alias/overlap rejection | `docs/specs/04-pou-declarations.md`, Calls; `docs/specs/09-semantic-rules.md`, Call binding | `call_binding_contract_acceptance.rs`; `call_binding_contract_rejection.rs`; parser call-binding contracts | `already_covered` |
| Variable-section ownership and storage qualifiers form a closed legality matrix and reject duplicate/conflicting qualifiers | `docs/specs/03-variables.md`, Variable sections and qualifiers; `docs/specs/04-pou-declarations.md` | `variable_section_qualifier_acceptance.rs`; `variable_section_qualifier_rejection.rs` | `already_covered` |
| `R_EDGE`/`F_EDGE` declarations are Boolean FB/program inputs with one suffix, no initializer, and independent hidden state | `docs/specs/03-variables.md`, Edge detection qualifiers | `parser_edge_declarations.rs`; `semantic_type_checking/edge_declaration_contract.rs` | `already_covered` |
| Member access preserves visibility, inheritance, namespace boundaries, and directional FB-member write restrictions | `docs/specs/03-variables.md`, Member access; `docs/specs/09-semantic-rules.md`, Member resolution | `member_access_acceptance.rs`; `member_access_declaration_rejection.rs`; `member_access_use_rejection.rs` | `already_covered` |
| Source `ANY*` categories remain built-in-formal-only and cannot become user storage, results, aggregate fields, pointers, or references | `docs/specs/02-data-types.md`, Source declaration boundary | `parser_generic_type_contract.rs`; `semantic_type_checking/generic_type_contract.rs` | `already_covered` |
| `VAR_ACCESS` and `VAR_CONFIG` enforce direction, target eligibility/type, ambiguity, project-merged instance resolution, and wildcard completion | `docs/specs/03-variables.md`, Special variables and incomplete addresses | `semantic_type_checking/assignments_and_var_access.rs`, VAR_ACCESS/VAR_CONFIG contract cases | `already_covered` |
| Reference and pointer formation, `REF`/`ADR`, lifetime, dereference, NULL, compatibility, and write-through follow the documented closed contract | `docs/specs/02-data-types.md`, References and pointers; `docs/specs/05-expressions.md`, Reference expressions | `reference_contract_acceptance.rs`; `reference_contract_rejection.rs`; parser reference contracts | `already_covered` |
| `ANY_BIT` partial selectors preserve exact widths, bounds, aliases, nested/dereference bases, typed writes, and read-only rejection | `docs/specs/02-data-types.md`, Partial access | `partial_access_acceptance.rs`; `partial_access_rejection.rs`; parser partial-access contracts | `already_covered` |
| `STRING`/`WSTRING` character indexing uses one-based capacity bounds, integer one-dimensional indexes, and exact `CHAR`/`WCHAR` width | `docs/specs/02-data-types.md`, Character access | `semantic_type_checking/string_index_contract.rs`; parser string-index contracts | `already_covered` |
| Control-flow statements enforce the documented acceptance/rejection matrix, CASE label rules, integer FOR rules, and definite function return | `docs/specs/06-statements.md`; `docs/specs/09-semantic-rules.md`, Control flow | `control_flow_contract_acceptance.rs`; `control_flow_contract_rejection.rs`; parser control-flow contracts | `already_covered` |
| Configuration/resource/task/program-instance declarations enforce field shapes, priority and schedule bounds, uniqueness, scope, and task binding | `docs/specs/18-configurations-resources-tasks.md` | `crates/trust-hir/src/db/diagnostics/configuration/contract_tests.rs` | `already_covered` |
| HIR warning projection preserves the documented unused, complexity, unreachable, nondeterminism, and shared-global ownership policies | `docs/specs/28-hir-warning-policy.md` | warning-specific contract modules under `crates/trust-hir/src/db/diagnostics/**/contract_tests.rs` | `already_covered` |
| Namespace/type/value/call resolution honors scoped namespaces and USING ambiguity, and reports wrong-kind without undefined fallback | `docs/specs/03-variables.md`, Scope; `docs/specs/04-pou-declarations.md`, Namespace/USING; `docs/specs/09-semantic-rules.md`, Ambiguity | `crates/trust-hir/tests/namespaces.rs`; `semantic_type_checking/wrong_kind_resolution.rs` | `already_covered` |
| `VAR_EXTERNAL` links case-insensitively to its global and enforces type, CONSTANT, initializer, and cross-file program/configuration ownership | `docs/specs/03-variables.md`, `VAR_EXTERNAL` | `semantic_type_checking/assignments_and_var_access.rs`, VAR_EXTERNAL contract cases | `already_covered` |
| Semantic recovery reports one primary diagnostic for unresolved, ambiguous, or wrong-kind operands/targets without dependent cascades | `docs/specs/09-semantic-rules.md`, Diagnostic ownership and cascade suppression | `phase18_behavior_closure.rs`; `wrong_kind_resolution.rs`; `hir_mutation_hardening.rs` | `already_covered` |
| The non-temporal operator matrix rejects real MOD, BOOL/bit-string mixing, aggregate/instance comparison, reference arithmetic, and unrepresentable contextual literals | `docs/specs/05-expressions.md`, Operators; `docs/specs/09-semantic-rules.md`, Expression typing | `expression_operator_contract_acceptance.rs`; `expression_operator_contract_rejection.rs` | `already_covered` |
| PLCopen semantic import preserves supported POU/type/interface/body/CODESYS-method identity and fails closed on malformed metadata | `docs/specs/22-developer-workflows.md`, PLCopen semantic import | `crates/trust-plcopen/src/plcopen/tests/semantic_import_contract.rs` | `already_covered` |
| PLCopen configuration/resource/task/program-instance translation preserves deterministic identity/order/scope/defaults and rejects invalid references | `docs/specs/22-developer-workflows.md`, PLCopen project-model translation | `crates/trust-plcopen/src/plcopen/tests/project_model_contract.rs` | `already_covered` |
| PLCopen GVL translation preserves plaintext precedence, structured declarations, qualified-only modes, strict adapter injection, and whole-list rejection | `docs/specs/22-developer-workflows.md`, PLCopen global-variable translation | `crates/trust-plcopen/src/plcopen/tests/global_var_contract.rs` | `already_covered` |
| PLCopen semantic export preserves complete POU/type/GVL/config/method projection, stable order/maps/counts/escaping/roundtrip, and blocks duplicate authority | `docs/specs/22-developer-workflows.md`, PLCopen semantic export | `crates/trust-plcopen/src/plcopen/tests/semantic_export_contract.rs` | `already_covered` |
| PLCopen import/export filesystem identity is recursive, contained, collision-safe, symlink-refusing, deterministic, and failure-atomic | `docs/specs/22-developer-workflows.md`, PLCopen filesystem transaction and identity | `crates/trust-plcopen/src/plcopen/tests/transaction_contract.rs` | `already_covered` |
| LSP configuration normalization preserves defaults, profiles, paths, and deterministic canonical-vs-alias severity precedence | `docs/specs/14-lsp.md`, Configuration normalization and bounded values | `crates/trust-lsp/src/config/configuration_contract_tests.rs` | `already_covered` |
| LSP dependency parsing and resolution fail closed across trust policy, transitive cycles, allowlists, and lock integrity | `docs/specs/14-lsp.md`, Dependency graph, trust, and lock integrity | `crates/trust-lsp/src/config/dependency_contract_tests.rs` | `already_covered` |
| Runtime inline values preserve endpoint precedence, scope filters, atomic snapshots, and instance merge semantics | `docs/specs/14-lsp.md`, Runtime values | `crates/trust-lsp/src/handlers/runtime_values/contract_tests.rs` | `already_covered` |
| Persistent-index lexical path aliases share one cache identity | `docs/specs/14-lsp.md`, Persisted and external-input integrity | `crates/trust-lsp/src/index_cache/contract_tests.rs` | `already_covered` |
| Library Markdown documentation preserves ATX heading and fenced-body syntax | `docs/specs/14-lsp.md`, Persisted and external-input integrity | `crates/trust-lsp/src/library_docs/contract_tests.rs` | `already_covered` |
| Library graph identities and diagnostics are case-insensitive, deduplicated, cycle-aware, and deterministically ordered | `docs/specs/14-lsp.md`, Library graph integrity | `crates/trust-lsp/src/library_graph/contract_tests.rs` | `already_covered` |
| Updates for unknown or closed documents are complete server-state no-ops | `docs/specs/14-lsp.md`, Server-state lifecycle and cache | `crates/trust-lsp/src/state/state_contract_tests.rs` | `already_covered` |
| Agent JSON-RPC requests preserve correlation and confine workspace path/symlink access | `docs/specs/20-agent-api-v1.md`, Request envelope and filesystem; `docs/guides/AGENT_CONTRACT_V1.md` | `crates/trust-dev/src/agent/contract_tests.rs` | `already_covered` |
| Agent harness execution validates selectors/bounds, isolates transactions, and preserves assertion/failure accounting and order | `docs/specs/20-agent-api-v1.md`, Harness execution | `crates/trust-dev/src/agent/harness_contract_tests.rs` | `already_covered` |
| API documentation discovery and extraction preserve source/tag authority | `docs/specs/22-developer-workflows.md`, API documentation generation | `crates/trust-dev/src/docs/tests/discovery_contract.rs` | `already_covered` |
| API documentation publication fails before write, escapes content, and publishes the selected artifact set deterministically and atomically | `docs/specs/22-developer-workflows.md`, API documentation generation | `crates/trust-dev/src/docs/tests/publication_contract.rs` | `already_covered` |
| Selected ST tests apply compiled bytecode, execute TEST_PROGRAM/TEST_FUNCTION_BLOCK bodies, and preserve assertion-vs-runtime-error classification | `docs/specs/22-developer-workflows.md`, Test case execution | `crates/trust-dev/src/test_cmd/tests.rs`; `crates/trust-runtime/tests/st_test_cli_command.rs` | `already_covered` |
| ADS/OPC UA external nodes use direction-aware counterpart labels instead of repeating the local role | `docs/specs/25-vscode-product-contract.md`, Link and counterpart truth | three counterpart-label cases in `editors/vscode/src/test/suite/network-canvas.test.ts` | `already_covered` |
| Release guards require an annotated exact-SHA tag, matching main/tag workflow runs, and unique published asset names | `docs/specs/24-release-evidence.md`, Main-push guard and tag-triggered preflight | `scripts/release_version_preflight_contract_tests.py`; `scripts/check_version_release_evidence_tests.py` | `already_covered` |
| Telemetry failed-flush and sink-reconfiguration paths retain aggregates for retry | `docs/specs/14-lsp.md`, Telemetry integrity | `crates/trust-lsp/src/telemetry/contract_tests.rs`, failed-flush/reconfiguration/disable cases | `already_covered` |
| Telemetry duration conversion clamps millisecond overflow instead of wrapping | `docs/specs/14-lsp.md`, Telemetry integrity | `crates/trust-lsp/src/telemetry/contract_tests.rs::duration_conversion_clamps_instead_of_wrapping` | `behavior_defect` resolved test-first; remote expected red observed wrapped value `384` instead of `u64::MAX`, then the same focused test passed after the checked conversion |
| Execution-mode `trust-dev test` compiles a supported ST project even when discovery finds zero TEST POUs, while list/zero-filter modes remain discovery-only | `docs/specs/22-developer-workflows.md`, Test case execution | `crates/trust-dev/tests/cli_smoke.rs`, invalid ordinary-`PROGRAM` multi-file CLI case | `already_covered`; independent challenge found the existing native integration already asserts nonzero and both diagnostics with no TEST POU |
| External-diagnostic severity strings trim surrounding whitespace and parse case-insensitively | `docs/specs/14-lsp.md`, Persisted and external-input integrity | `crates/trust-lsp/src/external_diagnostics/contract_tests.rs::string_severity_vocabulary_is_trimmed_and_case_insensitive` | `missing_spec` resolved; the owning specification and public configuration guide now name the shipped trimming behavior already asserted by the native test |
| Successful GitHub API responses with malformed JSON or a non-object top level produce a stable endpoint/status-bearing release-preflight failure without a traceback | `docs/specs/24-release-evidence.md`, GitHub API failure projection | `scripts/release_version_preflight_contract_tests.py::TagPreflightGitHubApiBoundaryTests`; `scripts/check_version_release_evidence_tests.py::GitHubApiBoundaryTests` and `VersionReleaseGuardContractTests` | `behavior_defect` resolved test-first; the contract now requires a top-level object and stable endpoint/status error, the expected malformed/list assertion reds were captured, and the shared decoder plus both CLI projections passed the focused tests and all 53 owning release tests on `trust-builder` |
| DAP initialize/launch/attach/configurationDone/terminate ordering is explicit and fail-closed | `docs/specs/13-debug-adapter.md`, Session lifecycle and request ordering | `crates/trust-debug/src/adapter/lifecycle_contract_tests.rs` | `already_covered` |
| DAP JSON uses canonical keys, required fields, lowercase discriminators, and fail-closed malformed payloads | `docs/specs/13-debug-adapter.md`, DAP transport and JSON wire schema | `crates/trust-debug/src/protocol/wire_schema_tests.rs` | `already_covered` |
| Duplicate breakpoint suppression is exact-identity and generation scoped within one scan | `docs/specs/13-debug-adapter.md`, Breakpoint lifecycle | breakpoint identity and cycle-generation tests under `crates/trust-runtime/src/host/debug/breakpoints*` | `already_covered` |
| Managed fleet add/auth/status/start/stop lifecycle validates manifests and fails closed without leaking child processes; every successful socket or HTTP shutdown writes its complete correlated acknowledgement before applying the deferred resource stop | `docs/specs/11-runtime-engine.md`, Managed local fleet scaffolding and shutdown response ordering | `crates/trust-runtime/tests/fleet_lifecycle_command.rs`; `crates/trust-runtime/src/control/tests/control_request_boundary_contract.rs`; `crates/trust-runtime/tests/web_io_config_integration/web_io_config_integration_part_22.rs::http_shutdown_returns_complete_correlated_acknowledgement`; inline lifecycle tests | `behavior_defect` resolved test-first; the focused socket and web-control ordering assertions failed before transport-owned completion, then passed with the shipped fleet lifecycle test after TCP/Unix and HTTP response boundaries applied the stop only after their write attempt |
| Control CLI requests preserve exact envelopes, scalar values, timeout handling, endpoint/token precedence, and nonzero failure on malformed success | `docs/specs/11-runtime-engine.md`, Scriptable control client | `crates/trust-runtime/tests/control_cli_command.rs`; inline control-exchange tests | `already_covered` |
| Modbus scan deadlines preserve `fault`/`warn`/`ignore`, queued output intent, and complete input images; declared-width scaling rejects nonfinite narrowing | `docs/specs/11-runtime-engine.md`, Modbus deadline and floating-point integrity | Modbus worker unit tests; `crates/trust-runtime/tests/modbus_driver.rs` | `already_covered` |
| MQTT raw reads select the newest drained payload and default-fault policy preserves stale-input, output-deadline, and typed-output failures | `docs/specs/11-runtime-engine.md`, MQTT default-fault and latest-value integrity | `crates/trust-runtime/src/io/mqtt/tests.rs` | `already_covered` |
| OPC UA client configuration, trust, one-shot cardinality, session generation, queued writes, finite scalar outputs, and shutdown authority fail closed | `docs/specs/11-runtime-engine.md`, OPC UA client configuration/trust/worker integrity | `crates/trust-runtime/src/host/opcua/tests/client_contracts.rs`; `worker_integrity.rs` | `already_covered` |
| Runtime configuration parsing rejects unknown, out-of-range, blank, insecure, or internally inconsistent runtime/protocol settings before startup | `docs/specs/11-runtime-engine.md`, Runtime configuration loading and validation | `crates/trust-runtime/src/config/tests.rs`; `crates/trust-runtime/tests/config_schema_command.rs` | `already_covered` |
| ADS server publication and lifecycle preserve canonical bounded symbols, exact client policy, security-first write rejection, and audit context | `docs/specs/11-runtime-engine.md`, ADS server publication, lifecycle, client policy, write-back, and audit | ADS server publication/lifecycle and policy/write/audit native suites | `already_covered` |
| ADS client configuration, transport, session, cache, subscription, and write-generation authority reject malformed or stale correlation without losing newer intent | `docs/specs/11-runtime-engine.md`, ADS client configuration and transport boundary | ADS contracts/backend tests; ADS client worker write-generation suites | `already_covered` |
| ADS onboarding identity, endpoint discovery, Doctor live proof, guarded-write restoration, route artifacts, and credential handling fail closed | `docs/specs/11-runtime-engine.md`, ADS onboarding contracts | ADS onboarding identity/discovery/route tests; Doctor and wire native suites | `already_covered` |
| ADS symbol import selection is exact; generated artifacts are confined and regeneration/apply is atomic and idempotent | `docs/specs/11-runtime-engine.md`, ADS symbol import selection, artifact safety, and apply | ADS import handler and onboarding import native suites | `already_covered` |
| ADS diagnostic production readiness requires complete, current, role-correct evidence and fails closed on empty or stale reports | `docs/specs/11-runtime-engine.md`, ADS diagnostic report and readiness truth | `crates/trust-runtime/src/host/ads/diagnostics/tests/readiness_tests.rs` | `already_covered` |
| Runtime Cloud action/profile and control/I/O proxy validation reject empty/duplicate targets and malformed routing/correlation/API versions | `docs/specs/11-runtime-engine.md`, Runtime Cloud action preflight and profile contract | runtime-cloud policy unit tests; web integration parts 14 and 18 | `already_covered` |
| Runtime Cloud desired revisions record only the immutable snapshot actually applied and keep superseding revisions pending across success or failure | `docs/specs/11-runtime-engine.md`, Desired-configuration reconciliation | `runtime_cloud/config_policy.rs`; web integration conflict/retry cases | `already_covered` |
| Runtime Cloud schema evolution and canonical keyspace reject blank/duplicate fields, overlap, overflow, insertion-before-tail, and reserved collisions | `docs/specs/11-runtime-engine.md`, Contract schema and canonical keyspace | `runtime_cloud/contracts.rs`; `keyspace.rs` native tests | `already_covered` |
| Runtime Cloud topology reports only measured latency/loss and scopes same-host/T0 classification to the actual peer group | `docs/specs/11-runtime-engine.md`, Topology measurement and same-host contract | runtime-cloud projection/link-policy and web integration tests | `already_covered` |
| Runtime Cloud HA lease, split-brain, replay, and duplicate dispatch decisions use the complete supplied group and prevent dual output | `docs/specs/11-runtime-engine.md`, HA lease, split-brain, and replay contract | `runtime_cloud/ha/tests.rs`; web integration part 19 | `already_covered` |
| Rejected online changes are failure-atomic across bytecode, variable/instance image, retain migration, scan/fault/telemetry state, and debugger mutations | `docs/specs/11-runtime-engine.md`, Online-change transaction | `crates/trust-runtime/tests/hot_reload.rs` | `already_covered` |
| Named bytecode resource application preserves declared identity, rejects unknown names, and retains only the single-resource legacy-placeholder compatibility path | `docs/specs/11-runtime-engine.md`, Resource selection; `docs/specs/12-bytecode.md` | `crates/trust-runtime/tests/process_image.rs`; bytecode encoder tests | `already_covered` |
| Runtime control jobs, state, events, faults, historian, metrics, and operation registry expose closed shapes, ordering/limits, health dominance, and fail-closed unavailable state | `docs/specs/11-runtime-engine.md`, Runtime control request, job, and state surfaces | status/event/historian and operation-registry contract suites | `already_covered` |
| Runtime config get/set preserves normalization, atomic rollback, startup-only restrictions, cross-field consistency, and secret redaction | `docs/specs/11-runtime-engine.md`, Runtime control configuration and authorization | config handler and validation contract suites | `already_covered` |
| Runtime I/O and variable mutation queues preserve exact grammar/bounds/order, replace-in-place force semantics, idempotent release, and existing state on rejection | `docs/specs/11-runtime-engine.md`, Runtime control I/O and variable mutation | I/O-handler and variable-handler contract suites | `already_covered` |
| Communication capability/schema/Test expose closed vocabularies, conservative multi-instance health, bounded target parsing, exact evidence, and recursive credential removal | `docs/specs/11-runtime-engine.md`, Communication schema, capabilities, and Test | capability/schema/discover/probe/browse contract suites | `already_covered` |
| Communication authoring is project-confined, secret-gated, production-validated, idempotent, dry-run safe, and transactionally failure-atomic | `docs/specs/11-runtime-engine.md`, Communication apply mutation and security | `control/comm_handlers/apply/runtime_file/contract_tests.rs` | `already_covered` |
| Offline communication and fleet topology never invent live evidence and preserve deterministic schema-v4 IDs/order, sidecar confinement, policy health, and credential absence | `docs/specs/11-runtime-engine.md`, Offline communication and fleet-topology truth | fleet offline/topology/host/protocol contract suites | `already_covered` |
| Host boundary errors, protocol envelopes, and endpoint/project resolution fail closed with exact source precedence and containment | `docs/specs/11-runtime-engine.md`, Runtime control boundary and launcher resolution | host-boundary contract suites; control endpoint/request boundary suites | `already_covered` |
| Harness compilation, session loading, coercion, execution, and JSON-line protocol preserve complete source sets, exact labels/errors, closed bounds/shapes, and fail-closed malformed input | `docs/specs/10-runtime-semantics.md`, Test harness and automation protocol | harness API/coercion/protocol contract suites | `already_covered` |
| Package registry storage authenticates first, confines identity, preserves immutable duplicate content, verifies complete sorted digests, and rejects tamper/symlink/partial publication | `docs/specs/22-developer-workflows.md`, Registry initialization and package storage integrity | `host/registry/registry_integrity_contract_tests.rs` | `already_covered` |
| Control credentials, pairing, and TLS materials preserve closed roles, exact secrets, expiry/pruning, one-time claim, atomic persistence, and no debug leakage | `docs/specs/11-runtime-engine.md`, Control credentials, pairing, and TLS | host-security and pairing contract suites | `already_covered` |
| Runtime simulation configuration and execution preserve closed typed/finite/time bounds, deterministic due ordering, disabled no-op behavior, and I/O/fault projection | `docs/specs/11-runtime-engine.md`, Simulation coupling, model execution, and conversion boundaries | `crates/trust-runtime/src/host/simulation_contract_tests.rs` | `already_covered` |
| Runtime metrics preserve finite empty identity, saturating counters, the exact rolling window/percentile rule, stable ordering, and profile-toggle state | `docs/specs/11-runtime-engine.md`, Runtime metrics window and profiling | `crates/trust-runtime/src/host/metrics_contract_tests.rs` | `already_covered` |
| OPC UA server loopback wire, cold-start, user authentication, and certificate trust enforce the documented public server boundary | `docs/specs/11-runtime-engine.md`, OPC UA server surface, security, and cold start | `crates/trust-runtime/tests/opcua_integration.rs` | `already_covered` |
| Runtime CLI dispatch and launcher validation reject malformed projects/options/config before mutation or server start and preserve backend/bundle/retain precedence | `docs/specs/11-runtime-engine.md`, Runtime CLI entrypoint and launcher | `crates/trust-runtime/tests/runtime_cli_entrypoint.rs`; run-module tests | `already_covered` |
| Guided setup preserves literal/dependency source discovery, coherent generated artifacts, safe defaults, dry-run no-op behavior, token randomness/expiry, and mode-specific option boundaries | `docs/specs/22-developer-workflows.md`, Guided setup | `crates/trust-runtime/tests/setup_command.rs` | `already_covered` |
| Cross-file enum values publish into their enclosing scope without turning same-leaf variants from distinct enum types into duplicate declarations or arbitrary resolution | `docs/specs/10-runtime-semantics.md`, User-defined type initialization; `docs/specs/26-hir-semantic-kernel.md`, imported symbol identity | `crates/trust-hir/src/db/queries/database/database_tests_part_01.rs`; `crates/trust-runtime/src/host/harness/compiler/project_assembly_contract_tests.rs::project_assembly_contract_resolves_later_enum_in_program_initializer` | `behavior_defect` resolved; two focused HIR importer tests and the exact end-to-end runtime initializer test passed remotely after a clean package rebuild |
| Lexer-rejected malformed identifier candidates retain the specific `E106` editor diagnostic rather than degrading to a generic parser code | `docs/specs/01-lexical-elements.md`, lexical rejection boundary; `docs/specs/14-lsp.md`, lexer-owned diagnostics and explainers | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `behavior_defect` resolved; all three registered pull/push/cancellation behavior tests passed remotely |
| Function and method call-local defaults round-trip recursively through STBC for fixed/wildcard arrays, structs, unions, nested strings, and NULL references; malformed/deep payloads fail closed, and supplied interface or object-instance formals do not require a fabricated portable constant | `docs/specs/03-variables.md`, call-local initializer lifetime; `docs/specs/12-bytecode.md`, `CONST_POOL`, parameter defaults, and validator-before-apply | `crates/trust-runtime/src/host/harness/compiler/pou_initializer_lifetime_contract_tests.rs`; `crates/trust-runtime/src/bytecode/validate.rs`; `crates/trust-runtime-core/src/vm/mod.rs` | `behavior_defect` plus missing native coverage resolved test-first; focused reds observed NULL aggregate publication, unsupported union constants, wildcard count `2^63`, missing depth rejection, decoder depth 32, non-NULL reference acceptance, and supplied interface/function-block rejection; then validator 5/5, decoder 5/5, and lifetime 21/21 passed remotely |
| Explicit variable initializers are compile-time constant expressions across scalar and aggregate types, except for the IEC reference-declaration `NULL`/`REF(...)` form; legal array repetitions and typed aggregate constructors remain accepted; unresolved and ambiguous references/callees produce exactly one primary diagnostic without E202 cascade | `docs/specs/03-variables.md`, Initialization and POU initializer contracts | `crates/trust-hir/tests/semantic_type_checking/variable_initializer_constant_expression.rs`; `crates/trust-runtime/tests/struct_initializers.rs` | `behavior_defect` resolved test-first; remote focused HIR module 19/19, valid reference initializer, preserved function-local REF lifetime rejection, and mutable function-local initializer rejection are green; final complete semantic/runtime suites remain part of row 005 |
| Malformed/unreadable LSP configuration and normalized defaults produce stable user-facing C001-C005 diagnostics; every configured eviction percentage outside `1..=100`, including `-1` and `256`, produces precise C003 at the invalid key | `docs/specs/14-lsp.md`, Configuration normalization and bounded values | `crates/trust-lsp/src/config/configuration_contract_tests.rs`; `crates/trust-lsp/src/handlers/tests/core_part_03.rs` | `behavior_defect` resolved test-first; remote expected reds observed `-1` and `256` falling back to `80` and publishing whole-file `C001`, then the same three focused tests passed after widening only the private raw TOML field and retaining the public bounded `u8` model |
| Project source discovery accepts a contained `.st`/`.pou` symlink and rejects an alias whose resolved target is not Structured Text or escapes containment | `docs/guides/AGENT_CONTRACT_V1.md`, Source Discovery; `docs/specs/20-agent-api-v1.md`, containment | `crates/trust-dev/src/workflow/contract_tests.rs` | `missing_test` plus `behavior_defect` resolved; focused remote discovery suite 9/9 green |
| Managed runtime `starting`, `stopping`, and `unavailable` states remain distinct and non-actionable in sidebar/canvas models | `docs/specs/25-vscode-product-contract.md`, Runtime lifecycle and controls | `editors/vscode/src/test/suite/runtime-controls-contract.test.ts`; `network-canvas.test.ts` | `missing_test` resolved; remote compile and all five required-browser canvas fixtures passed, then a build-free focused launch of the same registered Extension Host suite passed all six selected managed-state/mesh tests. The repository's full `npm test` remains a final-candidate gate rather than focused proof |
| Proven degraded/error mesh links are solid and retain target-specific health/detail; unknown future status is dashed; managed unavailable renders fail-closed | `docs/specs/25-vscode-product-contract.md`, Status projection | `editors/vscode/src/test/suite/network-canvas.test.ts`; `editors/vscode/scripts/network-canvas-overlap-check.js` | `missing_test` plus `behavior_defect` resolved test-first; remote lint/compile and required-browser status fixture green |
| Degraded/error point-to-point links retain backend detail and expose distinct semantic tone plus readable hover/focus detail | `docs/specs/25-vscode-product-contract.md`, Link and counterpart truth | `editors/vscode/src/test/suite/network-canvas.test.ts`; `editors/vscode/scripts/network-canvas-overlap-check.js` | `behavior_defect` resolved test-first; remote rendered red at four assertions, then four layout fixtures and the rendered status contract green |
| Legacy fleet snapshots receive globally unique host/container/runtime-owned UI identities for runtime instances, endpoints, configured mesh externals, independently resolved link endpoints, and shared-system runtime references at singleton and merge ingress; repeated reports of one owner still deduplicate | `docs/specs/25-vscode-product-contract.md`, Graph projection, merge, and fallback | `editors/vscode/src/test/suite/network-canvas-fleet-identity.test.ts`; real Extension Dev Host/CDP capture `DC-fleet-identity-scoped.png` plus `fleet-identity-scoped-proof.json` | `behavior_defect` resolved test-first; the registered remote Extension Dev Host baseline was 1 passing and 5 expected behavior-assertion failures, then the same suite passed 6/6 after the pure owner-scoped normalizer and single merge ingress. The shipped webview capture passed 1/1 and visibly preserved two same-named `RESOURCE` runtimes, two configured peers, and distinct degraded/error link detail across Line A and Line B |
| E303/E304 diagnostic explainers link to their actual IEC array/range authority | `docs/specs/14-lsp.md`, Diagnostic explainer coverage | `crates/trust-lsp/src/handlers/tests/core_part_01.rs` | `behavior_defect` resolved test-first; remote focused red then 1/1 green |

The direct behavior census now covers the surviving product delta by coherent
observable contract group. Independent read-only lanes reviewed the language,
PLCopen, library/example, runtime-core, runtime, protocol, control, debug, LSP,
developer-tool, VS Code, release, and accepted-baseline deletion surfaces. The
confirmed specification/test gaps and behavior defects are resolved at their
focused boundaries; final exact-candidate gates remain. No metadata state may
add product work during closeout.

## Execution Rows

- [x] `VERIF-P18-SPEC-TEST-000` Recover the accepted source baseline and remove
  the per-function code index, blanket implementation gaps, and generated
  burn-down campaign without deleting valid product specifications or tests.
- [x] `VERIF-P18-SPEC-TEST-001` Review the surviving net product diff from
  `1f313452` to the frozen candidate, hunk by hunk. Record one observable
  behavior per ledger row with its exact specification section and native test.
- [x] `VERIF-P18-SPEC-TEST-002` Independently challenge every proposed
  `missing_spec`, `missing_test`, `behavior_defect`, or `external_manual` row.
  Remove any row created only by verification metadata.
- [x] `VERIF-P18-SPEC-TEST-003` Remove remaining campaign-only instructions,
  assignment tools, and active work queues while preserving product code,
  specifications, native tests, fixtures, and valid fixes.
- [x] `VERIF-P18-SPEC-TEST-004` Resolve confirmed gaps one behavior at a time:
  specify first, then add the smallest native test; change product code only
  after an honest focused assertion failure.
- [x] `VERIF-P18-SPEC-TEST-005` Run affected focused tests, freeze the
  candidate, run final remote `fmt`, `clippy`, and `test-all` once, obtain an
  independent final diff review, commit one clean checkpoint, and stop without
  pushing.
  - Current focused validation: public-doc link/IA checks, diagram render/drift,
    focused HIR/trust-dev/release and registered VS Code fleet/status tests, the
    shipped-webview fleet capture, bytecode validator 5/5, portable decoder 5/5,
    POU initializer lifetime 21/21, LSP E106 3/3, debug launch 4/4, and runtime
    policy/I/O 2/2 are green. The initializer regression follow-up passed the
    focused HIR module 19/19 plus both affected runtime tests after the valid
    IEC `REF(...)` declaration red and the retained function-local lifetime red.
    The restored PLCopen positive and negative fixture
    harnesses passed; VS Code registration reported 477 facts in 41 registered
    files and the full Extension Host suite passed 477/477 after lint, compile,
    and required-browser canvas checks; the four runtime verticals passed
    3/3, 20/20, 1/1, and 4/4; mesh/TLS stability passed 8/8; and the host plus
    Windows warning-deny gate passed after two test/control-flow portability
    cleanups. The register/stack enum `CASE` differential and closest register
    executor locks passed after the scoped enum-label lowering repair; the
    initializer architecture and primitive-coercion suites passed after the
    cfg(test)-only contract allowlist correction. A first complete broad run
    then reached the Web IDE integrations and exposed one stale MQTT probe
    fixture: the test supplied a malformed authority while expecting the
    resolved-but-refused result. The fixture now uses a reserved and released
    loopback port, and that exact remote test is green. The next broad run
    exposed an older lexer token-table/test oracle that split malformed `1.`
    despite the newer fail-closed numeric-candidate contract and implementation;
    the table and native expectation now agree on one `Error` token. Because
    both broad runs stopped at stale assertions, neither is final-candidate
    proof. The following rerun reached release-evidence traces and exposed two
    more retired invariant-mapping phrases plus current high-severity
    `brace-expansion` and `js-yaml` advisories. The claim trace now checks the
    direct written-specification/native-test authority, and the VS Code lockfile
    uses patched compatible releases. The next complete rerun reached the
    managed-fleet lifecycle and exposed an intermittent shutdown response race:
    the resource stop could terminate the process before the detached control
    connection wrote its acknowledgement. The focused socket ordering
    assertion was red before the fix, then green after shutdown became a
    transport-completed action; the exact shipped-CLI lifecycle test is also
    green. Independent final review then found the HTTP `/api/control` path
    still completed the same action before serializing its response. The new
    web-control ordering test failed at that exact assertion, then passed after
    the HTTP and local Runtime Cloud proxy/action response boundaries adopted
    the same executable prepare-write-complete ownership. The native HTTP
    integration also received the complete correlated acknowledgement, and the
    Runtime Cloud architecture lock requires its routes to use that ownership.
    Before this HTTP follow-up changed the candidate, the socket-corrected tree
    had passed the complete remote `just test-all` gate, including
    3,761/3,761 runtime library tests, every runtime integration suite, the
    OpenOT and OSCAT gates, PLCopen positive/negative fixtures, language/LSP
    suites, verification-case runners, and documentation tests. Hardware,
    performance, and manual-only tests remained explicitly ignored and are not
    claimed as automated proof. A later cold exact-candidate `test-all` attempt
    stopped on storage exhaustion and is neither product-red evidence nor green
    proof. After safe generated-cache cleanup, an earlier HTTP-corrected staged
    tree (`ec017bcfe309fbf75331946f59c475a7c5571213`) completed a fresh cold
    remote `just test-all`: 8,136 passed, zero failed, and 24 intentionally
    ignored across 279 result groups. The validation checkout remained byte/tree
    identical to the staged candidate before and after the run.
  - On that earlier HTTP-corrected exact candidate, remote `just fmt`, `just
    clippy`, and `just test` are green (`just test`: 3,762/3,762). The four
    runtime verticals passed 3/3, 20/20, 1/1, and 4/4; mesh/TLS stability passed
    8/8; and `RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets`
    is green. The strict verification report exited zero for the complete 831
    path baseline delta while retaining all historical maintenance findings as
    advisory. Refreshed MP-001 discovery/snapshot parity passes with 119 LSP
    and 866 HIR cases. Fresh independent review found no product/spec/test,
    deletion, board-truth, release/version, or index-integrity blocker after
    all 831 paths were staged with zero unstaged or untracked files. That review
    left the single local checkpoint commit and exact-SHA preparation pending.
  - The one-commit candidate passed its remote VS Code, `fmt`, and Clippy
    stages twice, but both exact-SHA preparation attempts stopped at
    `just test-all` after accumulated VS Code/Clippy artifacts and unrelated
    concurrent builder work exhausted the shared filesystem. Those attempts
    are neither product-red evidence nor final green proof, and no passing
    exact-SHA artifact has yet been recorded.
  - The release-candidate gate was corrected test-first at this boundary:
    pull requests and exact candidates use a bounded report smoke while the
    exhaustive recursive verification-tooling suite remains scheduled/manual;
    candidate Rust commands disable incremental duplication; and the guard
    validates then reclaims only its task-owned generated target between
    Clippy and the cold `test-all` run. Final exact-candidate proof, independent
    review, and checkpoint amendment are bound to the passing exact-SHA
    artifact for this one-commit candidate. If that artifact is absent or red,
    this row is open regardless of the checked box. No push is part of this
    board closeout.

## Stop Gates

- Do not infer product work from an invariant, catalog, evidence, mutation,
  scanner, report, or denominator state.
- Do not require one broad aggregate test when precise native tests already
  assert the constituent behavior.
- Do not invent expected behavior from implementation or another truST engine.
- Do not alter product behavior before the focused native test establishes the
  expected assertion result.
- Do not delete a product specification, native test, required fixture, or
  valid product fix as campaign cleanup.
- Do not replace a written hardware, browser, or interoperability requirement
  with a mock or metadata claim.
- Do not run broad remote gates until the candidate is frozen and the builder
  satisfies the documented disk threshold.

## SOLID/KISS/DRY Acceptance

- One observable behavior produces at most one audit row.
- Product authority lives in the owning specification.
- Product verification lives in native executable assertions.
- Historical metadata may locate material but cannot create requirements.
- No parallel code-index, mapping proposal, proof-promotion, or mutation
  campaign is part of completion.
