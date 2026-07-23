# Phase 18 Zero-Debt Execution Board

Status: zero-debt contract accepted by commit-bound independent R3 review.
Findings `P18-C01` through `P18-C08` and `P18-R2-01` are resolved.

Current row: `VERIF-P18-003`, starting with exact slice
`P18-CODE-CODE_E78D5BAEDF1B5EE0A6C2DEA95C1E40ADF75CADF7F3FE427F6BDA5738D8453BC8`.
The accepted P18-002 ledger owns and orders every current fact; it does not
close any semantic, mapping, execution, or adequacy population.

## Objective

Finish the PLC verification program with zero known semantic, mapping,
execution, and adequacy debt. Structural accounting is not proof. Every live
behavior-bearing code fact must terminate in an authoritative specification,
invariant, executed test, suite, and evidence chain. Every discovered test must
have an accepted proof contract or be removed with independently reviewed
no-loss evidence.

The frozen review baseline at
`ee00258c42cadd98ac70a5c6523957f5ee44fe7c` contains the complete scoped code
denominator, including product and verification tooling:

| Population | Count |
| --- | ---: |
| live code facts | 15,229 |
| `specified` | 56 |
| `spec_gap` | 15,138 |
| `inherits_spec` | 30 |
| `generated` | 2 |
| `non_behavioral` | 2 |
| `vendored_external` | 1 |
| live test facts | 4,038 |
| `catalog_mapped` | 261 |
| `reviewed_nonmapping` | 3,777 |
| no reviewed specification/invariant binding | 3,687 |
| gate-inventory-owned nonmapping | 59 |
| ignored-test-registry-owned nonmapping | 23 |
| fuzz-program-owned nonmapping | 8 |

These counts are debt baselines, not accepted completion states.

## Final Zero Equations

Final acceptance requires all equations to hold on one frozen candidate:

```text
live code facts with disposition spec_gap                         = 0
live code facts referencing a registered_backlog gap              = 0
missing_spec + missing_tests + unmapped + stale review findings    = 0
duplicate + invented + overlapping + unreviewed code facts         = 0
reviewed_nonmapping + uncataloged + unreviewed test facts           = 0
required tests/campaigns ignored, skipped, filtered out, or unrun   = 0
zero-test filters and skip-as-success wrappers                      = 0
unexplained uncovered behavior-bearing functions/branches/conditions = 0
surviving viable mutants                                            = 0
unowned or unclosed campaign/proof findings                         = 0
```

The `spec_gap` equation is independent of gap lifecycle status. Marking a gap
`closed`, `registered_backlog`, or non-actionable while a live code fact still
terminates in it is a blocking failure. Closed gap records may remain as
history only after every affected live fact is rebound to a valid terminal
chain or removed from the live denominator.

Every required proof-bearing test or campaign must execute successfully in
every named applicable authority lane. Owned inventory is not execution. An
ignored, skipped, filtered-out, undiscovered, unavailable, timed-out,
harness-failed, or zero-test case remains blocking. It may leave the execution
denominator only through reviewed removal with exact no-loss evidence.

## Structural Disposition Rules

The following dispositions are allowed only under these exact final rules:

- `inherits_spec`: an exact acyclic parent chain ending in a `specified`
  boundary that executed successfully and is mutation-sensitive; cross-owner
  or cross-area inheritance requires an explicit reviewed decision.
- `generated`: exact generator code identity, generator/input closure, pinned
  tool, reproducible output digest, and executed product-boundary proof.
- `non_behavioral`: source-derived proof that the fact cannot affect shipped
  behavior, risk, packaging, configuration, or proof decisions. A rationale
  string, path, name, owner, or absence of a risk signal is insufficient.
- `vendored_external`: exact provenance/version/revision, reviewed boundary
  contract, vulnerability/supply-chain posture, and an executed product
  boundary test.

Each rule requires known-good and known-bad fixtures. Final validation must
reject inheritance cycles or unexecuted parents, stale generators or inputs,
behavior-bearing `non_behavioral` claims, and vendored code without an executed
boundary.

## Canonical Machine-Readable Substrate

`VERIF-P18-001` must create these reviewed contracts before semantic burn-down
starts:

| Contract | Canonical path |
| --- | --- |
| exact code/test burn-down ledger | `verification/p18-zero-debt-ledger.toml` |
| ledger schema | `verification/schemas/p18-zero-debt-ledger.schema.json` |
| proof/campaign budgets | `verification/p18-proof-budgets.toml` |
| budget schema | `verification/schemas/p18-proof-budgets.schema.json` |
| supported claim/platform matrix | `verification/p18-claim-matrix.toml` |
| claim schema | `verification/schemas/p18-claim-matrix.schema.json` |
| thin enforcing CLI | `scripts/p18_zero_debt_gate.py` |
| canonical report root | `target/gate-artifacts/p18-zero-debt/<full-sha>/` |

The implementation must use separate per-responsibility producer/validator
modules for semantic chains, test mapping, assertion reachability, coverage,
mutation, fuzz/property/differential campaigns, concurrency/time, unsafe/FFI,
hardware/endurance, rendered UX, platform/claim scope, and release evidence.
No god validator may combine those responsibilities.

Every ledger/budget/matrix row records:

- exact denominator identities, owner, area, risk, and prerequisite rows;
- eligible specification/decision and invariant identities;
- producer and independent validator commands/modules;
- pinned tool/version, toolchain, environment or named hardware topology;
- fixed seeds, input/corpus digests, shards, feature/platform matrix, and
  predeclared duration, iteration, coverage, mutation, or soak stop criteria;
- exact numerator, denominator, and zero-debt acceptance expression;
- canonical artifact path, schema, frozen source SHA, complete input closure,
  timestamps, exit status, and SHA-256 digest;
- known-good and known-bad fixture identities;
- CI/release route and reopen/invalidation dependencies.

Budgets are independently reviewed and frozen before a campaign starts. They
cannot be reduced after seeing results. A one-second/default-empty campaign,
manually narrowed target set, early stop without its predeclared plateau rule,
or post-run soak-duration choice is blocking.

The enforcing command is:

```text
python3 scripts/p18_zero_debt_gate.py --mode strict \
  --out-dir target/gate-artifacts/p18-zero-debt/<full-sha>
```

It must emit canonical JSON and Markdown plus per-proof-class artifact indexes,
and validators must reconstruct all summaries from raw artifacts.

## Dependency and Invalidation Order

1. `VERIF-P17-006` must close from its independent pilot acceptance.
2. A fresh independent review must close `VERIF-P18-000`.
3. `VERIF-P17-012` may then close only as the accepted Phase 17 denominator and
   fail-closed enforcement baseline. Its previously named broad milestone is
   deferred and merged into the one final Phase 18 candidate milestone.
4. `VERIF-P18-001` creates and tests the machine-readable substrate.
5. `VERIF-P18-002` freezes the deterministic burn-down ledger and slice order.
6. For each slice, `P18-003` establishes specification and invariant authority;
   `P18-005` maps or removes existing tests and `P18-006` adds and executes
   missing tests; `P18-004` then validates the complete bidirectional chain;
   and `P18-007` finally proves dynamic reachability and assertion strength.
   `P18-005` and `P18-006` may proceed in parallel only after their slice's
   `P18-003` authority is accepted.
7. Adequacy/environment rows `P18-008` through `P18-015` may run in parallel
   only for slices whose corrected chain `P18-003`, `P18-005`/`P18-006`,
   `P18-004`, and `P18-007` is closed and whose budget records were accepted
   before execution.
8. A failure in coverage, mutation, fuzz/property, concurrency/time,
   unsafe/FFI, hardware/endurance, rendered UX, platform, or claim verification
   reopens the owning spec/test/code slice and invalidates every downstream
   artifact that consumed its old source, metadata, budget, or evidence digest.
9. After `P18-001..015` are closed, freeze one candidate for `P18-016`; any
   source, validator, metadata, instruction, base, or evidence-contract change
   invalidates that candidate and returns to focused review.
10. `P18-017` binds the exact accepted candidate through main/tag/public proof.
11. `P18-018` is the final independent raw-evidence verdict and program close.

No pre-Phase-18 `fmt`/`clippy`/`test-all` run is the program's final milestone.
There is exactly one final broad candidate milestone, owned by `P18-016`.

## Execution Rows

- [x] `VERIF-P18-000` Obtain independent acceptance of this complete zero-debt
  contract, exact baseline arithmetic, zero equations, structural disposition
  rules, machine-readable substrate, budgets, dependency order, invalidation
  rules, stop gates, and final proof sequence. No implementation row is
  authorized before a commit-bound `CLEAR` or accepted
  `CLEAR-WITH-FINDINGS` verdict.
  Commit-bound R3 review returned `CLEAR` for exact contract commit
  `7c08f5a8fd31fb2f4c770d316656f0b78ff62673`; all reviewed file hashes
  matched their commit blobs, findings `P18-C01..C08` and `P18-R2-01` were
  resolved, and the review explicitly authorized the `P17-012` transition and
  `P18-001` as the next Phase 18 row after that transition. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-21-p18-zero-debt-contract-r3-review.md`.
- [x] `VERIF-P18-001` Implement the canonical schemas, ledgers, claim matrix,
  thin CLI, separate producer/validator modules, canonical artifact indexes,
  and negative fixtures defined above. Known-bad fixtures must fail for every
  missing schema field, stale digest, absent raw artifact, weakened budget,
  skipped producer, zero-test execution, and disabled validator catcher.
  Closed on clean detached `trust-builder` producer commit
  `e23e113d6025bdf2767534e77e70d8fc57f46443`: 7/7 focused contract/catcher
  tests, 882 metadata records, the 270-to-4,039 test-catalog join, diagram
  drift, and full code-spec accounting over 15,258 facts passed. The canonical
  report and 20-row artifact index bind the independent frozen contract SHA,
  exact clean producer SHA, normalized input closure, and raw artifact
  digests. Report mode remains report-only and strict mode remains open with
  15,138 `spec_gap`, 15,135 `registered_backlog`, 3,777
  `reviewed_nonmapping`, 23 ignored/skipped/filtered/unrun, and 10 unmeasured
  proof-class facts. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-21-p18-zero-debt-substrate.md`.
- [x] `VERIF-P18-002` Replace all eleven area-wide
  `SPEC_GAP_P17_HISTORICAL_CODE_*` populations with a deterministic fact-level
  semantic burn-down ledger. Group facts only when one reviewed requirement
  genuinely governs every member. Freeze owner, risk, authority, exact code and
  test identities, decisions, tests, environments, budgets, dependency slice,
  and blockers. Keep raw per-area counts visible after every slice.
  Closed on clean detached `trust-builder` artifact commit
  `103a619719da3f9cc2de2ce45d0b5b7f337042bc`, generated from exact clean input
  commit `51e742cfc9dda41ed40d55ed6088f26482dd425f`. The canonical ledger contains
  one open slice for each of 15,274 code and 4,040 test identities, with zero
  omission, duplication, or unowned slices. It preserves 15,138 `spec_gap`,
  15,135 `registered_backlog`, 3,777 `reviewed_nonmapping`, 23 ignored or
  conditional, 337 unresolved test-area, and every unmeasured proof-class
  blocker. Exact source/input provenance, 12/12 catchers, 884 metadata records,
  the 271-to-4,040 test join, diagram drift, and full code-spec accounting all
  passed. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-21-p18-fact-level-burn-down-ledger.md`.
- [ ] `VERIF-P18-003` Resolve every observable-behavior specification gap
  and establish its reviewed invariant authority before mapping or writing
  expected-result tests. Use IEC/PLCopen text, a reviewed product decision, a
  protocol/vendor contract, or another eligible oracle. Never infer expected
  behavior from implementation, output, tests, comments, names, or neighboring
  code. Resolve the complete `CURRENT_DT` clock, epoch, timezone, precision,
  range, failure, simulation, and replay boundary. Add a negative fixture
  proving a gap marked closed while live facts still use `spec_gap` fails the
  strict gate.

  Signed-integer slice progress (2026-07-22): exact slice
  `P18-CODE-CODE_F72E3E7BC892973CF9344497078A35D93380745CDC5DB4820F5CFB302AAEE14A`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_SEMANTICS_001#6-8-signed-integer-result-materialization` and
  `RT_SIGNED_INTEGER_MATERIALIZATION_001`. Its direct boundary matrix and
  generated decision-table runner are catalog-mapped, and a clean
  producer-authentic lock pair promotes the invariant to `G1`. This is slice
  progress only: `VERIF-P18-003` remains globally open. This exact slice's
  assertion-reachability item is now closed; mutation-sensitivity and the
  broad remote gates remain explicit debt before it can satisfy all downstream
  Phase 18 rows.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-signed-integer-materialization.md`.

  Integer-normalization slice progress (2026-07-22): exact sequence-two
  slice
  `P18-CODE-CODE_48DB0C5559CB11931C99FDD5BA0812B4045ACFFE01AF8DDB1E3BA4BA16F4087D`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_SEMANTICS_001#6-9-integer-runtime-normalization-to-i64` and
  `RT_INTEGER_I64_NORMALIZATION_001`. Its direct boundary matrix and generated
  decision-table runner are catalog-mapped, and a clean producer-authentic
  lock pair promotes the invariant to `G1`. This is slice progress only:
  this exact slice's assertion-reachability item is now closed, while mutation
  sensitivity, the broad remote gates, and `VERIF-P18-003` globally remain
  open. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-integer-i64-normalization.md`.

  Unsigned-materialization slice progress (2026-07-22): exact sequence-three
  slice
  `P18-CODE-CODE_A438DB9396B2895A5E07F5891D9467E790F822347F344B1F6AF0AB99C342BE99`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_SEMANTICS_001#6-10-unsigned-integer-result-materialization`
  and `RT_UNSIGNED_INTEGER_MATERIALIZATION_001`. Its direct boundary matrix
  and generated decision-table runner are catalog-mapped, and a clean
  producer-authentic lock pair promotes the invariant to `G1`. This is slice
  progress only: this exact slice's assertion-reachability item is now closed,
  while mutation sensitivity, the broad remote gates, and `VERIF-P18-003`
  globally remain open. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-unsigned-integer-materialization.md`.

  Retain-snapshot insertion slice progress (2026-07-22): exact sequence-four
  slice
  `P18-CODE-CODE_F4ABB9ACC5495574EA09AD38CFC84F2BDE1D0BACA961B3CEAF9D9E44992E3F53`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_ENGINE_001#retain-snapshot-identity-and-order` and
  `RT_RETAIN_SNAPSHOT_INSERT_001`. Its direct state-transition matrix and
  generated decision-table runner are catalog-mapped, and a clean
  producer-authentic lock pair promotes the invariant to `G1`. This is slice
  progress only: this exact slice's assertion-reachability item is now closed,
  and the refreshed fact ledger accounts it as `specified` with both mapped
  tests. Mutation sensitivity, the broad remote gates, and `VERIF-P18-003`
  globally remain open. The active pointer advances to sequence-five symbol
  `trust_runtime_core::task::evaluate_task_readiness`. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-retain-snapshot-insert.md`.

  Task-readiness slice progress (2026-07-22): exact sequence-five slice
  `P18-CODE-CODE_9081A7CEC84BFEF134E282B9DE3838334C15CBA557479C25AF9F230C205C2A73`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_ENGINE_001#task-readiness-and-overrun-accounting` and
  `RT_TASK_READINESS_001`. Five historical `reviewed_nonmapping` unit tests are
  now catalog-mapped and strengthened, and a generated seven-part trace covers
  event edges, periodic suppression, non-positive intervals, clock steps,
  coalescing, and saturating overruns. A clean producer-authentic lock pair
  promotes the invariant to `G1`. All six mapped tests independently catch
  their reviewed source mutations, raising the cumulative assertion result to
  14/277 with zero survivors or harness failures. This is slice progress only:
  the refreshed fact ledger now accounts the code fact as `specified`, moves
  the five historical unit tests out of `reviewed_nonmapping`, and accounts the
  generated trace as mapped. Mutation sensitivity, broad remote gates, and
  `VERIF-P18-003` globally remain open. The active pointer advances to
  sequence-six symbol `trust_runtime_core::watchdog::FaultSubsystem::clear`.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-task-readiness.md`.

  Fault-state clear slice progress (2026-07-22): exact sequence-six slice
  `P18-CODE-CODE_E58D6E19401CE00AC50751C139562696AB92E09720F9EBA17F89934B01312CC0`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_ENGINE_001#fault-state-clear-lifecycle` and
  `RT_FAULT_STATE_CLEAR_001`. The historical `reviewed_nonmapping` unit test is
  now catalog-mapped and strengthened, and a generated two-part trace covers
  recorded-fault clearing plus idempotent healthy clearing while preserving
  configured policy. A clean producer-authentic lock pair promotes the
  invariant to `G1`. Both mapped tests independently catch the reviewed
  clear-to-no-op mutation, raising the cumulative assertion result to 16/279
  with zero survivors or harness failures. This is slice progress only:
  the refreshed fact ledger now accounts the code fact as `specified`, moves
  the historical unit test out of `reviewed_nonmapping`, and accounts the
  generated trace as mapped. Mutation sensitivity, broad remote gates, and
  `VERIF-P18-003` globally remain open. The active pointer advances to
  sequence-seven symbol `trust_runtime_core::watchdog::FaultSubsystem::decision`.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-fault-state-clear.md`.

  Fault-policy decision slice progress (2026-07-22): exact sequence-seven
  slice
  `P18-CODE-CODE_E78D5BAEDF1B5EE0A6C2DEA95C1E40ADF75CADF7F3FE427F6BDA5738D8453BC8`
  is rebound from the historical runtime-safety gap to
  `SPEC_RUNTIME_ENGINE_001#fault-policy-decision-projection` and
  `RT_FAULT_POLICY_DECISION_001`. The historical core and host
  `reviewed_nonmapping` tests are now catalog-mapped and strengthened, and a
  generated three-policy trace executes every projection from both healthy and
  recorded-fault holders. A clean producer-authentic lock pair promotes the
  invariant to `G1`. All three mapped tests independently catch the reviewed
  configured-policy no-op mutation, raising the cumulative assertion result to
  19/282 with zero survivors or harness failures. This is slice progress only:
  mutation sensitivity, refreshed fact-ledger accounting, broad remote gates,
  and `VERIF-P18-003` globally remain open. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-fault-policy-decision.md`.

  Runtime/CLI coherent-batch progress (2026-07-22): the harness protocol,
  runtime entrypoint, ADS fail-closed behavior, watchdog contracts, benchmark
  surfaces, project check, workbench forwarding, Git initialization,
  build/CI, and guided setup/wizard subsystems have now been reviewed as
  specification-first batches. After the setup batch, the live code map has
  15,301 facts: 331 `specified`, 14,935 `spec_gap`, and 35 reviewed
  non-gap dispositions. The test denominator has 4,119 facts: 409
  `catalog_mapped` and 3,710 `reviewed_nonmapping`; the catalog contains 417
  mapped tests. The setup batch specifically binds 55 scanner-derived code
  facts to `SPEC_DEVELOPER_WORKFLOWS_001#project-initialization` through
  `RT_GUIDED_PROJECT_ARTIFACTS_001`, `RT_SETUP_MODE_ROUTING_001`, and
  `RT_SETUP_BROWSER_SECURITY_001`; maps 11 historical tests; and adds 21
  focused tests. Remote focused proof passes 19/19 setup binary tests and
  13/13 setup command tests. Red-green regressions fixed unsafe omitted browser
  write defaults, interactive port overflow, and silent acceptance of
  browser-only CLI options, including explicitly supplied default values. The
  broad `trust_runtime::cli::Command` enum remains an honest `spec_gap` after
  its scanner identity was refreshed automatically; setup proof does not claim
  to specify unrelated CLI variants. Metadata, staleness, and planner checks
  reach only the two deferred retain/restart mutation-digest mismatches.
  `VERIF-P18-003`, assertion reachability, mutation sensitivity, and the final
  broad remote gates remain globally open.

  Runtime-configuration coherent-batch progress (2026-07-22): the closed
  runtime/I/O schemas, connector sidecar loading, omission defaults, explicit
  values, security groups, and value-width boundaries are now governed by
  `SPEC_RUNTIME_ENGINE_001#runtime-configuration-loading-and-validation-contract`
  through `RT_CONFIG_SCHEMA_001`, `RT_CONFIG_BUNDLE_LOAD_001`, and
  `RT_CONFIG_VALUE_BOUNDS_001`. Automation refreshed all 95 live configuration
  code facts to `specified`, mapped all 71 historical configuration tests, and
  registered five new focused regressions. Remote focused proof passes 77/77
  configuration-filtered tests. Red-green repairs reject duration values that
  previously panicked during signed-nanosecond conversion, safe-state integers
  that previously narrowed across BYTE/WORD/DWORD widths, mixed I/O forms that
  silently ignored legacy fields, and blank explicit paths or collection/map
  entries and scalar credentials/selectors that previously defaulted,
  disappeared, or downgraded authentication/source pins. The live code map now
  has 15,304 facts: 426 `specified`, 14,843 `spec_gap`, and 35 reviewed non-gap
  dispositions. The test denominator has 4,124 facts: 485 `catalog_mapped` and
  3,639 `reviewed_nonmapping`; the catalog contains 493 mapped tests. Metadata
  and staleness checks reach only the two deferred retain/restart mutation-
  provenance mismatches. `VERIF-P18-003`, assertion reachability, mutation
  sensitivity, and the final broad remote gates remain globally open.

  Runtime Cloud action/profile coherent-batch progress (2026-07-22): the
  product specification now owns action identity, unique nonempty targets,
  preflight/dispatch correlation, role and target denials, secure profile
  preconditions, and the `dev`/`plant`/`wan` write-policy distinction. Generated
  semantic registration binds every live `runtime_cloud::profile_policy` and
  `runtime_cloud::routing` fact plus the complete focused unit and HTTP
  preflight integration set to
  `SPEC_RUNTIME_ENGINE_001#runtime-cloud-action-preflight-and-profile-contract`
  through `RT_RUNTIME_CLOUD_ACTION_POLICY_001`; the scoped audit rejects any
  unrelated metadata change. Expected-red tests exposed and focused-green
  repairs fixed WAN-only allowlist policy leaking into `dev`/`plant`, an empty
  target list becoming an allowed no-op, blank or duplicate target acceptance,
  and four Admin-only runtime/mesh configuration keys being classified as
  Engineer writes. The route integration test that encoded the obsolete
  default-`dev` denial was renamed and repaired to the documented profile
  contract, with its old scanner identity removed automatically. Remote focused
  proof passes the complete Runtime Cloud unit filter, the preflight route
  filter, and the Runtime Cloud architecture suite. The changes remain inside
  the existing profile/routing policy owners, introduce no transport or runtime
  execution dependency, keep files below the large-file threshold, and require
  no diagram update. The durable `prove.py` wrapper remains unable to write the
  paired records because the two pre-existing retain/restart mutation
  provenance mismatches fail global metadata validation; those digests were not
  edited manually. `VERIF-P18-003`, the remaining Runtime Cloud subdomains,
  assertion reachability, mutation sensitivity, and the final broad remote
  gates remain globally open.

  Runtime Cloud proxy-planning coherent-batch progress (2026-07-22): the same
  product action contract now explicitly requires the control and I/O proxy
  planners to emit only valid action requests or reject before preflight and
  side effects. Expected-red unit tests exposed an explicitly blank control
  request ID being forwarded as the correlation ID, blank `connected_via`
  values being emitted by both planners, and the I/O planner accepting malformed
  or breaking-major API versions. The owner-local fixes reject those inputs;
  the shipped HTTP adapters now return `400 contract_violation`, while valid
  remote control reads and I/O reads/writes remain green. Deterministic semantic
  automation binds all 9 live proxy-policy facts and all 15 focused tests to
  `RT_RUNTIME_CLOUD_PROXY_PLAN_001`; the generator scopes unit test IDs by owner
  module and the generic audit rejects duplicate generated IDs after its first
  pass detected a same-name collision. Remote focused proof passes all 66
  Runtime Cloud unit tests, the 1 control-proxy, 2 I/O-proxy, and 1 malformed-
  proxy HTTP cases, plus all 5 Runtime Cloud architecture assertions. Both
  policy files remain small single-responsibility request builders, routes stay
  policy-first adapters, no ownership or execution flow changed, and no diagram
  update is required. This is a truST product contract, not an IEC decision or
  deviation. Assertion reachability, mutation sensitivity, remaining Runtime
  Cloud subdomains, and the final broad remote gates remain open.

  Runtime Cloud desired-configuration coherent-batch progress (2026-07-22): a
  product specification now owns optimistic desired revision/ETag writes and
  immutable reconcile snapshots. Two expected-red state-machine tests exposed
  the same in-flight race on both completion outcomes: if desired revision N+1
  was accepted while N was applying, N success copied N+1 content into
  `reported` under revision N and marked it in sync, while N failure changed
  N+1 to an ineligible error state. The policy callback now receives the exact
  apply snapshot. Success reports only that snapshot; success or failure leaves
  a superseding revision pending and eligible for the next reconcile, with a
  failure diagnostic retained. Generated semantic registration binds all 11
  config-policy facts plus the single web adapter boundary and all 10 focused
  unit, HTTP, and architecture tests to
  `RT_RUNTIME_CLOUD_CONFIG_RECONCILE_001`. Remote focused proof passes 3 policy
  tests, 1 persisted-state test, all 6 config HTTP tests, and all 5 Runtime Cloud
  architecture assertions. The web module remains an adapter to the existing
  policy owner, no ownership or execution flow changed, both files remain below
  the large-file threshold, and no diagram update is required. Rollout policy
  was inspected but its multi-target authority remains insufficient for new
  assertions, so no rollout behavior was invented or claimed. This product
  contract is not an IEC decision or deviation. Assertion reachability,
  mutation sensitivity, rollout and remaining Runtime Cloud subdomains, and the
  final broad remote gates remain open.

  Runtime Cloud schema/keyspace coherent-batch progress (2026-07-22): the
  product specification now owns API-version compatibility, forward-additive
  schema layout, canonical key zones/config aliases, active-authority
  publication, UI/operations stale timeout, and retained-value safety. The
  expected-red schema matrix proved that duplicate field names, a new field
  overlapping the existing layout, overlapping appended fields, zero-sized
  ranges, and overflowing ranges were all reported compatible. The contracts
  owner now validates nonempty unique names, checked nonzero ranges, global
  non-overlap, stable previous fields, and true append-only placement. A
  behavior-lock matrix covers every runtime and active zone prefix without
  claiming that the key constructors validate arbitrary external segments;
  their contract begins with already validated identity segments. Generated
  semantic registration binds all 50 live contracts/keyspace facts and all 15
  focused unit and architecture tests to
  `RT_RUNTIME_CLOUD_SCHEMA_KEYSPACE_001`. Remote focused proof passes 5 contract
  tests, 9 keyspace tests, and all 5 Runtime Cloud architecture assertions. The
  shape validator remains inside the contract owner, both modules remain below
  the large-file threshold and transport-independent, no ownership or execution
  flow changed, and no diagram update is required. This is a truST product
  contract, not an IEC decision or deviation. Assertion reachability, mutation
  sensitivity, rollout and remaining Runtime Cloud subdomains, and the final
  broad remote gates remain open.

  Runtime Cloud topology/link-policy coherent-batch progress (2026-07-22): the
  product specification now distinguishes discovery-derived lifecycle truth
  from measured latency/loss and defines order-independent same-host evidence
  for the same-host-only T0 overlay. Expected-red tests proved that healthy
  discovery edges published invented `12 ms`/`0%` metrics and that a loopback
  address on one peer classified an unrelated peer pair as same-host. The
  projection now leaves unmeasured metrics absent; ordinary address matching
  ignores loopback, while a loopback peer is accepted only in a relation with
  the dashboard's known local runtime. Host-group authority, shared non-loopback
  addresses, local-loopback T0 acceptance, remote T0 rejection, mesh-edge
  retention, alternate transport projection, persistence, and audit remain
  covered. Generated semantic registration binds all 49 live projection,
  link-policy, state-adapter, and route-adapter facts plus 25 focused unit and
  HTTP tests to `RT_RUNTIME_CLOUD_TOPOLOGY_LINK_POLICY_001`; its scoped audit
  also proves that two architecture tests already owned by earlier Runtime
  Cloud invariants retained their existing catalog and invariant bindings.
  The post-batch planner exposed noncanonical taxonomy emitted by the previous
  config/schema generators; automation normalized their case-family, coverage,
  status-delta, and test-class vocabulary without changing test identities or
  proof ownership. Planner and staleness validation now reach only the two
  already deferred retain/restart mutation-provenance mismatches; no digest was
  edited manually.
  Remote focused proof passes all 89 Runtime Cloud unit tests, both
  link-transport HTTP tests, and all 5 Runtime Cloud architecture assertions.
  The transport-free policy owners and thin web adapters retain their existing
  responsibilities, every touched Rust file remains below the large-file
  threshold, no ownership or execution flow changed, and no diagram update is
  required. This is a truST product contract, not an IEC decision or deviation.
  Assertion reachability, mutation sensitivity, rollout and remaining Runtime
  Cloud subdomains, and the final broad remote gates remain open.

  Runtime Cloud HA fencing/replay coherent-batch progress (2026-07-22): the
  product specification now owns complete-group split-brain detection,
  lease-owner/role/fence authority, monotonic completed-command replay, live
  dispatch ownership, and recovered-pending reconciliation. Expected-red unit
  and HTTP tests proved that two valid ACTIVE owners were missed whenever the
  action targeted only one candidate, allowing the protected preflight, and
  that a duplicate request could receive a second live `Proceed` while its
  first dispatch remained pending. The HA request now evaluates every supplied
  group target, and the coordinator keeps non-serialized live ownership that
  rejects concurrent duplicates while permitting one same-ID/same-sequence
  resume after state recovery. Generated semantic registration binds all 23 HA
  owner facts plus the single HA preflight-adapter fact and all 13 focused unit
  and HTTP tests to `RT_RUNTIME_CLOUD_HA_FENCING_001`. Remote focused proof
  passes all 8 HA unit tests, all 5 HA HTTP tests, and all 5 Runtime Cloud
  architecture assertions. The complete HA policy remains transport-free; the
  broad mixed-responsibility action-dispatch route remains honestly unmapped
  rather than being overclaimed as purely HA. All touched Rust files remain
  below the large-file threshold, no ownership or execution flow changed, and
  no diagram update is required. This is a truST product safety contract, not
  an IEC decision or deviation. Assertion reachability, mutation sensitivity,
  rollout and the final broad remote gates remain open.

  ADS onboarding identity coherent-batch progress (2026-07-22): the runtime
  product specification now requires valid canonical runtime-host, target, and
  AMS identities before route, Doctor, or symbol-import authority exists, and
  keeps public/NAT-suspect identities ineligible for automatic route creation.
  Five expected-red tests proved that noncanonical IPv4 text produced an AMS
  identity, blank or malformed explicit AMS overrides were accepted or
  silently treated as omission, malformed runtime-host sources became valid
  when an override was present, malformed targets became identity authority,
  and valid equivalent AMS octets retained noncanonical text. The identity
  owner now parses and canonicalizes IP and six-octet AMS values before
  constructing `LocalIdentity`. Remote focused proof passes all 5 new
  validation tests and all 29 existing ADS onboarding tests. Deterministic
  semantic automation binds all 14 live identity-owner facts and 10 exact
  identity/classification tests to `RT_ADS_ONBOARDING_IDENTITY_001`; the scoped
  audit reports 605 specified code facts, 14,669 remaining `spec_gap` facts,
  605 mapped denominator tests, and 3,548 remaining `reviewed_nonmapping`
  tests. The owner remains transport-independent, no runtime ownership or
  execution flow changed, and no diagram update is required. This is a truST
  product contract outside IEC 61131-3, not an IEC decision or deviation.
  Assertion reachability, mutation sensitivity, remaining ADS onboarding
  subdomains, and the final broad remote gates remain open.

  ADS onboarding discovery coherent-batch progress (2026-07-22): the runtime
  product specification now defines manual, directed-identify, and optional
  directed-broadcast discovery, partial-success isolation, usable interface
  candidates, and ADS endpoint uniqueness by `(AMS Net ID, AMS port)` rather
  than host IP. The expected-red regression proved that discovery dropped a
  second valid PLC runtime whenever it shared the first runtime's host IP. The
  discovery owner now preserves distinct same-host AMS identities and ports
  while deduplicating repeated observations of one endpoint. A focused
  behavior lock also proves that a failed broadcast cannot erase an already
  valid manual identity. Remote focused proof passes the exact regression,
  the broadcast-failure lock, and all 29 existing ADS onboarding tests.
  Deterministic semantic automation binds all 10 live discovery-owner facts
  and 6 exact discovery tests to `RT_ADS_ONBOARDING_DISCOVERY_001`; the scoped
  audit reports 615 specified code facts, 14,659 remaining `spec_gap` facts,
  611 mapped denominator tests, and 3,544 remaining `reviewed_nonmapping`
  tests. The wire boundary and existing ownership remain unchanged, every
  touched Rust file remains below the large-file threshold, and no diagram
  update is required. This is a truST product contract outside IEC 61131-3,
  not an IEC decision or deviation. Assertion reachability, mutation
  sensitivity, remaining ADS onboarding subdomains, and the final broad remote
  gates remain open.

  ADS onboarding route-security coherent-batch progress (2026-07-22): the
  runtime product specification now owns credential-channel eligibility,
  nonblank one-shot credentials, credential-free deterministic artifacts, XML
  escaping, PowerShell data/code separation, safe filenames, exact-name route
  replacement/removal, backup and encoding reporting, and client/server role
  guidance. Two expected-red tests proved that blank usernames/passwords
  reached the automatic route wire and that a crafted route name could
  terminate the generated raw PowerShell XML here-string. Trusted eligible
  route addition now rejects blank credentials before the wire, and generated
  PowerShell carries escaped UTF-8 route XML as base64 data before decoding it
  for `DocumentFragment.InnerXml`. The injection regression decodes the payload
  and proves hostile text remains escaped XML. Remote focused proof passes both
  new security tests and all 29 existing ADS onboarding tests. Deterministic
  semantic automation binds all 21 live route-owner facts and 9 exact
  route/security tests to `RT_ADS_ONBOARDING_ROUTE_SECURITY_001`; the scoped
  audit reports 636 specified code facts, 14,638 remaining `spec_gap` facts,
  620 mapped denominator tests, and 3,537 remaining `reviewed_nonmapping`
  tests. The route owner and wire boundary retain their responsibilities, the
  Rust file remains below the large-file threshold, and no diagram update is
  required. This is a truST product security contract outside IEC 61131-3, not
  an IEC decision or deviation. Assertion reachability, mutation sensitivity,
  remaining ADS onboarding subdomains, and the final broad remote gates remain
  open.

  ADS symbol-import artifact-safety coherent-batch progress (2026-07-22): the
  runtime product specification now confines server-control import outputs to
  distinct relative paths under the selected project, rejects path syntax in
  connection-derived snapshot names and existing symlink components, requires
  snapshots to remain discoverable in the canonical cache, and protects
  generated ST before any sibling artifact write. Expected-red tests proved
  the path escapes, collisions, undiscoverable snapshot state, unsafe overwrite,
  and legitimate follow-up-regeneration failures. The apply boundary now
  validates existing generated ST against the current ADS configuration and
  cached snapshots before mutation, so an authoritative prior import can be
  updated while drifted or operator-edited source remains protected. Remote
  focused proof covers the complete server-control and core import filters.
  Deterministic semantic automation owns the exact code/test bindings and the
  generated invariant record. The artifact policy remains in its focused
  module; no broader ownership or execution flow changed, and no diagram update
  is required. This is a truST product security contract outside IEC 61131-3,
  not an IEC decision or deviation. Assertion reachability, mutation
  sensitivity, remaining ADS symbol-import semantics, and the final broad
  remote gates remain open.

  ADS symbol-import selection coherent-batch progress (2026-07-22): the runtime
  product specification now defines exact-list precedence, ASCII-insensitive
  `*` selection, empty-pattern behavior, deterministic collision suffixes, and
  generated identifiers that remain valid under IEC 61131-3:2013 6.1.2,
  Table 2, and 6.1.3. Three expected-red tests proved that an empty include
  pattern selected every uploaded symbol, an anchored glob stopped at an
  earlier suffix fragment instead of backtracking, and valid ST keywords not
  present in a copied partial list became generated variable names. The
  selection owner now uses a complete star matcher and the canonical ST lexer
  for keyword truth. Remote focused proof passes all 4 response-selection tests
  and all 8 core symbol-import tests. Deterministic semantic automation binds
  12 exact selection/name facts and 4 focused tests to
  `RT_ADS_SYMBOL_IMPORT_SELECTION_001`; the scoped audit reports 656 specified
  code facts, 14,623 remaining `spec_gap` facts, 632 mapped denominator tests,
  and 3,535 remaining `reviewed_nonmapping` tests. The import owner is 897
  lines, reuses the existing syntax dependency instead of duplicating keyword
  ownership, and needs no diagram update. This is a truST selection contract
  aligned with IEC identifier syntax, not an IEC decision or deviation.
  Assertion reachability, mutation sensitivity, remaining ADS import apply
  semantics, and the final broad remote gates remain open.

  ADS symbol-import apply/merge coherent-batch progress (2026-07-22): the
  runtime product specification now requires strict current-configuration
  parsing, exact connection replacement with deterministic preservation of
  other connections, canonical snapshot merging, globally collision-free
  generated names, faithful scalar/string/array/update-mode projection, and
  capability-faithful write acknowledgement. Expected-red tests exposed and
  fixed three product bugs: valid follow-up imports were blocked, snapshots
  could be persisted where future imports would not discover them, and an
  acknowledged Write-only symbol acquired a Read capability. Focused
  behavior-lock tests also cover malformed current TOML, STRING and array
  projection, notification mode, and every supported scalar type. The core
  apply tests were split into a focused test module so the production owner
  remains below the large-file guardrail. Deterministic semantic automation
  owns the exact code/test bindings, invariant synchronization, and live
  accounting; the combined scoped audit passes. This is a truST ADS product
  contract, not an IEC decision or deviation. Assertion reachability, mutation
  sensitivity, the remaining ADS subdomains, and the final broad remote gates
  remain open.

  ADS server command-dispatch coherent-batch progress (2026-07-22): the runtime
  product specification now defines policy-before-dispatch authorization,
  strict request framing, direct/by-name/by-handle and upload wire behavior,
  non-aliasing bounded handles, capability-gated audited writes, complete
  aggregate-write preflight, and notification registration/deletion safety.
  Three expected-red tests exposed and fixed two malformed sum-up paths that
  queued an earlier runtime write before detecting a truncated later item and
  notification-handle wrap that replaced a live registration. Additional
  focused behavior locks cover policy denial, `ReadState`, unsupported control
  commands, malformed request forms, group-versus-offset errors, missing
  notification receivers, and invalid notification deletion. Remote focused
  proof passes the complete command suite. The aggregate command owner and its
  tests were split from oversized files while leaving `CommandDispatcher` as
  the component owner, so no architecture flow or diagram claim changed.
  Deterministic semantic automation owns the dispatcher/request/aggregate/wire
  code bindings, all command-test identities, invariant synchronization, and
  live accounting; the scoped audit passes. This is a truST ADS protocol
  contract, not an IEC decision or deviation. Assertion reachability, mutation
  sensitivity, external-client/TwinCAT proof, the remaining ADS subdomains,
  and the final broad remote gates remain open.

  ADS diagnostic-report coherent-batch progress (2026-07-22): the runtime
  product specification now defines the version-two secret-free report schema,
  fail-closed overall aggregation, role-shaped production evidence, and current
  deployment/status/freshness requirements. Four expected-red tests exposed
  and fixed empty reports and non-blocking failed steps being promoted to
  `pass`, declared evidence age and live-status binding being ignored, and
  incomplete or client-shaped TwinCAT evidence granting server production
  readiness. The diagnostics tests were split by readiness and schema concern
  before expansion; systematic wire-name, deterministic hash, serialization
  failure, clock, expiry-overflow, and legacy-evidence behavior locks now keep
  every focused module below the large-file guardrail. Remote focused proof
  passes the complete diagnostic suite and the complete ADS library slice, and
  the shipped runtime binary compiles. Deterministic semantic automation owns
  the exact report/evidence code bindings, moved and new test identities,
  invariant synchronization, and live accounting; the scoped audit passes.
  No production ownership or execution flow changed, so no architecture
  diagram update is required. This is a truST ADS product contract outside IEC
  61131-3, not an IEC decision or deviation. Assertion reachability, mutation
  sensitivity, independent external-client/TwinCAT lab proof, the remaining
  ADS subdomains, and the final broad remote gates remain open.

  ADS onboarding live-wire coherent-batch progress (2026-07-22): the runtime
  product specification now defines the raw-ADS boundary, endpoint identity,
  target/local-scoped connection cache, fail-closed symbol/value handling, and
  guarded-write recovery contract. Four expected-red assertions exposed and
  fixed the real UDP path dropping distinct runtimes that shared one IP and
  guarded probe write/read-back failures returning without a verified restore
  or discarding the restore failure. Once a probe write is attempted, the wire
  now attempts and verifies the original value on every failure path and keeps
  both errors when recovery also fails. The deterministic mock was mechanically
  split from the production owner before expansion, leaving focused production,
  mock, and regression-test modules below the large-file guardrail. Remote
  focused proof passes the complete wire regressions and the complete onboarding
  slice. Deterministic semantic automation owns the live code identities,
  moved mock facts, selected existing tests, new regression tests, invariant,
  and accounting; the scoped audit passes. This internal module split changes
  no ownership or execution flow and requires no diagram update. It is a truST
  ADS product contract outside IEC 61131-3, not an IEC decision or deviation.
  Assertion reachability, mutation sensitivity, real TwinCAT value/type and
  recovery proof, the remaining ADS subdomains, and the final broad remote
  gates remain open.

  ADS onboarding Doctor coherent-batch progress (2026-07-22): the runtime
  product specification now owns ordered fail-closed Doctor orchestration,
  manual target validation, explicit symbol authority, active-device endpoint
  matching, and live point-health truth. Four expected-red assertions exposed
  and fixed malformed manual endpoints reaching the transport, blank or absent
  selected symbols being accepted, unrelated active-device snapshots
  substituting for the requested endpoint, and empty or timestamp-less live
  point sets being reported as a passing batch read. Job orchestration, target
  validation, and Doctor tests were mechanically separated before expansion;
  each resulting module remains below the large-file guardrail. Remote focused
  proof passes all 15 Doctor tests, all 56 onboarding tests without the live
  wire feature, all 61 onboarding tests with `ads-wire`, and the runtime library
  compile check. Deterministic semantic automation owns the live code facts,
  moved and new test identities, invariant, risk acknowledgements, and
  accounting; the scoped audit passes. The internal module split changes no
  ownership, data flow, or execution flow and requires no diagram update. This
  is a truST ADS product contract outside IEC 61131-3, not an IEC decision or
  deviation. Assertion reachability, mutation sensitivity, real TwinCAT proof,
  the remaining ADS subdomains, and the final broad remote gates remain open.
  <!-- generated:ads-client-transport:start -->
  ADS client transport coherent-batch progress (2026-07-22): the runtime
  product specification now defines canonical target and optional local AMS
  identity authority, non-zero target ports, plain-only security, explicit
  route ownership, local lifecycle cleanup, device-state mapping, supported
  symbol projection, exact handle sizes, sum-up position truth, notification
  decoding, and opaque symbol-version equality. Strengthened expected-red
  tables proved that shortened target and local AMS identities reached the
  connector under the old backend semantics; the target table also locks zero
  ports and other malformed canonical forms before connector or state cleanup.
  A private hermetic connector seam now proves both rejection and the positive
  canonical path without TCP or fixed ports. Shared AMS identity parsing keeps
  the existing onboarding and Doctor specification owners, while the backend
  tests were mechanically split from the production owner. Remote focused
  proof passes all 9 backend tests (5 lifecycle/security
  and 4 data/value). Deterministic semantic automation binds
  51 live transport/shared-identity facts and 9 focused tests
  to `RT_ADS_CLIENT_TRANSPORT_LIFECYCLE_001` and
  `RT_ADS_CLIENT_TRANSPORT_DATA_001`; the scoped audit reports
  1,019 specified code facts,
  14,280 remaining `spec_gap` facts,
  760 catalog-mapped tests, and
  3,451 remaining
  `reviewed_nonmapping` tests. The shared identity ownership and connector
  flow are reflected in the ADS client architecture diagram and architecture
  checklist. This is a truST ADS product contract outside IEC 61131-3, not an
  IEC decision or deviation. Remaining operation-specific behavior locks,
  assertion reachability, mutation sensitivity, external ADS/TwinCAT proof,
  the remaining ADS subdomains, and the final broad remote gates remain open.
  <!-- generated:ads-client-transport:end -->
  <!-- generated:ads-client-configuration:start -->
  ADS client configuration coherent-batch progress (2026-07-23): the runtime product specification now owns unique connection names, canonical target and explicit-local AMS identities, complete connection and point shape, exact point address/type size agreement, bounded STRING and array metadata, security acknowledgement, and local runtime identity readiness. Focused expected-red regressions exposed and fixed duplicate connection names, noncanonical identities, zero STRING capacity, stray STRING capacity, mismatched index byte sizes, and overflowing array byte length; table-driven behavior locks cover the remaining parser partitions, and the accepted ADS behavior-lock suite remains green. Deterministic scanner-driven automation binds 21 live code facts and 20 focused tests to `RT_ADS_CLIENT_CONFIGURATION_001`; repository totals become 1,040 specified facts, 14,261 remaining `spec_gap` facts, 780 catalog-mapped tests, and 3,441 remaining `reviewed_nonmapping` tests. Secure ADS transport remains an unresolved product decision and has no test authority. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation. Assertion reachability, mutation sensitivity, activation/runtime lifecycle behavior, external ADS/TwinCAT proof, remaining ADS subdomains, and final broad gates remain open.
  <!-- generated:ads-client-configuration:end -->
  <!-- generated:ads-server-doctor-truth:start -->
  ADS server Doctor coherent-batch progress (2026-07-23): the runtime product specification now requires live served-symbol authority, nonblank external proof, nonzero handles, complete and unambiguous notification streams, and exact response direction, endpoint, command, and invoke correlation. Focused expected-red regressions exposed and fixed caller-snapshot false health, blank proof acceptance, zero handles, truncated or malformed notification acceptance, duplicate matching samples, trailing bytes, and uncorrelated response frames; the loopback protocol owner is split from Doctor orchestration and accepted focused behavior locks remain green. Deterministic scanner-driven automation binds 48 live code facts and 15 focused tests to `RT_ADS_SERVER_DOCTOR_TRUTH_001`; repository totals become 1,088 specified facts, 14,217 remaining `spec_gap` facts, 795 catalog-mapped tests, and 3,438 remaining `reviewed_nonmapping` tests. Production readiness still requires TwinCAT evidence. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation. Assertion reachability, mutation sensitivity, broad remote gates, real external-client/TwinCAT proof, remaining ADS subdomains, and final full gates remain open.
  <!-- generated:ads-server-doctor-truth:end -->
  <!-- generated:ads-active-device-route-matching:start -->
  ADS active-device route-matching coherent-batch progress (2026-07-23): the runtime product specification now requires an IP-literal configured route to match the requested host IP and AMS port together, plus the supplied expected target AMS identity and both-present local identity, before reusing live ADS state. Focused expected-red regressions proved that the previous OR matcher accepted partial endpoints and allowed an earlier partial route to shadow a later exact route; a direct local-identity behavior lock, the minimal conjunction fix, and the positive lock are green. Deterministic scanner-driven automation binds 2 live code facts and 3 focused tests to `RT_ADS_ACTIVE_DEVICE_ROUTE_MATCHING_001`; repository totals become 1,090 specified facts, 14,215 remaining `spec_gap` facts, 798 catalog-mapped tests, and 3,437 remaining `reviewed_nonmapping` tests. Hostname resolution, omitted-local-identity semantics, multiple-exact-match ambiguity, device-state operational policy, and duplicate direct-start behavior remain unresolved product decisions with no invented tests. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation. Assertion reachability, mutation sensitivity, broad remote gates, remaining ADS subdomains, and final full gates remain open.
  <!-- generated:ads-active-device-route-matching:end -->
  <!-- generated:ads-client-worker-cache-integrity:start -->
  ADS client worker/cache integrity coherent-batch progress (2026-07-23): the runtime product specification now owns exact handle, poll, subscription, notification, and symbol-token correlation; generation-safe pending-write truth; finite egress admission; deterministic reconnect cleanup; and interruptible idle shutdown. Focused expected-red regressions exposed and fixed partial response acceptance, stale symbol authority, false write status and lost newer intents, non-finite egress queueing, and full-interval shutdown delay; existing behavior locks preserve scan-boundary, descriptor, notification, reconnect, and read/write contracts. Deterministic scanner-driven automation binds 149 live code facts and 27 focused tests to `RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001`; repository totals become 1,239 specified facts, 14,088 remaining `spec_gap` facts, 825 catalog-mapped tests, and 3,422 remaining `reviewed_nonmapping` tests. Numeric zero by-name handle and subscription policy, duplicate valid notifications, pending-write retry/drop and reconnect replay, device-state policy, duplicate direct start, external ADS/TwinCAT proof, assertion reachability, mutation sensitivity, and final broad gates remain unresolved or open. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:ads-client-worker-cache-integrity:end -->
- [ ] `VERIF-P18-005` Eliminate all 3,777 `reviewed_nonmapping` test facts.
  After the owning `P18-003` authority is accepted, catalog-map each to that
  accepted invariant and an executable suite, bind governed gate/fuzz/device
  campaigns to equivalent executed proof contracts, or remove it with
  independently reviewed no-loss evidence. No nonmapping or inventory-only
  disposition is final.
- [ ] `VERIF-P18-006` Add every missing executable behavior test through the
  required red-green route after the owning `P18-003` authority is accepted.
  Cover success, boundary, malformed input, negative, failure mapping,
  recovery, restart/retain, authorization, safe state, and resource limits at
  the closest owning boundary. Compile/smoke/source-text/HTTP-status/
  snapshot-presence checks cannot satisfy behavior claims.
- [ ] `VERIF-P18-004` After the owning slice's `P18-005` and `P18-006` work is
  complete, validate the bidirectional specification -> invariant -> code ->
  test -> suite -> evidence chains. Enforce every structural disposition rule
  above and its negative fixtures. Broad file/area links, missing mapped tests,
  and unexecuted terminal tests fail; this row cannot close from a pre-test
  chain skeleton.
- [ ] `VERIF-P18-007` Prove dynamic reachability and assertion strength for
  every mapped fact. Record source-SHA-bound execution traces and adversarial
  catcher-removal or mutation evidence showing the assertion fails when its
  invariant is violated. Phase 17 proved semantic linkage, not dynamic
  reachability or mutation sensitivity; Phase 18 must measure both without
  relying on an unsupported historical count.

  Signed-integer reachability progress (2026-07-22): the report-only
  assertion-reachability substrate now derives all 265 current catalog-mapped
  scanner identities and has a reviewed first shard for both
  `signed_from_i128` tests. The shard pins the exact CODE/invariant/source/test
  joins and one cargo-mutants adversarial replacement; no test becomes proven
  until a clean source-SHA-bound baseline and selected-test failure are
  reconstructed from the raw artifact. Clean remote execution at
  `992e87348e91f7614e8276e078617de122413624` proves both mapped tests against
  the viable SINT-arm-deletion mutant: two baselines passed, two selected-test
  failures were caught, and the canonical 265-record artifact retains the
  other 263 records as unrun debt with zero survivors, unviable outcomes,
  timeouts, or harness failures. The committed artifact is
  `sha256:b352613b01f6376f71480334b376491e9506e1f47ac50d88bceae03ca4d0d9b9`
  with authenticated input closure
  `sha256:f82d6b74c987ea7d52f78adfb990cd4b1faa020364f0d12f10a85e0d5f40eb3c`.
  This is slice progress only:
  `VERIF-P18-007` remains globally open and formal mutation adequacy remains
  separately blocked behind `VERIF-P18-009` and its accepted budget.
- [ ] `VERIF-P18-008` Produce function, line, branch, and condition coverage for
  every behavior-bearing Rust, TypeScript/JavaScript, Python, and shell surface.
  Final unexplained uncovered behavior is zero. Generated, platform, and
  hardware-only branches remain in the denominator with exact owning lanes.
- [ ] `VERIF-P18-009` Run mutation adequacy to zero surviving viable mutants for
  every behavior-bearing unit. Prioritize wrong-result, silent-corruption,
  safety, authorization, protocol, compiler/HIR, bytecode/VM, runtime, and gate
  paths. Unviable/equivalent classifications need exact reviewed artifacts;
  timeouts, builds, unsupported tools, and skipped shards remain debt.
- [ ] `VERIF-P18-010` Complete property, fuzz, malformed-input, and differential
  campaigns under the preaccepted per-target budgets. Every crash, divergence,
  hang, and minimized artifact becomes a cataloged regression before closure;
  all fuzz targets retain exact direct-surface and canonical-owner mappings.
- [ ] `VERIF-P18-011` Prove concurrency, time, determinism, and lifecycle
  behavior for races, cancellation, deadlines, wall/monotonic/scaled clocks,
  scheduler ordering, watchdogs, retain/reload/restart, force/release, queue
  saturation, reconnect/backoff, and repeated execution using Loom, Miri,
  sanitizers, stress, or deterministic models as appropriate. Sleep/retry-only
  tests are not proof.
- [ ] `VERIF-P18-012` Close unsafe, FFI, memory, and external-boundary debt.
  Every unsafe/delegated-unsafe path needs an exact safety contract, caller
  obligations, negative tests, and applicable Miri, sanitizer, Valgrind, ABI,
  or hardware evidence. Panic, ignored error, fallback default, and broad
  suppression cannot silently produce healthy status.
- [ ] `VERIF-P18-013` Complete real hardware and endurance proof for every
  shipped claim under preaccepted topology and soak budgets: device-in-loop,
  disconnect/reconnect, malformed frames, safe outputs, watchdogs, repeated
  cold/warm restart, and endurance. Missing hardware, simulation-only,
  ignored/skipped/filtered tests, or unavailable runners remain blocked.
- [ ] `VERIF-P18-014` Complete rendered operator/editor workflow proof for every
  shipped VS Code, HMI, browser, diagnostics/completion, debug, live-values,
  devices/connections, import/export, alarm/trend, and recovery journey.
  Register automated acceptance plus real rendered screenshot/recording proof
  where visual behavior matters. DOM/source/HTTP-only evidence is insufficient.
- [ ] `VERIF-P18-015` Complete the claim-derived supported build/platform matrix:
  Linux and Windows authority lanes, supported architectures, default/nondefault
  features, release/debug, locale/path/encoding, packaging/install/upgrade, and
  every public/package/configuration claim. The matrix denominator is frozen.
  Scope retirement requires a reviewed product/compatibility decision,
  consistent removal from implementation/package/configuration/docs claims,
  changelog/version/deprecation handling, and independent release verification.
  Tool/runner unavailability alone cannot retire a claim.
- [ ] `VERIF-P18-016` After `P18-001..015` close, freeze one clean zero-debt
  candidate. Reproduce focused proof, run the strict P18 gate, final remote
  `fmt`/`clippy`/`test-all`, exact strict preflight, and the complete GitHub
  candidate matrix on that exact SHA. Any source/metadata/instruction/base or
  evidence-contract change invalidates the candidate.
- [ ] `VERIF-P18-017` Move only the exact accepted candidate SHA to `main`,
  create the annotated tag when release rules require it, and complete the
  applicable public release, asset, checksum, provenance, Marketplace, package,
  and public-claim verification chain without source or metadata changes.
- [ ] `VERIF-P18-018` Obtain a separate commit/tag-bound independent `CLEAR`
  verdict over all raw zero-debt rows and every focused, adequacy, environment,
  candidate, GitHub, hardware/UX, and public artifact. Only this row may close
  the final program and Phase 18 stop gates.

## Stop Gates

- [ ] `VERIF-STOP-018A` Do not describe the program, codebase, or tests as
  complete, fully specified/mapped/tested, production-proven, or zero-debt
  while any Phase 18 row or zero equation is open.
- [ ] `VERIF-STOP-018B` Do not accept `spec_gap`, `registered_backlog`,
  `reviewed_nonmapping`, uncataloged/unreviewed tests, ignored/skipped/filtered/
  unrun campaigns, zero-test filters, open gaps, or unexplained uncovered
  behavior as retired debt. Inventory ownership is not proof.
- [ ] `VERIF-STOP-018C` Do not substitute passing broad tests, structural
  scanners, coverage percentages, code-to-gap mappings, compile/smoke checks,
  or implementation-derived expectations for semantic oracle and assertion/
  mutation adequacy.
- [ ] `VERIF-STOP-018D` Do not create permanent waivers or lower denominators,
  floors, campaign budgets, mutation/coverage obligations, hardware/UX scope,
  or platform/claim matrices to get green. Environmental blockers keep exact
  rows open with owner, scope, evidence, and next action.
- [ ] `VERIF-STOP-018E` Do not close from implementation-side evidence or a
  pre-public candidate review. Final closure requires `P18-018` after every
  applicable evidence tier is complete.
- [ ] `VERIF-STOP-018F` Do not begin unrelated product-feature implementation
  while Phase 18 is open. Product code changes are allowed only when required
  by an active Phase 18 red-green repair slice and must satisfy full
  code-spec-test enforcement immediately.

## SOLID/KISS/DRY Acceptance

- [ ] Extraction, semantic joins, proof production, validation, report
  rendering, and CLI orchestration remain separate responsibilities.
- [ ] Each proof class has a focused module, schema, report, validator, and
  negative fixture; no per-slice module approaches the repository's large-file
  guardrail.
- [ ] Existing authoritative inventories are joined by identity, never copied
  into parallel lists.
- [ ] Common artifact/provenance primitives are shared without combining proof
  semantics across coverage, mutation, fuzz, hardware, UX, or release lanes.
- [ ] Diagrams and architecture checklists are refreshed whenever ownership or
  data flow changes, and code-derived facts remain authoritative.
