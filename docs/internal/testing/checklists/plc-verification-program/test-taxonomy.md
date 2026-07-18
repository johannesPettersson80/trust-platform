# Test Taxonomy

This document owns the testing matrix: which code changes require which proof,
what each test class must prove, what coverage dimensions mean, and which
malformed inputs must be considered.

## Code Area to Minimum Test Matrix

If a change touches multiple areas, run the union of required tests and use the
highest-risk classification. A row can be waived only with `decision_ref` to an
active reviewed decision or deviation that explains why the touched code path is
not exercised by that test class.

| Touched area | Examples | Required proof before implementation | Minimum targeted tests after implementation | Broader gates before close |
| --- | --- | --- | --- | --- |
| `lexer_parser` Lexer/parser | `crates/trust-syntax/src/lexer/**`, parser grammar, token precedence, recovery | Parser/IEC invariant or bug repro; failing parser test or snapshot first | `cargo test -p trust-syntax`, parser/lexer snapshots, malformed/recovery tests | PR suite; fuzz/nightly if recovery or malformed behavior changed |
| `hir_type_diagnostics` HIR/type system/diagnostics | `crates/trust-hir/**`, type compatibility, standard function type rules | IEC/spec oracle or documented deviation; failing semantic test first | `cargo test -p trust-hir`, targeted semantic tests; IDE diagnostic test if surfaced | Conformance case if user-visible; PR suite |
| `source_lowering_host_harness` Source lowering and host harness | `crates/trust-runtime/src/host/harness/**`, lowering, source-to-runtime build | Dynamic repro when uncertain; failing source-level runtime/conformance case first | Targeted runtime/harness tests; conformance expected artifact when deterministic | Runtime vertical tests if execution behavior changes; PR suite |
| `bytecode_encoder_validator_container` Bytecode encoder/validator/container | `crates/trust-runtime/src/bytecode/**`, `crates/trust-runtime-core/src/bytecode/**` | Crafted bytecode or source repro; failing validator/encoder test first | `cargo test -p trust-runtime --test bytecode_validation`, encoder/core tests, malformed bytecode negatives, in-source runtime-core bytecode tests | Malformed bytecode fuzz/nightly when validator changes; runtime vertical tests if load/apply changes |
| `vm_value_backend_execution` VM/value semantics/backend execution | `crates/trust-runtime/src/runtime/vm/**`, `crates/trust-runtime-core/src/vm/**`, `crates/trust-runtime-core/src/value/**`, register IR, tier1, value operations, coercion | IEC oracle or runtime contract; failing VM/conformance test first; parity cannot be sole proof | VM core tests, runtime-core in-source tests, value tests, conformance case, backend parity as secondary guard | Runtime vertical tests; mutation shard where available |
| `debug_authority_lifecycle` Debug adapter, debug control, force/write/release | `crates/trust-debug/**`, runtime debug bridge, DAP/session/control bridge, extension debug write/force/release flows | Debug/DAP and runtime safety contract; failing debug-control/authz/lifecycle test first for force/write semantics | `cargo test -p trust-runtime --test debug_control`, `cargo test -p trust-runtime --test debug_stepping` where relevant, trust-debug adapter/session tests | VS Code debug journey for visible workflows; runtime vertical tests when scan/force/output behavior changes |
| `runtime_scheduler_lifecycle` Scheduler/lifecycle/watchdog/deadline | `scheduler/**`, `runtime/core/**`, `watchdog.rs` | Debug/reproduce first; failing/protective safety test first | `runtime_safety_fail_closed`, `runtime_reliability`, `complete_program`, deadline/signal tests | Runtime vertical suite; PR/release suite |
| `retain_restart_init_reset` Retain/restart/init/reset | `retain/**`, `runtime/restart.rs`, init services | Retain matrix invariant; failing warm/cold/fault/corrupt retain test first | `retain_integrity`, `runtime_restart`, init/reset tests, conformance retain/init cases | Runtime vertical tests; release suite |
| `process_image_memory_map` Process image and memory map | IO image, `%I/%Q/%M`, VAR_CONFIG, bounds | Failing bounds/mapping test or conformance case first | `process_image`, `io_cycle`, memory-map conformance | Runtime vertical tests; UI/Live Values journeys if visible |
| `modbus_mqtt_gpio_io` Modbus/MQTT/GPIO process I/O | `io/modbus*`, `io/mqtt*`, `io/gpio*`, point maps, queues | Slow/failure/device-state repro first; failing protocol/unit/loopback test first | Protocol unit tests, loopback integration, stale/queue/reconnect tests, `io_multidriver_live` | `runtime_comms_conformance_gate.sh`; hardware-lab before public hardware claim |
| `ethercat_io` EtherCAT | `io/ethercat/**`, adapter/bus/process data/storage | Mockable construction proof or lab proof before ownership changes | `ethercat_driver`, storage/reconnect tests, process data tests | Device-in-loop EtherCAT gate for hardware claims |
| `ads_opcua_connectors` ADS/OPC UA supervisory connectors | `host/ads/**`, `host/opcua/**` | Connection/state/reconnect repro or protocol mock first | `ads_cli_command`, `ads_web_api`, ADS library tests, `opcua_integration`, `opcua_client_runtime` | Runtime comms conformance; TwinCAT/OPC UA lab proof before public claim |
| `control_api_rbac_web` Control/API/RBAC/web routes | `control/**`, API handlers, authz, web ops | Contract/security invariant; failing API/authz test first | `api_smoke`, handler tests, authz/RBAC tests, route integrations | Runtime vertical tests; browser/VS Code proof if visible |
| `hmi_runtime_web_ui` HMI runtime API/web UI | `hmi/**`, `/hmi`, HMI assets, trends/alarms/writes | HMI contract invariant; failing API/widget/authz test first | HMI control tests, `hmi_readonly_integration`, trend/alarm/widget tests | Browser Playwright/screenshot proof; UI journey if workflow changes |
| `ide_source_intelligence` `trust-ide` source intelligence | rename, references, goto, code actions | Source transformation invariant; failing IDE test first for silent-corruption risk | `cargo test -p trust-ide`, conflict/rebind negatives | LSP/VS Code tests if surfaced |
| `lsp_protocol_boundary` `trust-lsp` protocol boundary | sync, positions, diagnostics, cancellation, index cache | LSP protocol invariant; failing protocol test first | `cargo test -p trust-lsp`, snapshots, perf tests for hot paths | `./scripts/prepush_ci_gate.sh`; VS Code npm gate for visible behavior |
| `vscode_extension` VS Code extension | `editors/vscode/src/**`, webviews, commands, debug | User workflow invariant; extension test or journey proof planned first | `npm run lint`, `npm run compile`, `npm test`; test registered in `index.ts` | Screenshot/CDP/journey evidence for visible changes; version sync |
| `public_workflow` User story or public workflow | Workflow specs, tutorials, public docs task promises, HMI/operator journeys, CLI/device setup walkthroughs | Written workflow spec or explicit spec gap first; map actor, steps, success/failure/status behavior, and acceptance evidence | Linked invariant plus contract/journey/CLI/browser proof as appropriate | Public docs and release summary show workflow proof or visible gap |
| `plcopen_import_export` PLCopen import/export | `trust-plcopen/**`, fixtures, CLI/import paths | Vendor/export corpus or malformed XML repro; failing diagnostic test first | `cargo test -p trust-plcopen`; runtime PLCopen command tests if CLI path touched | Real vendor export corpus before public compatibility claim |
| `trust_dev_cli` `trust-dev` CLI/workbench | `crates/trust-dev/**`, dev aliases, commit/test/docs | CLI contract invariant; failing CLI behavior test first | `cargo test -p trust-dev`; command-specific fixtures | Changelog/version if visible |
| `conformance_suite` Conformance suite | `conformance/**`, runner/models/reporting | Contract/schema invariant; failing schema/runner test first | Conformance runner tests; schema validation; run-twice determinism diff | CI conformance job; public docs only from generated result and gaps |
| `verification_tooling` Gate scripts/xtask/verification metadata | `scripts/*gate*`, `xtask/**`, `verification/**` | Self-test or known-bad fixture first | Script syntax/unit tests, validator tests, known-good/known-bad fixtures | Gate observability checks; remote-builder proof |
| `architecture_diagrams` Architecture/data-flow diagrams | `docs/diagrams/**`, ownership/data-flow docs | Code-derived fact or architecture map evidence first | Render diagrams, check drift, claim-check scripts | Update `architecture-improvements.md`; docs gates |
| `public_docs_release_version` Public docs/release/version | `docs/public/**`, `CHANGELOG.md`, versions | Docs/release invariant; link/schema/version check first | Public docs checks, version-release guard, release preflight | Release workflow/tag/latest proof when version changes |
| `unsafe_concurrency` Unsafe/concurrency-sensitive code | unsafe sites, atomics, locks, worker shutdown | Invariant comment and failure model first; deterministic test/tool plan | Unit/concurrency tests; Miri/Loom/sanitizer shard where applicable | Unsafe/concurrency checklist and nightly/deep gate |
| `performance_hot_paths` Performance-sensitive hot paths | scan cycle, VM dispatch, LSP interactive paths | Behavior-lock and performance baseline first | Focused perf/benchmark with correctness guard | Nightly/release perf trend |
| `security_supply_chain` Security and supply chain | dependency manifests, build scripts, release packaging, auth/security config | Security/supply-chain invariant; policy decision before gate | `cargo`/npm audit or deny-style gate where configured, license/provenance checks, RBAC negatives | Release gate includes dependency/security status and artifact trust proof |
| `platform_package` Platform/package behavior | platform-specific paths, VSIX/package binaries, Windows path handling | Platform matrix contract; failing platform or path-hygiene test first | path hygiene, package smoke, platform-targeted test where available | Release platform matrix; CI proof per supported target |
| `refactor_only` Refactor-only change | Any area where behavior must not change | Existing or new behavior-lock tests before edit | Same targeted tests before and after | Relevant broader gate; architecture docs if ownership/data flow changes |

The backticked identifiers are stable routing IDs and are drift-checked against
`verification/matrix.toml` in this reviewed order. `refactor_only` is matched by
planner intent and adds requirements to the areas found from changed paths; it
does not invent an area from the intent. `unsafe_concurrency` and
`performance_hot_paths` route known policy, gate, and hotspot paths only. The
classifier does not infer semantic changes from source text; ordinary source
paths still route through their owning canonical area's fallback globs when no
more-specific taxonomy route matches, and an unmatched path is blocked.
Path globs match complete normalized POSIX paths: `*` and `?` stay within one
path segment, while a complete `**` segment matches zero or more segments. The
same matcher is used for specific routes and canonical-area fallbacks.
`conditional_suite_tiers` are review prompts for the written conditions in this
table; the planner reports them separately and never promotes them to directly
required suites.

Crates with no `tests/` directory still count. The catalog scanner must inspect
practical in-source `#[cfg(test)]` modules, with `trust-runtime-core` as the
proof case.

## Test Classes

Every test catalog entry must state its test class, invariant IDs, oracle,
expected failure mode, and evidence destination. A command name is not enough.
Scanner-backed entries also carry their generated discovery identity; the two
non-native artifact subject kinds are closed to committed case tables and the
reviewed mutation shard runner.

| Test class | Must prove | Minimum content | Does not count |
| --- | --- | --- | --- |
| `debug_reproduction_probe` | Whether a suspected claim is real, rejected, or unproven. | Minimal input/environment, command, observed behavior, source refs, disposition. | Reading code and calling the claim proven. |
| `failing_regression` | Current behavior is wrong and future fix matters. | Minimal repro, oracle-expected behavior, red output before fix, green after. | Test written after fix with no red/protective proof. |
| `protective_red` | Known dangerous behavior is documented before implementation. | Ignored/protective marker, row ID, expected red symptom, unblock condition. | Ignored test with vague reason. |
| `behavior_lock` | Refactor preserves behavior. | Baseline output before edit, same output after edit. | `just test-all` alone. |
| `unit` | A small owner module handles normal, edge, error cases. | Direct assertions; no unnecessary network/time/hardware. | Only checking that code executes. |
| `integration` | Two or more components honor their contract. | Real boundary crossing, realistic fixture/config, expected output/status/error. | Mock-only test for a real boundary claim. |
| `runtime_vertical` | Runtime-facing interface drives runtime core and observes result. | API/CLI/control input, scan execution, process image/status/fault assertion. | Unit proof while claiming end-to-end behavior. |
| `iec_conformance` | Deterministic IEC/ST behavior matches oracle. | ST case, expected artifact, category, cycles/time model, run-twice diff. | VM-vs-eval parity as sole oracle. |
| `negative_malformed_input` | Bad input fails closed with right diagnostic/status. | Malformed payload/source/config, expected diagnostic/error, no partial apply unless designed. | Accepting malformed input because happy path passes. |
| `fuzz` | Parser/decoder/validator survives broad generated input. | Target, seed/minimized corpus rules, budget, crash minimization, regression handoff. | Fuzz run with no corpus, owner, or repro path. |
| `property` | A general invariant holds across generated values. | Generator domain, invariant, shrinking/repro path, bounded execution. | Random examples with no property. |
| `mutation` | Tests catch changes to safety-relevant branches. | Exact focused mutant shard, invariant owner, raw derived outcome, survivor report and resolved durable action; delivered artifact identity where relevant. | Coverage percentage alone, planned selector as an executed result, or association ID as a killed-by claim. |
| `miri_sanitizer_loom` | Unsafe/concurrency assumptions hold or tool is inapplicable. | Focused target, toolchain, invariant, triage path. | Whole-workspace failure used to declare area untestable. |
| `protocol_loopback` | Protocol logic works against controlled endpoint and reports honestly. | Local endpoint, connect/read/write/reconnect/stale/error cases. | TCP reachability as protocol truth. |
| `hardware_lab` | Public hardware claim works on real named equipment. | Device model, firmware, topology, env vars, safety precautions, command, skipped/unproven status. | Mock proof as hardware proof. |
| `soak_stability` | Long behavior stays bounded and observable. | Duration, memory/CPU/jitter/status thresholds, first-failure extraction. | Short happy-path integration run. |
| `performance` | Hot path meets budget or trend without weakening correctness. | Baseline, environment, sample count, threshold, correctness guard. | Optimizing without behavior-lock. |
| `lsp_protocol` | LSP wire contracts and editor-visible semantics hold. | Payloads, position encoding, cancellation/error shape, expected response/diagnostic. | Pure IDE tests for LSP boundary bugs. |
| `vscode_extension` | User-visible extension behavior works through extension host. | Test under `editors/vscode/src/test/suite/**`, registered in `index.ts`, `npm test`. | Unregistered test or compile-only proof. |
| `browser_webview_visual` | Browser-visible change renders and behaves in shipped surface. | Playwright/VS Code capture/CDP evidence, screenshot or DOM assertion, rebuilt assets. | HTTP 200 or stale screenshots. |
| `ui_journey_acceptance` | Complete user workflow is usable and truthful. | Journey runner/evidence, screenshots, JSON/log proof, acceptance audit; linked UI invariants validate only from `ux_accepted` journey rows. | Backend proof without visible journey evidence. |
| `rbac_security` | Unauthorized denied, authorized works, no secret leak. | Role/token setup, allowed/denied cases, audit/status assertions. | Allowed path only. |
| `supply_chain_security` | Dependencies and artifacts meet security/license/provenance rules. | Locked dependency inputs, audit/deny/license output, exceptions with owner/expiry. | One-time manual scan with no policy. |
| `platform_package` | Supported platform/package behavior works. | Platform target, path/encoding assumptions, package smoke, artifact identity. | Local-only Linux proof for platform claim. |
| `release_docs` | Public artifacts match tested behavior and known gaps. | Changelog/version/tag/release/doc checks, generated summary, artifact IDs. | Hand-written docs claim. |
| `architecture_diagram` | Architecture claims match source-derived facts. | Generated map/doctor check, diagram render/drift check. | Hand-authored diagram alone. |
| `metadata_validation` | Verification program is internally consistent. | Schema validation, stale paths, invariant-test-gate links, ignored-test classification. | Markdown prose only. |

Test-class completeness is counted only from the machine-readable planning
matrix and hand-owned catalog. For each mapped area, a required class is present
only when at least one catalog row of that class has a runnable status:
`mapped`, `test_written`, `implemented`, or `validated`. Planned artifacts are
reported separately and do not count. A scanner-backed row whose fact is
`ignored` or `conditional` is not effectively runnable and also does not count;
Phase 3 owns its reviewed ignore classification. Mechanical scanner facts have
no class until an exact `generated_test.discovery_id` binding is reviewed;
source names, paths, command hints, and lexical references cannot supply one.
Reports show counts and debt, never a single quality score.

The ignored-test registry is a non-runnable overlay. It binds a source
discovery identity to one reviewed ignore class, owner, area, reason, and
unblock condition, but it does not map the source fact to an invariant, oracle,
coverage cell, suite, or public claim. `red_protective`, `lab_required`, and
`flaky_quarantined` have additional evidence obligations; absent evidence stays
`unknown` rather than being inferred from source prose. Neither an ignored
record nor its command hint can be used as behavior proof.

## Coverage Dimensions

Every invariant needs a coverage matrix. Each required cell is one of:
`covered`, `covered_by_fuzz`, `not_applicable`, `blocked`, `spec_gap`,
`gap_open`, or `deferred`.

Coverage dimensions are also the canonical case-family vocabulary for committed
case files under `verification/cases/**`. Do not create a second family list in
`gen_cases.py`, `plan_tests.py`, tests, or checklist prose. If a new case family
is needed, add it here, update `verification/matrix.toml`, and explain the
product risk it covers.

Required dimensions:

- `happy_path`
- `boundary_low`
- `boundary_high`
- `below_min`
- `above_max`
- `wrong_type_or_shape`
- `missing_required`
- `extra_or_unknown`
- `duplicate_or_collision`
- `ordering_or_lifecycle`
- `encoding_or_unicode`
- `resource_limit`
- `auth_or_permission`
- `persistence_or_recovery`
- `concurrency_or_cancellation`
- `time_or_clock_fault`
- `hardware_or_network_fault`
- `supply_chain_or_artifact_fault`
- `platform_or_filesystem_variation`

Rules:

- Safety-critical, wrong-result, silent-corruption, and false-status
  invariants cannot be `validated` while applicable failure-mode dimensions are
  `gap_open` or `spec_gap`.
- `covered_by_fuzz` is not enough after a fuzz crash; the minimized crash must
  become a deterministic regression.
- Boundary tests must state the boundary source: IEC, protocol standard,
  product cap, runtime width, or reviewed decision.
- If a broad integration test covers a cell, the catalog must say how its
  assertion fails if that specific cell regresses.
- Catalog schema v2 cannot yet express per-invariant coverage dimensions.
  Phase 2A therefore reports every multi-invariant catalog row as a candidate
  with missing dimensions; ad hoc source text or unknown catalog fields cannot
  satisfy this rule.
- Existing-test size, whole-file similarity, and same-table input shape are
  review candidates only. They never mechanically authorize a move, split,
  rename, or fixture merge.

`decision_table` contract cases may use only data-shaped dimensions in v1:
happy path, boundary values, wrong type or shape, missing/extra fields,
encoding, resource limits, persistence of the touched value, and platform/file
shape where the input itself carries that variation. Runtime events such as
SIGTERM, worker down, slow protocol handshakes, queue-full stop handling, and
hardware reconnect are scenario/fault families. They belong to Phase 8
fault-injection harnesses and `contract_kind = "fault_scenario"` or
`protocol_trace`, not to v1 decision-table case generation.

The machine-readable source of truth for area-to-test-class requirements is
`verification/matrix.toml`. This Markdown table is the reviewed explanation of
that matrix. A drift check must fail if the metadata and this document disagree
after the matrix lands.

## Runtime Anomaly Taxonomy

Phase 8 owns a closed stimulus taxonomy in
`verification/runtime-anomaly-taxonomy.toml`. Its 19 stable class IDs, in
reviewed board order, are:

- `panic`, `timeout`, `deadline`, `watchdog`, `slow_device`, `disconnect`,
  `queue_full`, `stale_data`, `corrupt_retain`, `malformed_bytecode`,
  `bad_config`, `bad_signal`, `partial_web_request`, `disk_error`,
  `clock_step`, `monotonic_wall_clock_divergence`, `suspend_resume`,
  `timer_duration_overflow`, and `allocation_failure_oom`.

The taxonomy describes injected or ordinary-input conditions. It does not
supply expected product behavior. Every existing-test association is
hand-reviewed and bound to one live Rust scanner `discovery_id`, source kind,
path, and name. Names, paths, comments, reference candidates, area fallbacks,
and similar test prose never create an association.

Association kinds are `direct`, `partial`, `context_only`, and
`protective_red`. Only a non-ignored `direct` association is effectively
runnable for the Phase 8 gap report. That status still means only that an
existing assertion exercises part of the named stimulus; it creates no
invariant coverage, oracle, or proof. Partial, context-only, ignored, and
conditional rows remain test gaps.

Every class has one planned primary suite from `pr`, `nightly`, `release`, or
`hardware_lab`, plus optional conditional suites. This is routing intent, not
evidence that a suite ran. Simulated slow-device and disconnect cases may use
nightly while real-device claims retain a conditional `hardware_lab` route.

The scan-cycle allocation review reuses the active runtime-engine statements
that dynamic allocation is absent from the hot path and execution performs no
heap allocation. Allocation-failure/OOM outside that promised path remains
test debt. Restart/time-base behavior uses an exact schema-v1 variant. The
committed `existing_open_gap` state binds only
`SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001`. The migration-ready
`resolved_source` state instead binds an active, oracle-eligible spec source,
its exact durable path, and the same ID as `superseded_gap_id`; mixed fields,
an invented gap, or a public-claim source fails. `existing_open_gap` requires
the gap to remain actionable. `resolved_source` permits that migration state
while the gap is open and remains valid after closure only when the gap's
`resolution_source_ref` is the same source. The resolved-source state remains
an ownership review only: it creates no test coverage, proof, or gap closure.
The committed taxonomy remains in the open-gap state until the execution slice
reaches its reviewed migration row.

Phase 8 adds no fault toggle or production hook. `VERIF-P8-005` and
`VERIF-P8-006` remain open until a general governed harness and an enforceable
design-review boundary exist.

The runtime anomaly denominator gives every one of the 3,220 live Rust test
facts an explicit mapped or reviewed-nonmapping disposition. Mapped rows bind
an exact reviewed mapping ID; nonmapping rows use a closed rationale vocabulary.
The validator rejects overlap, omission, stale identity, or a new unreviewed
fact. This closes `VERIF-P8-002` without converting associations into coverage
or proof.

## Malformed Input Taxonomy

Malformed-input tests must pick from a surface-specific taxonomy. The first
version can be incomplete, but missing classes must be visible as `gap_open`,
`spec_gap`, `blocked`, or `deferred`.

The inventoried bytecode/VM pilot atomizes this prose into reviewed stable IDs
in `verification/malformed-input-taxonomy.toml`, with its human-readable mirror
in `verification/malformed-input-taxonomy.md`. Catalog mappings use only
explicit `malformed_input_class_ids`; names, paths, case IDs, and mutation
associations never create a mapping.

### ST Source, Lexer, Parser

- invalid token bytes or characters,
- unterminated string/comment,
- bad numeric literal,
- missing delimiter,
- extra delimiter,
- invalid operator sequence,
- invalid memory address syntax,
- unsupported keyword/construct,
- excessive nesting or large declaration block,
- recovery after malformed statement.

Minimum assertions:

- no panic or loop,
- diagnostic span stable enough for editor use,
- recovery consumes or advances,
- valid code after malformed region remains visible when applicable.

### HIR and Type Checking

- unknown symbol/type/POU,
- duplicate declaration and shadow/collision,
- type mismatch in assignment, call, return, comparison, arithmetic, field
  access,
- invalid implicit conversion,
- wrong function/FB/method argument count/direction/kind,
- illegal `REF()` target,
- subrange/string/array/enum/struct initializer violations,
- unsupported graphical/non-ST body passed into semantic entry points.

Minimum assertions:

- error-severity diagnostic blocks lowering,
- diagnostic identifies user-facing construct,
- no runtime model/bytecode is produced for rejected semantics.

### PLCopen XML

- malformed XML,
- unsupported namespace or required element missing,
- unsupported FBD/LD/SFC/vendor graphical body,
- supported ST plus benign metadata,
- supported ST plus unsupported executable body,
- CDATA `]]>` edge cases,
- missing/duplicate POU names, source IDs, variables, body elements,
- invalid identifiers/types after XML decode,
- external references/vendor metadata not translatable to ST,
- large/deep XML and repeated elements.

Minimum assertions:

- unsupported executable non-ST rejects loudly with named diagnostic,
- benign metadata does not skip valid ST unless documented,
- importer never synthesizes garbled ST from arbitrary XML text.

### Bytecode Container and Instruction Stream

- bad magic, version, checksum, schema tag,
- truncated/wrong/duplicate/missing section,
- unknown opcode,
- operand index out of bounds,
- jump target outside code or not on instruction boundary,
- stack underflow, leftover, or type mismatch,
- const type incompatible with use,
- parameter direction or call target mismatch,
- reference escape or local-frame reference persistence,
- ambiguous/missing/stale instance owner,
- resource limits for locals, refs, instructions, arg count, call depth.

Minimum assertions:

- invalid bytecode fails validation/load before arbitrary execution,
- error reason is specific,
- VM does not silently execute with stale IDs, wrong tags, or wrong refs.

### Runtime Config, Project Config, HMI Config

- missing required field/section,
- unknown field or enum,
- wrong field type,
- duplicate name/id/device/point/view/widget,
- invalid address/topic/endpoint/node/path,
- unsupported schema version,
- invalid auth/RBAC configuration,
- invalid safe-state/output configuration,
- huge config, deep nesting, large point map, path traversal.

Minimum assertions:

- bad config rejects before partial runtime apply unless atomic fallback exists,
- error points to field/section,
- secrets are not printed.

### Protocol Payloads and Discovery Inputs

- truncated frame/payload,
- wrong length/checksum/transaction/session identifier,
- valid transport endpoint with wrong protocol response,
- unsupported function/message type,
- out-of-order response or stale generation,
- timeout, slow response, reconnect during read/write,
- duplicate point/topic/register/tag,
- wrong type/encoding payload,
- NaN/Inf or unsupported floating-point values crossing IO/comms into REAL
  values,
- disconnect, reconnect storm, stale retained data.

Minimum assertions:

- status is honest: confirmed, likely, port_reachable, stale, degraded, faulted,
- scan cycle does not block on slow/malformed device behavior,
- safety-required output writes are delivered or not-safe is reported.

### LSP and Editor Protocol Inputs

- UTF-16/UTF-8 mismatch and supplementary-plane characters,
- incremental edit range outside document,
- overlapping/stale incremental edits,
- didClose after unsaved dirty edit,
- cancellation while diagnostics/references in flight,
- partial-result duplication,
- missing/evicted document,
- symlink/canonical path collision,
- large workspace and large semantic token response.

Minimum assertions:

- server buffer stays synchronized with editor text,
- cancelled diagnostics do not publish false empty success,
- refactor/source-transform commands refuse unsafe edits with visible reason.

### API, Web, and HMI Payloads

- missing auth token or wrong role,
- wrong JSON type or missing field,
- unknown command/action/widget/write target,
- invalid write value for type/range/quality,
- NaN/Inf or unsupported numeric payload where REAL values are accepted,
- stale runtime generation or disconnected runtime,
- partial HTTP body or slow client,
- oversized payload,
- bad route/method/content type.

Minimum assertions:

- unauthorized action denied,
- bad payload cannot mutate runtime state,
- error is specific and does not leak secrets,
- slow/partial request does not block critical routes unless documented.

### Persisted State, Retain, Cache, and Reports

- missing file,
- truncated file,
- wrong version/schema,
- invalid checksum/hash,
- same-size/same-second cache edge,
- corrupt retain value or wrong type,
- duplicate retained key or stale instance ID,
- read-only path or disk write failure.

Minimum assertions:

- runtime fails closed or degrades visibly,
- no silent retain loss,
- cache invalidation uses content identity strong enough for the claim.

### Supply Chain and Release Artifacts

- missing or modified lockfile,
- unexpected dependency source,
- denied license,
- known vulnerability without reviewed exception,
- unsigned/unverifiable release artifact where signing/checksum is required,
- package version mismatch,
- generated binary not from tested commit.

Minimum assertions:

- release summary states dependency/security status,
- exceptions have owner and expiry,
- artifact identity maps to tested commit and tag.

### Platform and Filesystem Variation

- Windows path separators and drive prefixes,
- case-sensitive vs case-insensitive paths,
- symlinked workspace roots,
- non-UTF-8 paths where supported by platform APIs,
- long paths,
- executable permission differences,
- platform-specific socket/control endpoints.

Minimum assertions:

- platform-specific claims have platform proof,
- path assumptions are explicit,
- unsupported platforms fail with clear diagnostics rather than false success.

## Phase 9 Fuzz Surface Program

The Phase 9 inventory is a separate association audit over existing executable
fuzz mechanisms. It does not broaden the Phase 2 scanner denominator, infer
semantic coverage from names, or promote any coverage cell to
`covered_by_fuzz`.

The first report requires these eight ordered surfaces:

| Surface ID | Reviewed boundary |
| --- | --- |
| `st_lexer_parser` | Structured Text tokenization, parsing, and recovery input |
| `hir_lowering_input` | HIR analysis plus the lowering boundary; HIR-only exercise is partial |
| `plcopen_xml` | PLCopen XML decode/import input |
| `bytecode_container_instructions` | Bytecode container and instruction-stream validation input |
| `protocol_payloads` | Protocol, transport, and runtime communication payload input |
| `config_files` | Runtime, project, protocol, and HMI configuration files |
| `lsp_incremental_edits` | LSP document synchronization and incremental-edit input |
| `hmi_schema_payloads` | HMI schema/API payload input |

The live inventory denominator is eleven executable facts, not every test whose
name contains `malformed` or `random`:

- five parsed cargo-fuzz targets: `syntax_parse`, `hir_semantic`, `ams_frame`,
  `boundary_noop`, and `command_dispatch`;
- five bounded Rust fuzz smokes: malformed bytecode, mesh payload, shared-memory
  header, runtime-cloud API payload, and WAN allowlist parser;
- one bounded Rust property smoke for generated parser initializer recovery.

The five cargo-fuzz records are discovered from every tracked cargo-fuzz
manifest, including the crate-local ADS workspace that Phase 2 intentionally
excluded. The six Rust records resolve by exact live scanner discovery ID. The
reviewed target IDs are stable hand-owned identities; target kind is closed to
`cargo_fuzz` or `bounded_rust_smoke`, and association strength is closed to
`direct` or `partial`.

The initial surface derivation is intentionally strict:

- `st_lexer_parser` and `protocol_payloads` are `cargo_fuzz_target`;
- `bytecode_container_instructions` is `smoke_only`;
- `hir_lowering_input` and `lsp_incremental_edits` are `partial_only`;
- `plcopen_xml`, `config_files`, and `hmi_schema_payloads` are `unmapped`.

Thus six of eight surfaces remain report gaps. A direct bounded smoke is not a
substitute for a sustained cargo-fuzz target, and a lower-layer HIR edit cycle
does not prove the LSP incremental-edit boundary. The WAN allowlist parser is a
partial protocol-payload association; it is not a config-file parser and cannot
fill that surface.

Each target has one `primary_tier`: `pr_smoke`, `nightly`, or
`manual_extended`. `additional_tiers` may name another real mode from the same
vocabulary, but is sorted, duplicate-free, and disjoint from the primary tier.
Only `syntax_parse` and `hir_semantic` initially add `nightly` to their
`pr_smoke` primary tier. Tier assignment states where a target is intended or
observed to run; a separate enforcement field preserves whether that path is
required, planned, or manual. It is not passing evidence.

Required/wired tier claims are source-bound: reviewed gate scripts and workflows
retain exact raw-byte digests, scripts retain executable mode, relevant trigger
and job blocks remain unique and digest-bound, and workspace-wide test claims
retain the target crate in the effective default Cargo workspace. Lexical
command matches alone are not an execution authority.
Bounded Rust smokes also require a fresh `not_ignored` scanner fact; an ignored
or conditional test cannot keep a runnable tier merely because its filter
command still exits successfully.
This audit does not evaluate arbitrary Rust `cfg` expressions or prove
parent-module reachability. Its `wired` state is command-path metadata, never an
observed execution result.

Generated seed corpora and raw crash artifacts stay machine-local and ignored
under the owning fuzz workspace. CI uploads remain run artifacts unless a
durable evidence row binds the run, artifact, and retention. Phase 9 checks
storage geometry and ignore posture only; corpus bytes, counts, quality,
coverage, and completeness are not assessed. A minimized crash becomes useful
regression input only after it is promoted to a tracked deterministic native
test or fixture with a reviewed identity and runnable command.

That final crash handoff is not enforced today. There is no exhaustive crash
ledger joining every raw or minimized artifact to its deterministic regression,
so `VERIF-P9-005` remains open. The generated audit reports target/surface gaps
and the open handoff boundary without running a fuzz campaign, closing a spec
gap, or creating proof.
