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
| Lexer/parser | `crates/trust-syntax/src/lexer/**`, parser grammar, token precedence, recovery | Parser/IEC invariant or bug repro; failing parser test or snapshot first | `cargo test -p trust-syntax`, parser/lexer snapshots, malformed/recovery tests | PR suite; fuzz/nightly if recovery or malformed behavior changed |
| HIR/type system/diagnostics | `crates/trust-hir/**`, type compatibility, standard function type rules | IEC/spec oracle or documented deviation; failing semantic test first | `cargo test -p trust-hir`, targeted semantic tests; IDE diagnostic test if surfaced | Conformance case if user-visible; PR suite |
| Source lowering and host harness | `crates/trust-runtime/src/host/harness/**`, lowering, source-to-runtime build | Dynamic repro when uncertain; failing source-level runtime/conformance case first | Targeted runtime/harness tests; conformance expected artifact when deterministic | Runtime vertical tests if execution behavior changes; PR suite |
| Bytecode encoder/validator/container | `crates/trust-runtime/src/bytecode/**`, `crates/trust-runtime-core/src/bytecode/**` | Crafted bytecode or source repro; failing validator/encoder test first | `cargo test -p trust-runtime --test bytecode_validation`, encoder/core tests, malformed bytecode negatives, in-source runtime-core bytecode tests | Malformed bytecode fuzz/nightly when validator changes; runtime vertical tests if load/apply changes |
| VM/value semantics/backend execution | `crates/trust-runtime/src/runtime/vm/**`, `crates/trust-runtime-core/src/vm/**`, `crates/trust-runtime-core/src/value/**`, register IR, tier1, value operations, coercion | IEC oracle or runtime contract; failing VM/conformance test first; parity cannot be sole proof | VM core tests, runtime-core in-source tests, value tests, conformance case, backend parity as secondary guard | Runtime vertical tests; mutation shard where available |
| Debug adapter, debug control, force/write/release | `crates/trust-debug/**`, runtime debug bridge, DAP/session/control bridge, extension debug write/force/release flows | Debug/DAP and runtime safety contract; failing debug-control/authz/lifecycle test first for force/write semantics | `cargo test -p trust-runtime --test debug_control`, `cargo test -p trust-runtime --test debug_stepping` where relevant, trust-debug adapter/session tests | VS Code debug journey for visible workflows; runtime vertical tests when scan/force/output behavior changes |
| Scheduler/lifecycle/watchdog/deadline | `scheduler/**`, `runtime/core/**`, `watchdog.rs` | Debug/reproduce first; failing/protective safety test first | `runtime_safety_fail_closed`, `runtime_reliability`, `complete_program`, deadline/signal tests | Runtime vertical suite; PR/release suite |
| Retain/restart/init/reset | `retain/**`, `runtime/restart.rs`, init services | Retain matrix invariant; failing warm/cold/fault/corrupt retain test first | `retain_integrity`, `runtime_restart`, init/reset tests, conformance retain/init cases | Runtime vertical tests; release suite |
| Process image and memory map | IO image, `%I/%Q/%M`, VAR_CONFIG, bounds | Failing bounds/mapping test or conformance case first | `process_image`, `io_cycle`, memory-map conformance | Runtime vertical tests; UI/Live Values journeys if visible |
| Modbus/MQTT/GPIO process I/O | `io/modbus*`, `io/mqtt*`, `io/gpio*`, point maps, queues | Slow/failure/device-state repro first; failing protocol/unit/loopback test first | Protocol unit tests, loopback integration, stale/queue/reconnect tests, `io_multidriver_live` | `runtime_comms_conformance_gate.sh`; hardware-lab before public hardware claim |
| EtherCAT | `io/ethercat/**`, adapter/bus/process data/storage | Mockable construction proof or lab proof before ownership changes | `ethercat_driver`, storage/reconnect tests, process data tests | Device-in-loop EtherCAT gate for hardware claims |
| ADS/OPC UA supervisory connectors | `host/ads/**`, `host/opcua/**` | Connection/state/reconnect repro or protocol mock first | `ads_cli_command`, `ads_web_api`, ADS library tests, `opcua_integration`, `opcua_client_runtime` | Runtime comms conformance; TwinCAT/OPC UA lab proof before public claim |
| Control/API/RBAC/web routes | `control/**`, API handlers, authz, web ops | Contract/security invariant; failing API/authz test first | `api_smoke`, handler tests, authz/RBAC tests, route integrations | Runtime vertical tests; browser/VS Code proof if visible |
| HMI runtime API/web UI | `hmi/**`, `/hmi`, HMI assets, trends/alarms/writes | HMI contract invariant; failing API/widget/authz test first | HMI control tests, `hmi_readonly_integration`, trend/alarm/widget tests | Browser Playwright/screenshot proof; UI journey if workflow changes |
| `trust-ide` source intelligence | rename, references, goto, code actions | Source transformation invariant; failing IDE test first for silent-corruption risk | `cargo test -p trust-ide`, conflict/rebind negatives | LSP/VS Code tests if surfaced |
| `trust-lsp` protocol boundary | sync, positions, diagnostics, cancellation, index cache | LSP protocol invariant; failing protocol test first | `cargo test -p trust-lsp`, snapshots, perf tests for hot paths | `./scripts/prepush_ci_gate.sh`; VS Code npm gate for visible behavior |
| VS Code extension | `editors/vscode/src/**`, webviews, commands, debug | User workflow invariant; extension test or journey proof planned first | `npm run lint`, `npm run compile`, `npm test`; test registered in `index.ts` | Screenshot/CDP/journey evidence for visible changes; version sync |
| User story or public workflow | Workflow specs, tutorials, public docs task promises, HMI/operator journeys, CLI/device setup walkthroughs | Written workflow spec or explicit spec gap first; map actor, steps, success/failure/status behavior, and acceptance evidence | Linked invariant plus contract/journey/CLI/browser proof as appropriate | Public docs and release summary show workflow proof or visible gap |
| PLCopen import/export | `trust-plcopen/**`, fixtures, CLI/import paths | Vendor/export corpus or malformed XML repro; failing diagnostic test first | `cargo test -p trust-plcopen`; runtime PLCopen command tests if CLI path touched | Real vendor export corpus before public compatibility claim |
| `trust-dev` CLI/workbench | `crates/trust-dev/**`, dev aliases, commit/test/docs | CLI contract invariant; failing CLI behavior test first | `cargo test -p trust-dev`; command-specific fixtures | Changelog/version if visible |
| Conformance suite | `conformance/**`, runner/models/reporting | Contract/schema invariant; failing schema/runner test first | Conformance runner tests; schema validation; run-twice determinism diff | CI conformance job; public docs only from generated result and gaps |
| Gate scripts/xtask/verification metadata | `scripts/*gate*`, `xtask/**`, `verification/**` | Self-test or known-bad fixture first | Script syntax/unit tests, validator tests, known-good/known-bad fixtures | Gate observability checks; remote-builder proof |
| Architecture/data-flow diagrams | `docs/diagrams/**`, ownership/data-flow docs | Code-derived fact or architecture map evidence first | Render diagrams, check drift, claim-check scripts | Update `architecture-improvements.md`; docs gates |
| Public docs/release/version | `docs/public/**`, `CHANGELOG.md`, versions | Docs/release invariant; link/schema/version check first | Public docs checks, version-release guard, release preflight | Release workflow/tag/latest proof when version changes |
| Unsafe/concurrency-sensitive code | unsafe sites, atomics, locks, worker shutdown | Invariant comment and failure model first; deterministic test/tool plan | Unit/concurrency tests; Miri/Loom/sanitizer shard where applicable | Unsafe/concurrency checklist and nightly/deep gate |
| Performance-sensitive hot paths | scan cycle, VM dispatch, LSP interactive paths | Behavior-lock and performance baseline first | Focused perf/benchmark with correctness guard | Nightly/release perf trend |
| Security and supply chain | dependency manifests, build scripts, release packaging, auth/security config | Security/supply-chain invariant; policy decision before gate | `cargo`/npm audit or deny-style gate where configured, license/provenance checks, RBAC negatives | Release gate includes dependency/security status and artifact trust proof |
| Platform/package behavior | platform-specific paths, VSIX/package binaries, Windows path handling | Platform matrix contract; failing platform or path-hygiene test first | path hygiene, package smoke, platform-targeted test where available | Release platform matrix; CI proof per supported target |
| Refactor-only change | Any area where behavior must not change | Existing or new behavior-lock tests before edit | Same targeted tests before and after | Relevant broader gate; architecture docs if ownership/data flow changes |

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
| `mutation` | Tests catch changes to safety-relevant branches. | Mutant shard, invariant owner, survivor report and action. | Coverage percentage alone. |
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
Phase 3 owns its reviewed ignore classification. Mechanical scanner facts have no class
until an exact `generated_test.discovery_id` binding is reviewed; source names,
paths, command hints, and lexical references cannot supply one. Reports show
counts and debt, never a single quality score.

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

## Malformed Input Taxonomy

Malformed-input tests must pick from a surface-specific taxonomy. The first
version can be incomplete, but missing classes must be visible as `gap_open`,
`spec_gap`, `blocked`, or `deferred`.

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
