# Phase 18 Zero-Debt Execution Board

Status: zero-debt contract accepted by commit-bound independent R3 review.
Findings `P18-C01` through `P18-C08` and `P18-R2-01` are resolved.

Current row: `VERIF-P18-003`, continuing as the repository-wide authority and
coverage campaign over 13,718 explicitly mapped `spec_gap` code facts with
zero code-spec findings. The code map now represents all 17,491 live facts, of
which 3,513 are `specified`. The test denominator is 8,695 = 4,462
`catalog_mapped` + 4,233 `reviewed_nonmapping`; `VERIF-P18-005` therefore
remains open. The accepted P18-002 ledger owns and orders every current fact;
it does not close any remaining specification, execution, reachability,
mutation, or adequacy population.

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

  Verification authority-tool progress (2026-08-13): fourteen exact CLI and
  focused authority/review/selection/transaction source groups are bound to
  `SPEC_VERIFICATION_PROGRAM_CONTRACT_001#code-to-spec-refresh-proposal-quarantine`,
  `VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001`, and
  `TEST_VERIFICATION_METADATA_GATE_001`. A complete independently audited
  38-group review accepts only those 14 singular groups, retains all 24 prior
  withholds, and atomically promotes exactly 115 code facts. The final report
  at `07e58204` accounts 17,491 facts with zero findings, 13,720 `spec_gap`,
  and 3,511 `specified`; the focused ownership suite passes 65/65 and diagram
  drift is zero on `trust-builder`. Execution reachability, mutation,
  adequacy, and broad candidate proof remain open; no P18-016 broad gate ran.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-13-p18-authority-tool-contract.md`.

  Metadata-integrity entrypoint progress (2026-08-13): an expected focused red
  proved that the document-wide metadata-integrity oracle could not enter the
  exact-fragment authority-reuse queue. Binding the existing
  `#verification-metadata-model` contract made the focused test and full reuse
  suite pass 11/11. A complete 26-group review retains 24 prior withholds and
  atomically promotes only the exact Python validator and shell gate
  entrypoints to `VERIFICATION_METADATA_INTEGRITY_001` and
  `TEST_VERIFICATION_METADATA_GATE_001`. The final report at `066619f0`
  accounts 17,491 facts with zero findings, 13,718 `spec_gap`, and 3,513
  `specified`. This closes authority only; the invariant's producer-authentic
  lock and final remote-gate debt remain open, and no P18-016 broad gate ran.

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
  ADS client worker/cache integrity coherent-batch progress (2026-07-23): the runtime product specification now owns exact handle, poll, subscription, notification, and symbol-token correlation; generation-safe pending-write truth; finite egress admission; deterministic reconnect cleanup; and interruptible idle shutdown. Focused expected-red regressions exposed and fixed partial response acceptance, stale symbol authority, false write status and lost newer intents, non-finite egress queueing, and full-interval shutdown delay; existing behavior locks preserve scan-boundary, descriptor, notification, reconnect, and read/write contracts. Deterministic scanner-driven automation binds 97 live code facts and 27 focused tests to `RT_ADS_CLIENT_WORKER_CACHE_INTEGRITY_001`; repository totals become 1,390 specified facts, 13,946 remaining `spec_gap` facts, 871 catalog-mapped tests, and 3,400 remaining `reviewed_nonmapping` tests. Numeric zero by-name handle and subscription policy, duplicate valid notifications, pending-write reconnect replay, device-state policy, duplicate direct start, external ADS/TwinCAT proof, assertion reachability, mutation sensitivity, and final broad gates remain unresolved or open. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:ads-client-worker-cache-integrity:end -->
  <!-- generated:ads-server-publication-lifecycle:start -->
  ADS server publication/lifecycle coherent-batch progress (2026-07-23): automated test discovery and the runtime product specification now cover bounded disabled defaults, deterministic symbol selection, supported scalar publication, exact namespace/glob behavior, fail-closed publisher errors, live client status, and TCP/UDP shutdown ownership. Focused regressions exposed and fixed insertion-order-dependent symbol truncation. Deterministic scanner-driven automation binds 33 live code facts and 15 focused tests to `RT_ADS_SERVER_PUBLICATION_LIFECYCLE_001`; repository totals become 1,272 specified facts, 14,055 remaining `spec_gap` facts, 840 catalog-mapped tests, and 3,417 remaining `reviewed_nonmapping` tests. STRING and array descriptor policy, snapshot freshness authority, atomic symbol/version refresh, external ADS/TwinCAT proof, assertion reachability, mutation sensitivity, and final broad gates remain unresolved or open. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:ads-server-publication-lifecycle:end -->
  <!-- generated:ads-server-policy-write-audit:start -->
  ADS server policy/write/audit coherent-batch progress (2026-07-23): automated test discovery and the runtime product specification now cover parsed exact IP/CIDR pins, explicit unpinned opt-in, bounded refusal evidence, security-first write admission, resource-state and malformed-value rejection, mutation-free failures, and stable audit projection. Focused regressions exposed and fixed textual-IP and unpinned-policy authorization bugs. Deterministic scanner-driven automation binds 31 live code facts and 15 focused tests to `RT_ADS_SERVER_POLICY_WRITE_AUDIT_001`; repository totals become 1,303 specified facts, 14,024 remaining `spec_gap` facts, 855 catalog-mapped tests, and 3,409 remaining `reviewed_nonmapping` tests. The existing non-finite server-ingress test retains its runtime-safety ownership; external ADS/TwinCAT proof, assertion reachability, mutation sensitivity, and final broad gates remain open. This is a truST ADS product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:ads-server-policy-write-audit:end -->
  <!-- generated:opcua-client-worker-cache-integrity:start -->
  OPC UA client worker/cache integrity coherent-batch progress (2026-07-23): automated test discovery and the runtime product specification now cover exact configured sample identity, current session and monotonic event authority, generation-safe output completion, finite scalar egress, exact write-response cardinality, and prompt authority-revoking shutdown. Focused expected-red regressions exposed and fixed dynamic sample acceptance, stale callback and older-value regression, lost newer writes, non-finite output queueing, short write-result acceptance, and full-interval shutdown delay; existing persistent session, subscription, stale/reconnect, and write behavior locks remain green. Deterministic scanner-driven automation binds 87 live code facts and 16 focused tests to `RT_OPCUA_CLIENT_WORKER_CACHE_INTEGRITY_001`; repository totals become 1,390 specified facts, 13,946 remaining `spec_gap` facts, 871 catalog-mapped tests, and 3,400 remaining `reviewed_nonmapping` tests. Pending-output replay, server source-timestamp ordering, external OPC UA proof, assertion reachability, mutation sensitivity, and final broad gates remain open. This is a truST OPC UA product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:opcua-client-worker-cache-integrity:end -->
  <!-- generated:opcua-client-config-trust-integrity:start -->
  OPC UA client configuration/trust coherent-batch progress (2026-07-23): the runtime product specification now owns unique connection identity, unambiguous auth/access parsing, PKI-root containment, an exact one-shot service-result cardinality guard, and stable recovery classification. Focused expected-red regressions exposed and fixed duplicate connection acceptance, nested/root/target symbolic-link certificate escape, and short/extra cardinality-guard acceptance; selected security-spelling, trust, promotion, and runtime-binding locks remain green. Deterministic scanner-driven automation binds 23 live code facts and 12 focused tests to `RT_OPCUA_CLIENT_CONFIG_TRUST_INTEGRITY_001`; repository totals become 1,413 specified facts, 13,927 remaining `spec_gap` facts, 883 catalog-mapped tests, and 3,395 remaining `reviewed_nonmapping` tests. Node-ID grammar without the wire feature, certificate destination filename collisions, concurrent filesystem mutation, multi-certificate promotion atomicity, exhaustive parser/PKI/classifier partitions, live discovery/browse, external OPC UA proof, cardinality call-site reachability, assertion reachability, mutation sensitivity, and final broad gates remain open. This is a truST OPC UA product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:opcua-client-config-trust-integrity:end -->
  <!-- generated:mqtt-default-fault-latest-value-integrity:start -->
  MQTT latest-value/default-fault coherent-batch progress (2026-07-23): the runtime product specification now requires newest raw payload authority and fail-closed default-fault projection for stale input, output deadlines, and completed ordinary or Sparkplug typed-output preflight. Four expected-red regressions exposed and fixed oldest-payload selection and three degraded-success paths; a focused Sparkplug preflight behavior lock closes the affected side path. Deterministic scanner-driven automation binds 3 live code facts and 15 focused tests to `RT_MQTT_DEFAULT_FAULT_LATEST_VALUE_001`; repository totals become 1,416 specified facts, 13,924 remaining `spec_gap` facts, 898 catalog-mapped tests, and 3,385 remaining `reviewed_nonmapping` tests. Broad driver/worker facts, reconnect replay, queue overflow, Sparkplug/session semantics, external broker proof, assertion reachability, mutation sensitivity, and final broad gates remain open. This is a truST MQTT product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:mqtt-default-fault-latest-value-integrity:end -->
  <!-- generated:modbus-declared-f32-finiteness:start -->
  Modbus declared-F32 finiteness coherent-batch progress (2026-07-23): the runtime product specification now requires post-scaling and inverse-scaling narrowing to remain finite at the declared F32 width. Two expected-red helper regressions exposed and fixed process-image and wire infinity synthesis; those two helper tests, two driver call-site locks, and all 25 Modbus integration tests are green. Deterministic scanner-driven automation binds 1 live code fact and 4 focused tests to `RT_MODBUS_DECLARED_F32_FINITE_001`; repository totals become 1,417 specified facts, 13,924 remaining `spec_gap` facts, 902 catalog-mapped tests, and 3,385 remaining `reviewed_nonmapping` tests. Mixed-branch encoder facts, integer rounding, multi-point output atomicity and partial publication, external Modbus proof, assertion reachability, mutation sensitivity, and final broad gates remain open. This is a truST Modbus product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:modbus-declared-f32-finiteness:end -->
  <!-- generated:direct-address-parser-core:start -->
  Direct-address parser coherent-batch progress (2026-07-23): IEC 61131-3 Ed.3 sections 6.5.5.1, 6.5.5.2, and 6.5.5.4 now govern runtime area, size, hierarchy, decimal-component, and exact unsized-wildcard parsing. Two expected-red matrices exposed early-return acceptance of trailing, sized, and spaced wildcard text plus signed numeric components; the focused parser fix now requires exact wildcard syntax and digit-only components. Deterministic scanner-driven automation binds 3 live code facts and 5 focused tests to `RT_DIRECT_ADDRESS_PARSER_001`; repository totals become 1,420 specified facts, 13,921 remaining `spec_gap` facts, 909 catalog-mapped tests, and 3,383 remaining `reviewed_nonmapping` tests. Wildcard declaration-context/VAR_CONFIG resolution, deep-hierarchy resource stress, downstream interface/coercion semantics, IoAddress and IoSize type-wide facts, assertion reachability, mutation sensitivity, and final broad gates remain open. This is IEC-conforming runtime behavior, not an IEC decision or deviation.
  <!-- generated:direct-address-parser-core:end -->
  <!-- generated:modbus-worker-deadline-policy:start -->
  Modbus worker deadline-policy coherent-batch progress (2026-07-23): the runtime product specification now requires scan-deadline input and output handling to preserve fault, warn, and ignore policy. Two expected-red decision matrices exposed cached-input false success and a hard-coded degraded-success output deadline; the focused worker fix preserves caller bytes for faulted stale input and leaves timed-out asynchronous output queued. Deterministic scanner-driven automation binds 5 live code facts and 5 focused tests to `RT_MODBUS_WORKER_DEADLINE_POLICY_001`; repository totals become 1,425 specified facts, 13,916 remaining `spec_gap` facts, 914 catalog-mapped tests, and 3,380 remaining `reviewed_nonmapping` tests. Broad worker facts, completed transport failures, snapshot shaping, output replacement/correlation and reconnect backoff, sequence wrap, worker shutdown, concurrency stress, external Modbus proof, assertion reachability, mutation sensitivity, and final broad gates remain open. This is a truST Modbus product contract outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:modbus-worker-deadline-policy:end -->
  <!-- generated:offline-comm-authoring-cli:start -->
  Offline communication authoring coherent-batch progress (2026-07-23): automated test discovery found two existing CLI integration tests covering every action family; the runtime product specification now owns exact offline routing, local input rejection, trust-store handling, and default-template validity. Focused proof exposed and fixed an empty generated mesh token that made a fresh project fail current configuration validation, and a new negative matrix closes malformed JSON and snapshot-file gaps. Deterministic scanner-driven automation binds 4 live code facts and 3 focused tests to `RT_OFFLINE_COMM_AUTHORING_CLI_001`; repository totals become 1,429 specified facts, 13,912 remaining `spec_gap` facts, 917 catalog-mapped tests, and 3,380 remaining `reviewed_nonmapping` tests. External protocol discovery, assertion reachability, mutation sensitivity, and final broad gates remain open. This is truST host-product behavior outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:offline-comm-authoring-cli:end -->
  <!-- generated:standalone-ide-bootstrap:start -->
  Standalone IDE bootstrap coherent-batch progress (2026-07-23): automated discovery found three focused unit tests and one existing CLI integration lock for primary-runtime selection, deterministic recursive sources, local nonexecuting control state, and pre-bind configuration failure. The runtime product specification now owns that bootstrap plus the deprecated alias. A focused expected-red regression exposed and fixed acceptance of an ordinary file as a project path, and a new alias-ordering lock closes the remaining entrypoint branch. Deterministic scanner-driven automation binds 5 live code facts and 6 focused tests to `RT_STANDALONE_IDE_BOOTSTRAP_001`; repository totals become 1,434 specified facts, 13,907 remaining `spec_gap` facts, 922 catalog-mapped tests, and 3,380 remaining `reviewed_nonmapping` tests. Rendered Browser IDE proof, source-discovery I/O fault injection, assertion reachability, mutation sensitivity, and final broad gates remain open. This is truST host-product behavior outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:standalone-ide-bootstrap:end -->
  <!-- generated:runtime-conformance-runner:start -->
  Runtime conformance-runner coherent-batch progress (2026-07-23): automatic catalogue discovery found 10 pre-existing runner unit tests, one CLI parser test, one CLI integration test, and 21 already catalog-mapped executable case manifests. The missing product specification now owns discovery, case kinds, lifecycle, typed values, artifact modes, summary profiles, and failure taxonomy. Four compact focused matrices close the proven discovery-identity, series/restart, typed-value, and connector-token partitions; 14 runner unit tests, the CLI integration, and all 21 shipped cases pass. Deterministic scanner-driven automation binds 41 live code facts and 16 focused tests to `RT_CONFORMANCE_RUNNER_INTEGRITY_001`; repository totals become 1,475 specified facts, 13,866 remaining `spec_gap` facts, 938 catalog-mapped tests, and 3,373 remaining `reviewed_nonmapping` tests. No production bug was exposed in this subsystem. Assertion reachability, mutation sensitivity, cross-platform filesystem proof, and final broad gates remain open. These are truST product rules outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:runtime-conformance-runner:end -->
  <!-- generated:runtime-control-client:start -->
  Runtime control-client coherent-batch progress (2026-07-23): automatic catalogue discovery found two request/exchange unit tests and four TCP CLI integration tests. The product specification already owned endpoint and credential precedence, canonical request shape, protocol failure, and response status; missing scalar, exact rendering, response-correlation, and resource-bound rules are now specified. Six compact focused tests close scalar/result, target-precedence, project-load, Unix transport, and task-rendering partitions. Expected-red regressions exposed and fixed acceptance of missing or mismatched response IDs and malformed ok=true action results. All five unit tests and all seven CLI integration tests pass. Deterministic scanner-driven automation binds 9 live code facts and 12 focused tests to `RT_CONTROL_CLI_CLIENT_001`; repository totals become 1,484 specified facts, 13,858 remaining `spec_gap` facts, 950 catalog-mapped tests, and 3,373 remaining `reviewed_nonmapping` tests. The response-ID false-success bug is fixed. Windows TCP, connect/write timeout fault injection, assertion reachability, mutation sensitivity, and final broad gates remain open. Focused red/green evidence is recorded in `docs/internal/testing/evidence/plc-verification-program/2026-07-23-p18-control-client.md`. This is truST product behavior outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:runtime-control-client:end -->
  <!-- generated:runtime-versioned-deploy:start -->
  Versioned deployment coherent-batch progress (2026-07-23): automatic catalogue discovery found ten unit and six integration tests. The product specification already owned validation, labels, pointer containment, rollback, pruning, and deterministic summaries; missing pointer-slot preflight, protocol-sidecar closure, and symlink rules are now specified. Seven compact regressions close those gaps, and all eleven unit plus twelve integration tests pass. Deterministic scanner-driven automation binds 32 live code facts and 23 focused tests to `RT_VERSIONED_DEPLOY_STATE_001`; repository totals become 1,516 specified facts, 13,830 remaining `spec_gap` facts, 973 catalog-mapped tests, and 3,373 remaining `reviewed_nonmapping` tests. Expected-red proof exposed partial pointer/summary state, missing enabled sidecars, and followed symlinks; all are fixed. Windows symlink variation, filesystem fault injection, assertion reachability, mutation sensitivity, and final broad gates remain open. Focused evidence is recorded in `docs/internal/testing/evidence/plc-verification-program/2026-07-23-p18-versioned-deploy.md`. This is truST product behavior outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:runtime-versioned-deploy:end -->
  <!-- generated:runtime-managed-fleet:start -->
  Managed-fleet coherent-batch progress (2026-07-23): the automatic catalogue found nineteen existing focused tests. The existing product specification already owned scaffolding, validation, credential, control, and lifecycle behavior; two missing path and spawn-order details are now specified without an IEC entry. Eleven compact assertions close the concrete uncovered clauses. Deterministic scanner-driven automation binds 41 live code facts and 30 focused tests to `RT_MANAGED_FLEET_LIFECYCLE_001`; repository totals become 1,557 specified facts, 13,795 remaining `spec_gap` facts, 1,003 catalog-mapped tests, and 3,364 remaining `reviewed_nonmapping` tests. Expected-red proof exposed one real bug: whitespace-padded runtime names were silently normalized and accepted; validation now rejects them before filesystem mutation. Twelve fleet units, fourteen lifecycle units, three CLI parser tests, and one shipped lifecycle integration are green. Assertion reachability, mutation sensitivity, Windows process variation, and final broad gates remain open. This is truST product behavior outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:runtime-managed-fleet:end -->
  <!-- generated:runtime-hmi-scaffold-command:start -->
  HMI scaffold-command coherent-batch progress (2026-07-23): the automatic catalogue found two shipped CLI integrations and three parser tests. The product specification now owns project selection, compile-before-mutation, mode lifecycle, style, and truthful summary behavior. Deterministic scanner-driven automation binds 5 live code facts and 9 focused tests to `RT_HMI_SCAFFOLD_COMMAND_001`; repository totals become 1,562 specified facts, 13,789 remaining `spec_gap` facts, 1,012 catalog-mapped tests, and 3,361 remaining `reviewed_nonmapping` tests. The live scanner removes one stale deleted `load_sources` fact instead of inventing authority for it. HMI scaffolding is truST product behavior outside IEC 61131-3, not an IEC decision or deviation. Focused remote command and parser proof is green; assertion reachability, mutation sensitivity, and final broad gates remain open.
  <!-- generated:runtime-hmi-scaffold-command:end -->
  <!-- generated:runtime-numeric-prompt:start -->
  Numeric-prompt coherent-batch progress (2026-07-23): automatic test discovery found two existing parser behavior locks. The existing product specification already owns full-range unsigned parsing, labeled rejection, and empty-submission default behavior. Deterministic scanner-driven automation binds 2 live code facts and 3 focused tests to `RT_NUMERIC_PROMPT_001`; repository totals become 1,564 specified facts, 13,788 remaining `spec_gap` facts, 1,015 catalog-mapped tests, and 3,361 remaining `reviewed_nonmapping` tests. These setup and wizard prompt rules are truST product behavior outside IEC 61131-3, not an IEC decision or deviation. Focused remote numeric-prompt proof is green; assertion reachability, mutation sensitivity, and final broad gates remain open.
  <!-- generated:runtime-numeric-prompt:end -->
  <!-- generated:runtime-registry-project-resolution:start -->
  Registry project-resolution coherent-batch progress (2026-07-23): automatic test discovery found direct explicit-path and detection-failure coverage. The existing product specification already owns optional-project resolution, so no new specification was needed. Deterministic scanner-driven automation binds 1 live code fact and 4 focused tests to `RT_REGISTRY_PROJECT_RESOLUTION_001`; repository totals become 1,565 specified facts, 13,787 remaining `spec_gap` facts, 1,019 catalog-mapped tests, and 3,360 remaining `reviewed_nonmapping` tests. Registry project selection is truST product behavior outside IEC 61131-3, not an IEC decision or deviation. Focused remote registry project-resolution proof is green; assertion reachability, mutation sensitivity, and final broad gates remain open.
  <!-- generated:runtime-registry-project-resolution:end -->
  <!-- generated:runtime-play-launch:start -->
  Play-launch coherent-batch progress (2026-07-23): automatic test discovery found one direct existing pre-mutation behavior lock and rejected parser and lexical false positives. Product specifications now define explicit completeness and the narrow omitted-project first-run exception. Deterministic scanner-driven automation binds 1 live code fact and 1 focused tests to `RT_PLAY_LAUNCH_001`; repository totals become 1,566 specified facts, 13,786 remaining `spec_gap` facts, 1,019 catalog-mapped tests, and 3,360 remaining `reviewed_nonmapping` tests. Play launch and bundle selection are truST host/product behavior outside IEC 61131-3, not an IEC decision or deviation.
  <!-- generated:runtime-play-launch:end -->
  <!-- generated:hir-assignments-and-variable-semantics:start -->
  HIR assignment and variable-semantics coherent-batch progress
  (2026-07-25; updated 2026-07-26):
  automatic discovery found 69 existing focused tests. Saved per-test execution
  evidence, reviewed code-spec authority, and exact assertion-to-behavior
  review catalog-map all 69 tests across 11 HIR invariants; all 69 high-risk
  rows have two independent reviewers, 80 oracle-bound assertions, six setup-only
  assertions, and no unclassified assertion. The two former
  primary-diagnostic-only cases now assert both the evaluator's primary
  diagnostic and suppression of the dependent bounds/range diagnostic. The
  focused run exposed that the former subrange fixture could succeed through
  ordinary undefined-name resolution; the typed divide-by-zero fixture now
  proves the intended constant-evaluation path. IEC 61131-3 Ed.3
  §§6.4.4.5.1 and 6.6.1.2.4 close the enum-valued array-bound authority gap.
  The remaining subrange case exposed a compiler false positive: a
  nonconstant index was rejected merely because its declared subrange partly
  exceeded the array bounds. The focused red/green fix now reserves static
  rejection for constant out-of-range values and leaves actual computed-index
  enforcement to the existing runtime bounds check. This is IEC-conforming
  behavior, so no IEC decision or deviation entry was added.
  Deterministic atomic generation raises repository totals to 1,632 specified
  code facts, leaves 13,753 `spec_gap` facts, raises the denominator to 1,142
  `catalog_mapped` tests, and leaves 3,244 `reviewed_nonmapping` tests. The
  first atomic apply exposed and fixed a focused-review bug where the prebuilt
  assertion extractor was still sent schema v1 after the extractor contract
  advanced to v2; the existing focused transaction test now locks the shared
  schema constant. The strengthened two-test shard, its 2/2 focused trace, and
  three focused red/green regression checks are green; the final two-test
  trace observed 2/2 tests and the reviewed transaction accepted both rows
  atomically. The preceding
  1,360-case focused verification-tool suite remains green; a redundant
  umbrella repeat for this two-row metadata follow-up was stopped after five
  minutes without failure and will not be repeated.
  No broad workspace test, coverage campaign, mutation campaign, or final gate
  has run. Assertion reachability, mutation sensitivity, the final frozen
  source-closure and broad-report refresh, remaining semantic domains, and
  final broad gates remain open.
  <!-- generated:hir-assignments-and-variable-semantics:end -->
  <!-- generated:runtime-string-element-semantics:start -->
  Runtime string-element coherent-batch progress (2026-07-26):
  automatic discovery selected six existing focused runtime and runtime-core
  tests. A product specification and `VM_SEAM_STRING_ELEMENT_001` now own the
  Unicode-scalar count, position, narrow/wide index, replacement-shape,
  destination-width, and reference-path behavior. Focused red/green proof
  exposed and fixed a real width-check bypass: a one-scalar `STRING` or
  `WSTRING` replacement previously returned early and could place a scalar
  outside the destination `CHAR` or `WCHAR` width. The corrected conversion
  applies destination width and Unicode-scalar validation uniformly.
  Deterministic numbered authority and assertion review binds 12 live
  `bytecode_vm` facts and catalog-maps all six tests with two independent
  reviewers. Two covered `runtime_safety` storage helpers remain honest
  `spec_gap` rows instead of being forced across the area boundary. Repository
  totals become 1,644 specified code facts, 13,741 remaining `spec_gap` facts,
  1,148 `catalog_mapped` denominator tests, and 3,238 remaining
  `reviewed_nonmapping` tests. The focused trace observed 6/6 tests in 18
  seconds and 80 focused mapping/authority tests are green. Lower- and
  upper-index boundary behaviors still require dedicated cataloged tests; no
  coverage cell or proof level was promoted from trace evidence alone. The
  contract uses the existing genuine `DEV-017` deviation and adds no unrelated
  IEC decision or deviation. No broad workspace test, coverage campaign,
  mutation campaign, or final gate has run.
  <!-- generated:runtime-string-element-semantics:end -->
  <!-- generated:parser-bounded-recovery-catalog:start -->

  Parser bounded-recovery catalog progress (2026-07-26): the product
  recovery contract now states how nested parentheses, brackets, separators,
  owning closers, and statement boundaries interact in a bounded scan. The
  policy remains a truST tooling decision recorded in `IEC_DECISIONS.md`, not
  an IEC deviation. A separate `IEC_PARSE_BOUNDED_SCAN_001` invariant prevents
  five internal helper tests from overstating parser-level diagnostic proof;
  two parser-level tests bind only to the existing missing-semicolon and
  missing-inner-terminator partitions of `IEC_PARSE_RECOVER_001`.
  Execution-derived attribution and two independent semantic reviewers bind
  the exact two helper code facts and catalog-map all seven tests. Repository
  totals become 1,646 specified code facts, 13,739 remaining `spec_gap` facts,
  1,155 `catalog_mapped` denominator tests, and 3,231 remaining
  `reviewed_nonmapping` tests; the catalog contains 1,163 rows including the
  eight rows outside the frozen denominator. The focused trace observed 7/7
  tests in 18 seconds. Its five-test and two-test review scopes reused that
  saved raw evidence without rerunning tests, and 80 focused
  mapping/authority self-tests are green. Valid-source parser smoke tests and
  unrelated colocated runtime/ADS tests remain honest nonmapping debt rather
  than being forced into semantically different invariants. No broad
  workspace test, coverage campaign, mutation campaign, or final gate has run.
  <!-- generated:parser-bounded-recovery-catalog:end -->
  <!-- generated:runtime-host-observability:start -->

  Host-runtime observability catalog progress (2026-07-28): the runtime
  product specification and decision record now own bounded execution-backend
  selection and construction, historian recording and export, and runtime
  metrics/profiling behavior. A fail-closed per-slice generator binds the
  exact fifteen live tests and seventy-five assertions to
  `RT_EXECUTION_BACKEND_SELECTION_001`,
  `RT_HISTORIAN_RECORDING_001`, and
  `RT_RUNTIME_METRICS_PROFILING_001`: 64 assertions are direct product oracles,
  eleven are reviewed setup or preconditions, and none is unclassified or
  non-oracle. The former no-assertion `loads_runtime` smoke test now locks the
  documented fresh runtime state. The focused builder trace observed 15/15
  selected tests, and all four catalog transactions passed dry-run and atomic
  apply. The frozen metadata report now records 4,393 live tests, 3,732
  `catalog_mapped`, 661 `reviewed_nonmapping`, 154 missing-specification
  authority tests, 507 separately owned nonmapping tests, and zero confirmed
  missing tests. This is truST host-product behavior outside IEC 61131-3, not
  an IEC decision or deviation. `VERIF-P18-003`, assertion reachability,
  mutation sensitivity, remaining semantic domains, and the final broad gates
  remain open. Focused evidence is recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-host-runtime-observability.md`.
  <!-- generated:runtime-host-observability:end -->
  <!-- generated:runtime-boundary-fail-closed:start -->

  Runtime boundary fail-closed catalog progress (2026-07-28): twelve existing
  resolver, harness, initialization, debug-write, and reference-helper tests
  now terminate in five existing runtime and VM invariants. IEC 61131-3 Ed.3
  §6.6.6.5.1 supplies the normative initial-NULL authority for the
  interface-typed variable; candidate reporting, stable harness errors,
  fallback-creation policy, queued-debug fault visibility, and helper
  representation remain explicit truST product contracts, not IEC decisions
  or deviations. A fail-closed per-slice generator binds all 45 assertions:
  36 are direct oracles, nine are setup or preconditions, and none is
  unclassified or non-oracle. The focused builder trace observed 12/12
  selected tests, and all four catalog transactions passed dry-run and atomic
  apply. The frozen metadata report now records 4,393 live tests, 3,744
  `catalog_mapped`, 649 `reviewed_nonmapping`, 142 missing-specification
  authority tests, 507 separately owned nonmapping tests, zero confirmed
  missing tests, and 207 specified gap-open cells. `VERIF-P18-003`, the
  repository-wide planner metadata baseline, assertion reachability, mutation
  sensitivity, remaining semantic domains, and the final broad gates remain
  open. Focused evidence is recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-runtime-boundary-fail-closed.md`.
  <!-- generated:runtime-boundary-fail-closed:end -->
  <!-- generated:remaining-authority-142:start -->

  Remaining-authority coherent-campaign progress (2026-07-28): the complete
  frozen `no_reviewed_spec_or_invariant_binding` population was processed as
  one 142-test, 92-path transaction. Fail-closed assertion review binds 139
  tests and 1,079 extracted assertions to nine bounded invariants; three
  dynamic external-oracle tests are honestly withheld as
  `assertion_oracle_unresolved` rather than falsely mapped. The complete
  builder campaign observed 221/221 selected and companion tests passing with
  zero failures or pending results, and the 90 mapping plans plus three
  classification changes passed dry run and atomic apply. The frozen test
  equations now report 4,393 live tests, 3,883 `catalog_mapped`, 510
  `reviewed_nonmapping`, zero missing-specification-authority tests, and 510
  separately owned nonmapping tests. Removing the authority stop gate makes
  the previously blocked behavior-demand calculation honest: 422 confirmed
  missing tests and 31 exact-behavior-mapping debts are now visible. This is
  progress across `P18-003`, `P18-005`, and the entry condition for `P18-006`;
  none of those rows closes globally. The aggregate metadata/planner baseline,
  code specification gaps, owned nonmapping terminal population, new missing
  tests, evidence registration, reachability, mutation sensitivity, and final
  broad gates remain open. No broad workspace gate ran. Focused evidence is
  recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-remaining-authority-142.md`.
  <!-- generated:remaining-authority-142:end -->
  <!-- generated:case-backed-behavior-101:start -->

  Case-backed exact-behavior mapping progress (2026-07-28): the 422 unbound
  specified behavior slots were first joined to current case tables, existing
  mapped runners, and retained per-case execution evidence instead of being
  treated immediately as 422 new tests. A fail-closed transaction accepts 101
  behavior references across 37 invariants and 37 existing runners only where
  the current invariant contract digest, case-file digest, exact
  partition/expectation fields, and passing evidence for all 105 assigned
  cases agree. Fifteen stale-contract case tables and one unmatched rename
  behavior remain open. All 72 affected current-contract proof digests,
  including four legacy execution-contract-v2 red/green rows without the newer
  explicit binding field, were rebound without changing historical bindings or
  execution results. A targeted validator comparison returned to the exact
  1,291-finding pre-transaction baseline with zero selected-proof findings.
  Local and
  builder focused suites pass 11/11, the three-file recoverable transaction is
  byte-identical across checkouts, and frozen behavior demand falls from 422
  to 321 while test-denominator equations remain 3,883 mapped, 510 owned
  nonmapping, and zero missing authority. This is `P18-004`/`P18-005` mapping
  progress using existing proof, not 101 newly written tests or global row
  closure. The remaining 321 slots still require existing-proof refinement
  before the genuine `P18-006` test-writing residue is known. No broad
  workspace gate ran. Focused evidence is recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-case-backed-behavior-mapping.md`.
  <!-- generated:case-backed-behavior-101:end -->
  <!-- generated:stale-case-authority-refresh-54:start -->

  Stale case-authority coherent-campaign progress (2026-07-28): all fifteen
  stale case tables were reconstructed from current invariant authority.
  Thirteen tables remain exact one-to-one behavior partitions and now bind 54
  previously open behavior references through 59 exact case IDs, 13 existing
  runners, and 82 passing cases. Six generator-owned tables are byte-equal to
  fresh output; seven reviewed state-machine tables retain exact current
  semantics. `RT_SAFE_STOP_001` and `VM_SEAM_VALID_001` remain honestly
  rejected because their case scenarios do not equal current behavior
  partitions. The transaction retires 26 superseded current proof records and
  installs 13 candidate-SHA-bound current records. Because the twelve changed
  runner sources also affect 46 accepted generated-test mappings, all 46
  unchanged assertion inventories were re-extracted on the builder and their
  content-addressed proposal snapshots, accepted decisions, and catalog source
  closures were atomically rebound; canonical decision validation reports
  zero failures. The unified builder campaign passes 13/13 runners and 82/82
  cases, while 23 focused transaction and equation tests pass locally. Frozen
  behavior demand falls from 321 to 267 with zero missing authority. This is
  further `P18-004`/`P18-005` existing-proof refinement, not 54 new tests or
  global row closure. No broad workspace gate ran. Focused evidence is
  recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-stale-case-authority-refresh-54.md`.
  <!-- generated:stale-case-authority-refresh-54:end -->
  <!-- generated:ads-exact-behavior-71:start -->

  ADS exact-behavior coherent-campaign progress (2026-07-28): all 71 frozen
  ADS behavior demands are now bound to 145 exact catalog tests across 24
  source paths and 20 accepted invariants. Seventy demands reuse existing
  reviewed tests through invariant-local exact-name assignments; the sole
  genuine residue,
  `RT_ADS_ONBOARDING_WIRE_SAFETY_001#behavior:4`, received a new failing
  regression. Clean builder commit
  `f86c117b5219e3a0409df1113c028ce3233f6ab2` failed by assertion because an
  empty sum-up result manufactured a successful wire projection. Descendant
  commit `1ed0f2c1b255c1608c92d2c54505c2a440a08eac` requires a complete
  one-to-one result set before projecting any value and passes the unified
  145/145-test campaign in six exact default, no-default, ads-wire-only,
  ADS-server, and CLI shards. The new scanner identity is cataloged and
  denominator-mapped, the user-visible fix is recorded for `0.24.59`, and the
  frozen equations now report 4,394 live tests, 3,884 `catalog_mapped`, 510
  owned `reviewed_nonmapping`, zero missing authority, and 196 confirmed
  missing behavior tests. This is large-batch `P18-004`/`P18-005` progress
  plus one honest `P18-006` red-green repair; none of those rows closes
  globally. The repository metadata baseline, the remaining behavior domains,
  bidirectional terminal-evidence validation, reachability, mutation
  sensitivity, and final broad gates remain open. No broad workspace gate
  ran. Focused evidence and raw shard results are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-ads-exact-behavior-71.md`.
  <!-- generated:ads-exact-behavior-71:end -->
  <!-- generated:runtime-other-exact-behavior-106:start -->

  Runtime-other exact-behavior coherent-campaign progress (2026-07-28): all
  106 frozen behavior demands in the runtime-other authority group are bound
  to 196 exact selected catalog tests and 230 assignment links across 27
  command groups. The clean builder campaign at
  `1dc849081acc06da759b56888930a36b3c6eeee2` passes 196/196 tests; the
  additional denominator-missing breakpoint suppression test passes 1/1 and
  is bound to `DEBUG_CONTROL_EXECUTION_001#behavior:2`. The atomic metadata
  transaction at `d7df94e2de0df6dc7b50d4b0fad1f55080561f99` records 34 new
  exact mappings, preserves 163 existing mappings, adds five live denominator
  facts, refreshes 12 accepted proposal/decision snapshots and seven changed
  assertion-source closures, and updates 15 provenance records. The frozen
  equations now report 4,399 live tests, 3,918 `catalog_mapped`, 481 owned
  `reviewed_nonmapping`, zero existing tests lacking specification authority,
  and 90 confirmed missing behavior tests. The focused mapping, proof, and
  campaign suite passes 10/10 locally and on the exact clean builder checkout.
  The repository-wide staleness validator remains red on the open Phase 18
  metadata/proof baseline. This is large-batch `P18-003` through `P18-006`
  progress, not global row closure. No broad workspace gate ran. Focused
  evidence and retained machine-readable artifacts are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-28-p18-runtime-other-exact-behavior-106.md`.
  <!-- generated:runtime-other-exact-behavior-106:end -->
  <!-- generated:runtime-cloud-exact-behavior-23:start -->

  Runtime-cloud exact-behavior coherent-campaign progress (2026-07-29): all 23
  frozen managed-fleet and runtime-cloud behavior demands are bound to 52
  exact existing catalog tests and 53 assignment links across seven
  invariants. The clean builder campaign at
  `dce893b021f95d2156ce882c1d43c9c0a19ca7df` passes 52/52 tests in four
  exact `cargo nextest` groups. Its 52 assertion inventories contain 439
  extracted assertions and at least one direct reviewed oracle for every
  selected test. Forty-four unresolved helper-call annotations remain
  explicit downstream reachability and mutation-strength debt. The atomic
  metadata transaction at
  `ae57d2c3c397fa22af6076d813f9d8e463ad73fd` adds exact behavior references
  without changing production code, tests, specifications, or the frozen
  denominator. The equations remain 4,399 live tests, 3,918
  `catalog_mapped`, 481 owned `reviewed_nonmapping`, and zero existing tests
  lacking authority, while confirmed missing behavior tests fall from 90 to
  67. The focused proof, campaign, assertion, and transaction suite passes
  19/19 locally and on the exact clean builder checkout. This is large-batch
  `P18-003` through `P18-006` progress, not global row closure or `P18-007`
  proof. No broad workspace gate ran. Focused evidence and retained
  machine-readable artifacts are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-runtime-cloud-exact-behavior-23.md`.
  <!-- generated:runtime-cloud-exact-behavior-23:end -->
  <!-- generated:protocol-exact-behavior-29:start -->

  Protocol exact-behavior coherent-campaign progress (2026-07-29): all 29
  frozen MQTT, OPC UA, and Modbus behavior demands are bound to 32 exact tests
  and 35 assignment links across seven invariants. The accepted clean builder
  campaign at `8f3139a0a424ba7e89fa95c868113daaa7883898` passes 32/32
  tests in four exact `cargo nextest` groups. A focused Modbus reconnect
  assertion first failed honestly with `attempts=1`; the corrected stimulus
  crosses the worker's 100 ms backoff and proves the next connection attempt
  without production changes. Three OPC UA wire tests are promoted only after
  dynamic fixture reads were replaced by digest-bound `include_str!` inputs;
  their accepted assertion inventories have zero unresolved helpers. The
  atomic metadata transaction at
  `0c129691cd22d830e537d5c1f2e25ad3396f98d7` promotes those three tests,
  refreshes the Modbus mapping decision, and passes the complete focused suite
  42/42 locally and on the exact clean builder checkout. The frozen equations
  remain 4,399 live tests with zero authority debt, while mapped tests rise
  from 3,918 to 3,921, reviewed nonmapping falls from 481 to 478, and confirmed
  missing behavior tests fall from 67 to 38. Four helper annotations on
  already-cataloged selected tests remain explicit `P18-007` debt. This is
  large-batch `P18-003` through `P18-006` progress, not global row closure.
  No broad workspace gate ran. Focused evidence and retained machine-readable
  artifacts are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-protocol-exact-behavior-29.md`.
  <!-- generated:protocol-exact-behavior-29:end -->
  <!-- generated:verification-owned-exact-behavior-11:start -->

  Verification-owned exact-behavior coherent-campaign progress (2026-07-29):
  all eleven remaining case-artifact-helper and IEC table-inventory behavior
  demands are bound to fifteen exact tests and seventeen behavior links across
  two invariants. The accepted clean builder campaign at
  `1338cd7a48e690c43b65c7239216f651a97178b3` passes 15/15 tests in two
  exact `cargo nextest` groups. A focused assertion-extractor test first
  failed honestly because generated file paths were lost across helper
  parameters and struct fields. The new fail-closed dataflow retains only
  generated paths, while external, reassigned, escaping, and unclosed fields
  remain unresolved. The IEC repository scan binds 1,571 exact Rust source
  digests through a canonical source-tree oracle; malformed roots are rejected.
  The fifteen accepted inventories contain 120 assertions, 54 direct oracles,
  and zero unresolved helpers. The atomic metadata transaction at
  `a124d727921776884fce10e78a62e00a12a365b2` promotes twelve tests and
  refreshes three existing mapping decisions. The frozen equations become
  4,401 live tests, 3,933 `catalog_mapped`, 468 `reviewed_nonmapping`, zero
  missing authority, and 27 confirmed missing behavior tests. The focused
  route, transaction, assertion-extractor, case-helper, and IEC inventory
  suites pass 52/52 on the exact clean builder checkout. This is large-batch
  `P18-003` through `P18-006` progress, not global row closure. No broad
  workspace gate ran. Focused evidence and retained machine-readable artifacts
  are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-verification-owned-exact-behavior-11.md`.
  <!-- generated:verification-owned-exact-behavior-11:end -->
  <!-- generated:residual-exact-behavior-27:start -->

  Residual exact-behavior coherent-campaign progress (2026-07-29): all 27
  remaining formally confirmed behavior demands are bound through 31 exact
  links to 26 tests across sixteen invariants. The accepted clean builder
  campaign at `4c35370499ff2423a22de327a67be444f5c5fdf5` passes 26/26
  tests in fifteen exact command groups. Its 26 fail-closed assertion
  inventories contain 282 extracted assertions and zero unresolved helpers.
  Nine live test facts are promoted and seventeen existing catalog mappings
  are reused. Expected-red HIR and runtime assertions exposed absent IEC Table
  46 `TP_TIME`, `TON_TIME`, and `TOF_TIME` support; the source repair at
  `68629aacfaa89a34de946e9f952adfa9fb3f6459` registers the explicit aliases
  with fixed `TIME` bindings, and the accepted HIR plus consolidated runtime
  tests pass. The atomic mapping transaction makes the test equations
  4,409 live = 3,942 `catalog_mapped` + 467 `reviewed_nonmapping`, with zero
  missing authority, zero confirmed missing behavior demands, and zero missing
  behavior candidates. The 467 owned nonmapping facts, 208 specified gap-open
  cells, nine current exact-behavior-marker debts, eleven empty historical
  placeholder invariants, repository metadata baseline, bidirectional chain,
  reachability, mutation sensitivity, and final broad gates remain open. This
  is large-batch `P18-003` through `P18-006` progress, not global row closure.
  No broad workspace gate ran. Focused evidence and retained machine-readable
  artifacts are recorded in
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-residual-exact-behavior-27.md`.
  <!-- generated:residual-exact-behavior-27:end -->
  <!-- generated:vscode-authority-40:start -->

  VS Code authority coherent-campaign progress (2026-07-29): a new opt-in
  exact Mocha selector preserves the unfiltered default and emits atomic,
  commit-bound per-test JSON evidence. Two focused expected-red assertions
  separately exposed the absent selector and absent evidence writer before
  their paired green implementation. A third expected-red assertion exposed
  that TypeScript assertion inventories omitted reachable same-file helpers;
  the cycle-safe unambiguous helper closure is now locked by eight focused
  extractor tests. On the manually bootstrapped clean builder checkout at
  `1c09656eed64d2b8a9800eb402de98b3399c9a72`, the harness contract passes
  1/1 and the real VS Code Extension Development Host executes exactly 39
  selected tests with 39 passes and zero failures. The 40 accepted inventories
  contain 186 assertions and zero unresolved helpers. Twenty-nine tests bind
  explicitly non-primary retained component locks, ten bind existing shipped
  VS Code product behaviors, and one binds the exact-execution harness. The
  clean scanner records 4,411 live facts with zero warnings. The atomic
  transaction makes the test equations 4,411 live = 3,983 `catalog_mapped` +
  428 `reviewed_nonmapping`, with zero missing authority and zero confirmed
  missing behavior tests. The 428 owned nonmapping facts, 208 specified
  gap-open cells, nine current exact-behavior-marker debts, eleven empty
  historical placeholder invariants, bidirectional reachability, mutation
  sensitivity, and final broad gates remain open. This is large-batch
  `P18-003` through `P18-006` progress, not global row closure. No broad
  workspace or unfiltered VS Code gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-vscode-authority-40.md`.
  <!-- generated:vscode-authority-40:end -->
  <!-- generated:rust-assertion-helper-closure:start -->

  Rust assertion-helper closure progress (2026-07-29): one test-first
  verification-tooling repair now resolves the owning Cargo target, complete
  `cfg(test)` unit-module graph, restricted visibility, repeated-super paths,
  transitive named/glob re-exports, and function-local imports while excluding
  ordinary production execution from delegated test proof. On the manually
  bootstrapped clean builder checkout at
  `84d6c31e8b930efc1b4c550e432dee74d4e612ae`, the combined assertion gate
  and module-graph suite pass 56/56 focused checks. Regeneration of the
  complete 336-test extractable product remainder raises bound assertions from
  2,905 to 3,525, reduces assertion-empty inventories from 20 to 11, and
  reduces 418 recovered unresolved helpers to 85 dynamic
  `unresolved-oracle` reads with zero unresolved function helpers. The frozen
  equations remain 4,411 live = 3,983 `catalog_mapped` + 428
  `reviewed_nonmapping`; no mapping or adequacy row is promoted by this
  structural repair. The next authority batch classifies the 85 reads as
  immutable repository inputs, closed generated paths, or genuine host/runtime
  observations. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-rust-assertion-helper-closure.md`.
  <!-- generated:rust-assertion-helper-closure:end -->
  <!-- generated:rust-assertion-oracle-closure-326:start -->

  Rust assertion-oracle and compile-negative coherent-batch progress
  (2026-07-29): the 85 dynamic oracle reads across 79 tests now bind immutable
  repository inputs or explicit runtime observations, and exact focused
  execution passes 79/79. The remaining 11 assertion-empty inventories were
  closed by explicit compile contracts or by converting compile-negative
  Structured Text declarations from test POUs to ordinary programs. An
  expected-red OSCAT assertion exposed that unfiltered `trust-dev test`
  returned false success without compiling a zero-test project; the runner now
  compiles before reporting an empty unfiltered result while preserving
  discovery-only `--list` and filter-empty behavior. The PLCopen mapped
  negative test now source-binds and executes all nine selected fixture
  projects, and the OPC UA subscription API contract exposes an explicit
  compile-time assertion. On the manually bootstrapped clean builder checkout
  at `63532855e2eaedb714469817f8411c56c2743676`, 21/21 runner unit tests,
  both mapped compile-negative tests, the OPC UA compile contract, a valid
  zero-test project compile, 45/45 assertion-inventory checks, and diagram
  drift pass. The complete remaining extractable population is 326 inventories
  with 3,539 assertions, zero unresolved helpers, and zero assertion-empty
  identities. The exact scanner records 4,401 live facts with zero warnings,
  making the unchanged mapping equation 4,401 live = 3,983
  `catalog_mapped` + 418 `reviewed_nonmapping`. The 245 denominator facts still
  carrying the historical `assertion_oracle_unresolved` reason remain open
  until semantic mapping transactions replace that reason; extractor closure
  is not mapping proof. The two changed mapped absence tests received fresh,
  hand-reviewed assertion provenance rather than a blind digest refresh, and
  their five missing-required invariant cells now record exact focused
  coverage. `VERIF-P18-005` and the global program rows remain open. No broad
  workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-rust-oracle-closure-326.md`.
  <!-- generated:rust-assertion-oracle-closure-326:end -->
  <!-- generated:openot-oracle-closure-38:start -->

  OpenOT assertion-oracle closure progress (2026-07-29): the exact 38-test
  OpenOT population now has an exhaustive 620-assertion review: 293 direct
  product-oracle assertions, 327 setup/preconditions, zero non-oracle
  observations, zero unresolved helpers, and zero assertion-empty identities.
  Exact focused execution on source checkpoint
  `02f5311d053ba5dc3a2686c161853218a492af0c` passes all 37 telemetry tests
  plus the fenced cross-process capstone, for 38/38. The atomic transaction
  refreshes 32 accepted mappings, adds six accepted mappings, closes all 14
  catalog-coverage cells across the five owning OpenOT invariants, and moves
  the exact denominator from 4,401 = 3,983 mapped + 418 nonmapping to 4,401 =
  3,989 mapped + 412 nonmapping. A validator-discovered historical overmapping
  was removed: the sampling-policy definition test now binds only authoring
  translation, while the separate typed-audit test retains event-resolution
  coverage. A root-record audit proves all 6,500 unrelated proposal/decision
  records byte-identical. `VERIF-P18-003` and `VERIF-P18-005` remain globally
  open; dynamic reachability, mutation sensitivity, and broad gates remain
  explicit downstream debt. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-openot-oracle-closure-38.md`.
  <!-- generated:openot-oracle-closure-38:end -->
  <!-- generated:vm-oracle-closure-49:start -->

  VM, bytecode, and SIZEOF assertion-oracle closure progress (2026-07-29):
  the exact 49-test population at `ad2d9952` passes 49/49 focused tests and has
  an exhaustive 396-assertion review: 162 direct product oracles, 234
  setup/preconditions, zero non-oracle observations, and 174 exact behavior
  bindings. Full staleness isolation rejected an initial weaker assignment
  that treated 25 opcode-presence assertions as setup and used a cross-area
  runtime-language authority. The accepted rebuilt transaction recognizes
  those assertions as product proof and binds opcode routing and canonical
  stack execution to the bytecode-native `VM_STACK_EXECUTION_001` invariant.
  The transaction adds 49 accepted mappings and moves the exact denominator
  from 4,401 = 3,989 mapped + 412 nonmapping to 4,401 = 4,038 mapped + 363
  nonmapping. A root-record audit proves all 6,576 pre-existing
  proposal/decision records byte-identical, the focused mapping suite passes
  72/72, and the final repository-wide staleness output contains zero findings
  tied to these 49 test identities or seven owning invariants.
  `VERIF-P18-003` and `VERIF-P18-005` remain globally open; dynamic
  reachability, mutation sensitivity, and broad gates remain explicit
  downstream debt. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-vm-oracle-closure-49.md`.
  <!-- generated:vm-oracle-closure-49:end -->
  <!-- generated:hmi-web-control-io-oracle-closure-77:start -->

  HMI, Web IDE, Runtime Cloud, control, and I/O assertion-oracle closure
  progress (2026-07-29): the exact 77-test population selected at `65652cfb`
  passes 77/77 focused tests and has an exhaustive 1,493-assertion review: 524
  direct product oracles, 969 setup/preconditions, zero non-oracle
  observations, and 524 exact behavior bindings. The operator-first HMI
  specification is now active normative authority; the unified Web IDE,
  configuration UI, bounded HMI persistence, Runtime Cloud
  state/dispatch/rollout, and audited WAN-allowlist update contracts are
  explicit product requirements. Six new invariants and the existing
  action-policy invariant add 27 coverage cells, all of which reconcile to
  accepted mappings, so repository-wide `gap_open` remains 203. Full
  identifier isolation rejected a cross-area connector-status draft; the
  accepted clean rebuild binds those four golden tests to the existing
  cross-surface `UI_STATUS_001` authority. The transaction adds 77 accepted
  mappings and moves the exact denominator from 4,401 = 4,038 mapped + 363
  nonmapping to 4,401 = 4,115 mapped + 286 nonmapping. A root-record audit
  proves all 6,674 pre-existing proposal/decision records byte-identical, the
  focused mapping suite passes 72/72, and the final repository-wide staleness
  output contains zero findings across 244 exact batch identifiers.
  `VERIF-P18-003` and `VERIF-P18-005` remain globally open; dynamic
  reachability, mutation sensitivity, rendered browser acceptance, and broad
  gates remain explicit downstream debt. No broad workspace gate ran.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-hmi-web-control-io-oracle-closure-77.md`.
  <!-- generated:hmi-web-control-io-oracle-closure-77:end -->
  <!-- generated:lsp-ide-oracle-closure-33:start -->

  LSP and IDE assertion-oracle closure progress (2026-07-29): the exact
  portable 33-test population at `55b79fa5` passes 33/33 focused tests and has
  an exhaustive 270-assertion review: 118 direct product oracles, 152
  setup/preconditions, zero non-oracle observations, and 118 exact behavior
  bindings. Seven new editor-safety invariants bind project configuration,
  document state, HMI authoring, cache identity, formatting/signature help,
  telemetry, and source refactors to the active LSP product contract. All new
  coverage cells reconcile to accepted mappings, so repository-wide
  `gap_open` remains 203. The transaction adds 33 accepted mappings and moves
  the exact denominator from 4,401 = 4,115 mapped + 286 nonmapping to 4,401 =
  4,148 mapped + 253 nonmapping. A root-record audit proves all 6,828
  pre-existing proposal/decision records byte-identical, the focused mapping
  suite passes 72/72, all 19 lock-bound digests match, and the final
  repository-wide staleness output contains zero findings across 106 exact
  batch identifiers. Two Windows-only URI tests remain explicit platform-lane
  debt because the Linux extractor resolves no assertion syntax for their
  gated bodies. `VERIF-P18-003` and `VERIF-P18-005` remain globally open;
  dynamic reachability, mutation sensitivity, platform proof, and broad gates
  remain explicit downstream debt. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-lsp-ide-oracle-closure-33.md`.
  <!-- generated:lsp-ide-oracle-closure-33:end -->
  <!-- generated:portable-assertion-oracle-closure-78:start -->

  Portable assertion-oracle closure progress (2026-07-29): the complete
  remaining portable 78-test population at `8a0a9fa2` passes 78/78 focused
  tests across 20 exact owning target groups and has an exhaustive
  865-assertion review: 402 direct product oracles, 463 setup/preconditions,
  zero non-oracle observations, and 403 exact behavior bindings. Twenty-one
  accepted invariants, including six new product authorities, bind runtime,
  PLCopen, developer workflow, platform, example, and supply-chain behavior.
  All new coverage cells reconcile to accepted mappings, so repository-wide
  `gap_open` remains 203. The transaction adds 78 accepted mappings and moves
  the exact denominator from 4,401 = 4,148 mapped + 253 nonmapping to 4,401 =
  4,226 mapped + 175 nonmapping. A root-record audit proves all 6,894
  pre-existing proposal/decision records byte-identical, the focused mapping
  suite passes 72/72, all 69 lock-bound digests match, and the final
  repository-wide staleness output contains zero findings across 156 exact
  batch identifiers. Only the two Windows-only URI tests remain
  `assertion_oracle_unresolved`; the other 173 denominator facts are governed
  nonmapping inventories. `VERIF-P18-003` and `VERIF-P18-005` remain globally
  open; dynamic reachability, mutation sensitivity, platform proof, and broad
  gates remain explicit downstream debt. No broad workspace gate ran.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-portable-oracle-closure-78.md`.
  <!-- generated:portable-assertion-oracle-closure-78:end -->
  <!-- generated:windows-uri-oracle-closure-2:start -->

  Windows URI assertion-oracle closure progress (2026-07-29): the final two
  `assertion_oracle_unresolved` discoveries were compiled for
  `x86_64-pc-windows-gnu` and pass 2/2 exact tests under Wine 10.0. A
  Windows-native extraction resolves all seven assertions as six direct
  product oracles and one setup/precondition, with six exact bindings to the
  accepted LSP URI/path authority and zero unresolved helpers. The platform
  lane exposed a cross-platform assertion-extractor defect; the new focused
  validator test has the required assertion red and paired green, and the
  verification-inventory specification now excludes POSIX, drive-rooted, and
  rooted/UNC host-runtime paths from repository fixture closure. The atomic
  transaction adds the two mappings and moves the exact denominator from
  4,401 = 4,226 mapped + 175 nonmapping to 4,401 = 4,228 mapped + 173
  nonmapping. The remaining population is entirely governed inventory: 83
  product non-oracles, 59 gate-owned facts, 23 ignored-test registry facts,
  and eight fuzz programs. A root-record audit proves all 7,050 pre-existing
  proposal/decision records byte-identical, the focused mapping suite passes
  72/72, the scoped assertion-inventory gate passes 8/8 Python and 38/38
  xtask tests, and staleness isolation finds zero findings for all four exact
  batch identifiers. `VERIF-P18-003` and `VERIF-P18-005` remain globally
  open; inventory closure, dynamic reachability, mutation sensitivity, and
  broad gates remain explicit downstream debt. No broad workspace gate ran.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-windows-uri-oracle-closure-2.md`.
  <!-- generated:windows-uri-oracle-closure-2:end -->
  <!-- generated:governed-gate-fuzz-closure-67:start -->

  Governed gate and fuzz structural mapping progress (2026-07-29): all 59
  `gate_inventory_owned` and eight `fuzz_program_owned` facts now have exact
  generated catalog bindings to explicit verification-governance authority.
  The mappings bind every discovery to its live inventory row, source digest,
  structural validation command, suite/enforcement posture, environment,
  artifact/corpus policy, production-surface association, and
  crash-regression handoff. Focused structural validation passes 36/36 gate
  and suite tests plus 36/36 fuzz program/source tests. A new mapping-review
  rule has the required assertion red and paired green and permits empty
  assertion sets only for gate scripts, GitHub workflow jobs, and fuzz targets
  using verification-owned `metadata_validation` contracts; Rust/ST/VS Code
  behavior mappings still require real oracle-bound assertions. The atomic
  transaction moves the exact denominator from 4,401 = 4,228 mapped + 173
  nonmapping to 4,401 = 4,295 mapped + 106 nonmapping. Only 83 product
  non-oracle/harness facts and 23 ignored-test facts remain. A root-record
  audit proves all 7,054 pre-existing proposal/decision records
  byte-identical, the focused mapping suite passes 73/73, and staleness
  isolation finds zero findings across all 134 new discovery/catalog
  identities. These mappings prove structural inventory and routing only;
  exact-SHA native GitHub job, gate, and fuzz campaign execution remains
  explicit downstream debt. `VERIF-P18-003` and `VERIF-P18-005` remain
  globally open; no broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-governed-gate-fuzz-closure-67.md`.
  <!-- generated:governed-gate-fuzz-closure-67:end -->
  <!-- generated:ignored-test-execution-closure-23:start -->

  Ignored-test execution-contract mapping progress (2026-07-29): all 23
  `ignored_test_registry_owned` facts now have exact generated catalog
  bindings to `VERIFICATION_IGNORED_TEST_EXECUTION_001`. The mappings preserve
  each live registry identity, class, reason, unblock condition, native
  environment or topology, and retained-artifact boundary. They deliberately
  prove only structural registry identity and routing. The extraction review
  records 161 product assertions and eight unresolved helper observations,
  claims none of them as product proof, and retains all 23 native executions
  as downstream debt: 15 performance/soak, five hardware-lab, and three
  manual/parent-harness tests. Focused registry/source/schema validation passes
  17/17. A mapping-review rule has the required assertion red and paired green
  and permits ignored/conditional Rust tests to use this verification-owned
  metadata contract only when every observed assertion is classified as
  non-oracle/setup and the product assertions remain explicitly unproved;
  ordinary Rust product mappings still require real oracle-bound assertions
  and reject unresolved helpers. The atomic transaction moves the exact
  denominator from 4,401 = 4,295 mapped + 106 nonmapping to 4,401 = 4,318
  mapped + 83 nonmapping. All remaining facts are
  `not_catalogable_product_oracle`. A root-record audit proves all 7,188
  pre-existing proposal/decision records byte-identical, the focused mapping
  suite passes 74/74, and staleness/metadata isolation finds zero findings
  across all 46 new discovery/catalog identities and the new invariant.
  `VERIF-P18-003` and `VERIF-P18-005` remain globally open; native ignored-test
  execution, the remaining 83 product/harness authorities, dynamic
  reachability, adequacy campaigns, and broad gates remain explicit debt. No
  broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-ignored-test-execution-closure-23.md`.
  <!-- generated:ignored-test-execution-closure-23:end -->
  <!-- generated:product-oracle-final-82-closure:start -->

  Final product-oracle denominator closure (2026-07-29): the last 83
  nonmapping scanner facts were reviewed as one frozen batch. One fact was a
  stray `TEST_PROGRAM` inside the intentionally uncompilable
  `single_axis_negative_stop_active` fixture; the declaration was removed
  while its invalid `MC_Stop.Active` source and exact E105 diagnostic remain
  executable through the strengthened existing negative parent. The remaining
  82 facts received exact specification and invariant authority, complete
  assertion review, focused execution, and two-reviewer accepted mappings.
  Their 455 assertions classify as 297 oracle-bound and 158
  setup/precondition assertions with zero unresolved helpers. All 76 selected
  Rust facts and six selected Structured Text facts pass at their owning
  boundaries; the refreshed negative parent also passes. Strengthening shared
  sources required 23 already-accepted source closures to be rebound without
  changing their reviewed semantics. The ADS setup asset test was corrected
  from a broad cross-area route to its exact runtime-safety specification and
  invariant. The atomic transaction moves 4,401 =
  4,318 `catalog_mapped` + 83 `reviewed_nonmapping` to 4,400 =
  4,400 `catalog_mapped` + zero `reviewed_nonmapping`, with an empty proposal
  queue. The focused mapping suite passes 74/74. Repository-wide staleness and
  metadata validation remain red on downstream Phase 18 debt; exact isolation
  finds zero batch-blocking findings and retains one pre-existing unrelated
  case-generator digest finding. `VERIF-P18-003`, `VERIF-P18-006`, native
  ignored-test execution, bidirectional reachability, mutation sensitivity,
  adequacy campaigns, and the final broad candidate gates remain open. No
  broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-product-oracle-final-82-closure.md`.
  <!-- generated:product-oracle-final-82-closure:end -->
  <!-- generated:exact-behavior-marker-9-closure:start -->

  Exact-behavior-marker closure progress (2026-07-29): all nine current stale
  marker debts are closed as one compiler/lexer/precedence batch. The nine
  invariants contain 51 behaviors bound exactly through 75 catalog identities,
  with zero missing or extra behavior references and zero missing catalog
  tests. On clean detached `trust-builder` accepted commit
  `e2c20f68a440bb656652216bee656fcb75e96a11`, the complete owning HIR,
  syntax, and precedence binaries pass 381/381. The frozen report retains
  4,400 live = 4,400 `catalog_mapped`, zero nonmapping, and zero confirmed
  missing tests; exact-behavior debt moves from 20 to the eleven historical
  placeholders, and `gap_open` coverage cells move from the current measured
  191 to 173. Private-hook-correct baseline and accepted metadata logs are
  byte-identical with 1,610 retained downstream findings and zero batch-added
  findings. The eleven placeholders cannot yet be removed because live
  code-spec facts still terminate in their historical gaps.
  `VERIF-P18-003`, `VERIF-P18-006`, reachability, mutation sensitivity,
  adequacy campaigns, environment proof, and final broad gates remain open.
  No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-exact-behavior-marker-9-closure.md`.
  <!-- generated:exact-behavior-marker-9-closure:end -->
  <!-- generated:code-spec-refresh-missing-1750:start -->

  Singular-area code-spec refresh progress (2026-07-29): the frozen clean
  17,135-fact code scan exposed 1,755 facts absent from the accepted map. One
  authenticated review selected all 1,750 facts whose live area candidate was
  singular and withheld all five multi-area facts. The selected facts remain
  explicit `spec_gap`; each is linked to its historical gap record, and all
  915 risk-bearing facts have exact body-, owner-, area-, signal-, and
  expiry-bound acknowledgements. Clean detached `trust-builder` commit
  `86e7b67d1a50c9df9f96e25a8802d1c535895deb` reports 17,130 mapped live
  facts, exactly five `missing_spec`, 15,447 `spec_gap`, 1,648 `specified`,
  and 709 retained findings. The post-refresh proposal has five missing
  mappings, 165 body-digest rebinds, 40 stale identities, 36 semantic groups,
  and zero unclassified findings. The metadata validator retains the exact
  1,610-finding pre-batch log byte-for-byte. This refresh closes no
  specification, test, reachability, mutation, or adequacy debt; it makes the
  live code denominator current for the next `VERIF-P18-003` authority batch.
  No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-code-spec-refresh-missing-1750.md`.
  <!-- generated:code-spec-refresh-missing-1750:end -->
  <!-- generated:code-spec-identity-maintenance:start -->

  Code-spec identity-maintenance progress (2026-07-29): the clean 17,178-fact
  source report exposed 48 missing mappings, 165 exact-identity body-digest
  rebinds, 40 stale identities, 36 semantic groups, and zero unclassified
  findings. One authenticated, live-reconstructed, compare-and-swap
  transaction mapped 47 facts directly to explicit historical gaps, mapped
  one `matches_breakpoint` successor through its exact predecessor, accepted
  all 165 rebinds, transferred 19 stale identities, and removed 21 stale
  identities. Four cross-area facts received explicit owner and area decisions
  but remain `spec_gap`; no authority or proof was inferred. The first
  projection was rejected before canonical application because
  `git diff --check` found malformed EOF whitespace in all three generated
  TOMLs. After a focused renderer fix and complete regeneration, clean detached
  `trust-builder` commit
  `0b2495babdbc5604a74f2239859feb223352be25` reports 17,178 live =
  17,178 mapped, zero missing/rebind/stale/unclassified identity findings,
  15,492 `spec_gap`, and 306 semantic findings fully partitioned into 33 review
  groups. The complete focused acceptance suite passes 45/45. The metadata
  validator remains byte-identical at its accepted 1,610-finding downstream
  debt baseline. `VERIF-P18-003`, specification authority, product-test proof,
  execution, reachability, mutation sensitivity, adequacy, and the final broad
  gates remain open. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-code-spec-identity-maintenance.md`.
  <!-- generated:code-spec-identity-maintenance:end -->
  <!-- generated:code-spec-semantic-review:start -->

  Code-spec semantic-review progress (2026-07-29): the initial 306 findings
  were completely partitioned into 33 review groups. Twenty-two
  invariant-to-test groups exposed a Cartesian-product validator defect; the
  accepted metadata contract requires a bipartite proof set in which every
  listed test links at least one listed invariant and every listed invariant
  links at least one test. The corrected validator removed those false
  groups. One recoverable, live-reconstructed semantic transaction accepted
  ten cross-area groups only after their invariant claims and exact source
  references covered all 255 affected facts, and removed
  `TEST_RUNTIME_HARNESS_ERROR_CLASSIFICATION_001` from two unrelated
  restart-lifecycle mappings while retaining its true harness transport and
  boundary-I/O mappings. The complete focused suite passes 70/70. Clean
  detached `trust-builder` commit
  `587bd203aa81b0fd0a5ec5d79a625b97a902542a` reports 17,192 live =
  17,192 mapped facts, zero identity findings, zero semantic findings, and
  an empty refresh proposal. The accepted report retains 15,506 `spec_gap`
  facts across 173 gap-open coverage cells and eleven historical
  placeholders. `VERIF-P18-003`, specification authority, execution,
  reachability, mutation sensitivity, adequacy, and final broad gates remain
  open. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-code-spec-semantic-review.md`.
  <!-- generated:code-spec-semantic-review:end -->
  <!-- generated:code-spec-authority-reuse:start -->

  Code-spec authority-reuse progress (2026-07-29): a clean zero-finding
  17,227-fact report proposed existing exact-path authority for 639 of 15,541
  `spec_gap` facts. One complete semantic review accepted 35 groups containing
  356 facts and withheld 22 groups containing 283 facts: eight ambiguous,
  eleven with mixed semantics, and three with a direct claim mismatch.
  Exact-path proximity never selected or expanded authority. The recoverable
  transaction live-reconstructed the proposal twice and changed only the
  selected code mappings and owning gap affected-code lists. Clean detached
  `trust-builder` commit
  `91710d3b27767df53d8a7c0358fa719837685fec` passes the focused
  authority/semantic/refresh suite 62/62 and reports 17,227 live = 17,227
  mapped facts, zero findings, 15,185 `spec_gap`, and 2,007 `specified`.
  Its identity/semantic refresh proposal is empty. The next authority proposal
  contains exactly the 22 withheld groups: 212 singular-candidate and 71
  ambiguous-candidate facts; another 14,902 facts have no reusable exact-path
  authority. `VERIF-P18-003`, missing-test review, execution, bidirectional
  chains, reachability, mutation sensitivity, adequacy, environment proof, and
  final broad gates remain open. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-code-spec-authority-reuse.md`.
  <!-- generated:code-spec-authority-reuse:end -->
  <!-- generated:residual-authority-trace-189:start -->

  Residual-authority trace progress (2026-07-29): the retained 189-identity
  Rust trace for all 22 withheld exact-path authority groups now completes on
  clean detached `trust-builder` commit
  `218d47ff4e581d2e49484e4b394ef6e3646da033`. The corrected resume reuses all
  183 prior instrumented executions, emits six explicit
  `no_nextest_identity` records, and attributes 19,350 test-to-code edges
  across 1,798 unique code facts. Within the 283 withheld candidate facts,
  proposed-authority tests reach 97 and do not reach 186: ambiguous groups are
  32/71 reached, mixed-semantics groups 65/199, and claim-mismatch groups
  0/13. Full dynamic reachability for the two PLCopen path groups does not
  change their accepted withholding: one remains authority-ambiguous and the
  other still contains behavior outside its singular claim. The clean
  17,227-fact code-spec report has zero findings and retains 15,185
  `spec_gap`; no mapping or proof state changes. `VERIF-P18-003`,
  `VERIF-P18-006`, bidirectional chains, formal reachability, mutation,
  adequacy, environment proof, and final gates remain open. No broad workspace
  gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-residual-authority-trace-189.md`.
  <!-- generated:residual-authority-trace-189:end -->
  <!-- generated:dap-wire-schema-6:start -->

  Debug-adapter wire-schema progress (2026-07-30): one normative JSON wire
  contract now owns request/response/event envelopes, standard DAP payloads,
  truST I/O and variable state/mutations, breakpoint/reload payloads, omission
  and default rules, explicit zero handling, and fail-closed malformed input.
  Six new conformance tests contribute 52 reviewed assertions and pass 6/6
  through the controlled clean-builder runner; the complete protocol selection
  passes 7/7. A closed-path runner gap was fixed with an assertion red and
  paired green. Focused coverage observes all six tests. Sixty production DAP
  protocol types are rebound from `spec_gap` to
  `DEBUG_DAP_WIRE_SCHEMA_001`, reducing the exact 17,229-fact census to 15,104
  remaining `spec_gap` facts with zero findings. No product defect was found.
  This is slice progress only; large subsystem authority batches, ten PLCopen
  test bindings, mutation/adequacy work, and final broad gates remain open.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-30-p18-dap-wire-schema.md`.
  <!-- generated:dap-wire-schema-6:end -->

  Portable runtime-core and startup coherent-batch progress (2026-07-30):
  focused authority and executable proof now cover portable lifecycle/default
  construction, recursive compound values, watchdog configuration, datetime
  calculation, LValue projections, stable error identity, numeric widening,
  initializer catalogs, task-state construction, `SIZEOF`, and runtime startup
  orchestration. The portable-core batches retire 68 live `spec_gap` facts.
  Five new startup tests plus two strengthened signal tests pass 7/7; an exact
  eight-test trace observes 8/8 selected identities, and reviewed automation
  binds all 17 traced startup facts. The clean 17,232-fact report now contains
  3,096 `specified` and 14,101 remaining `spec_gap` facts. The frozen
  4,457-test denominator contains 4,440 catalog-mapped and 17
  `reviewed_nonmapping` identities. Two pre-existing retain-snapshot invariant
  backlink findings remain explicit. The datetime proof exposed and fixed
  extreme-year intermediate overflow and acceptance of unrepresentable
  resolutions; the startup proof exposed no production defect. This is slice
  progress only: `VERIF-P18-003`, the two retain findings, remaining
  specifications/tests, mutation/adequacy/environment proof, and final broad
  gates remain open. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-30-p18-portable-core-startup.md`.

  Bytecode and authority-selection coherent-batch progress (2026-07-30): six
  exact bytecode specification sections, six focused invariants, and eight new
  cataloged behavior-lock tests bind 233 executable facts across the complete
  237-fact `crates/trust-runtime/src/bytecode/` denominator. Focused execution
  passes 10/10; the complete module trace observes 118/118 selected identities,
  the exact authority trace 51/51, and the new-test trace 8/8. Four structural
  facts remain explicit `spec_gap`: the public module type and three
  `ClassLike` trait signatures. The work also exposed and fixed a
  catalog-identity substitution defect, specified the recoverable
  authority-selection contract, and binds all 11 facts in that transaction
  module. The clean 17,233-fact report contains 3,340 `specified`, 13,858
  remaining `spec_gap`, and zero findings. This is 244 newly specified facts
  relative to the preceding startup report. `VERIF-P18-003`, the retained
  structural facts, mutation/adequacy/environment proof, and final broad gates
  remain open. No broad workspace gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-30-p18-bytecode-authority.md`.

  trust-syntax contract-authority progress (2026-08-01): the exact 229-test
  uncataloged `trust-syntax` contract population is now semantically routed
  against reviewed product specification authority before catalog mapping.
  Four public specification documents close the lexer candidate, lexer/parser
  handoff, textual ACTION, call-argument, generic/user-defined type,
  reference/pointer, partial-access, string-index, and parser recovery
  boundaries. Five new `specified` invariants contribute 90 explicit behavior
  partitions: 198 tests route to those new partitions and 31 route to existing
  lexer pragma/token, statement-form, and parser-recovery partitions. Static
  local validation parses every new invariant, validates its schema, resolves
  every oracle fragment against the registered active source, and accounts
  229/229 tests with zero unrouted identities. This is specification-authoring
  progress only: the tests remain `reviewed_nonmapping`, the new invariants
  retain explicit catalog-mapping/reachability/mutation/broad-gate debt, no
  code-spec fact changed disposition, and `VERIF-P18-003` remains globally
  open. The mandatory planner was attempted but stopped when it remained a
  long-running repository scan; it is not counted as proof. Proof is local;
  no compilation, test execution, conformance scan, coverage trace, or broad
  workspace gate ran.

  trust-plcopen contract-authority progress (2026-08-01): the cached exact
  denominator contains 195 live `trust-plcopen` tests: 15 are already
  catalog-mapped and the remaining 180 are the active
  `reviewed_nonmapping` authority population. Those 180 comprise 170 focused
  global-variable, project-model, semantic-import, semantic-export, and
  transactional-publication contract tests plus three stable contract-table
  and seven shim/metric tests. The public PLCopen compatibility specification
  now owns the five previously missing product contracts, and five new
  `specified` invariants contribute 61 explicit behavior partitions. Static
  routing accounts 180/180 tests with zero unrouted identities across 71 new
  or existing owning partitions; the stable target/options/POU tables and
  shim/metric rules reuse their existing reviewed authority instead of
  duplicating it. Targeted local metadata validation parses all five new
  invariants, checks the canonical case-family vocabulary and structured
  deltas, validates their schemas, resolves every oracle fragment against the
  active registered source, confirms repository-wide invariant-ID uniqueness,
  and passes `git diff --check`. This is specification-authoring progress
  only: all 180 tests remain `reviewed_nonmapping`, catalog binding, focused
  execution, reachability, mutation sensitivity, cross-platform proof, and
  broad gates remain explicit debt, no code-spec fact changed disposition,
  and `VERIF-P18-003` remains globally open. Proof is local; no compilation,
  test execution, conformance scan, coverage trace, or broad workspace gate
  ran. A metadata-validator help probe unexpectedly entered the validator's
  repository-wide scan and was terminated without output; it is not counted
  as proof.

  trust-dev contract-authority progress (2026-08-01): the exact 132-test
  `reviewed_nonmapping` population is now semantically routed as one complete
  package batch: 14 JSON-RPC envelope/dispatch tests, 14 workspace containment
  tests, 28 harness transaction tests, 39 documentation
  discovery/publication tests, and 37 language-workflow projection tests. The
  public Agent API and developer-workflow specifications already contained
  the detailed expected behavior; this batch therefore did not invent a new
  product rule from tests or implementation. It added four stable direct
  Agent API authority anchors, expanded the registered source coverage, and
  removed the stale registry limitation that contradicted the already
  normative symbolic-link containment contract. Five new `specified`
  invariants contribute 48 behavior partitions, and static routing accounts
  132/132 tests with zero unrouted identities. Targeted local metadata
  validation checks canonical case families, structured deltas, schemas,
  direct oracle fragments, source-registry coverage, and repository-wide
  invariant-ID uniqueness; `git diff --check` passes. This is authority
  progress only: all 132 tests remain `reviewed_nonmapping`, and catalog
  binding, focused execution, reachability, mutation sensitivity,
  cross-platform proof, broad gates, and `VERIF-P18-003` remain open. Proof is
  local; no compilation, test execution, conformance scan, coverage trace, or
  broad workspace gate ran.

  trust-lsp contract-authority progress (2026-08-01): the exact 270-test
  `reviewed_nonmapping` population across the ten configuration, dependency,
  document-text, external-diagnostic, runtime-value, index-cache,
  library-document, library-graph, server-state, and telemetry contract-test
  modules is now semantically routed as one complete package batch. The public
  LSP product specification already contained the detailed expected behavior;
  this batch therefore did not infer or invent product rules from tests or
  implementation. It expanded the registered source coverage and added ten
  complementary `specified` invariants with 87 behavior partitions. Static
  routing accounts 270/270 tests with zero unrouted identities: 47
  configuration, 42 dependency, 40 document-text, 21 external-diagnostic, 27
  runtime-value, 16 index-cache, 16 library-document, 20 library-graph, 21
  server-state, and 20 telemetry tests. Targeted local metadata validation
  checks all ten schemas, canonical case families, structured deltas, direct
  oracle fragments, source-registry coverage, and 400 repository-wide unique
  invariant IDs; `git diff --check` passes. This is authority progress only:
  all 270 tests remain `reviewed_nonmapping`, and catalog binding, focused
  execution, reachability, mutation sensitivity, cross-platform proof, broad
  gates, and `VERIF-P18-003` remain open. Proof is local; no compilation, test
  execution, conformance scan, coverage trace, or broad workspace gate ran.

  trust-hir contract-authority progress (2026-08-01): the exact 770-test
  `reviewed_nonmapping` population across 35 semantic-model, diagnostic, and
  type-checking modules is now semantically routed with zero leftovers. This
  review found that most expected behavior was already governed by the public
  data-type, variable, POU, expression, semantic-rule, configuration, HIR
  kernel, and warning specifications and by 12 existing reviewed invariants;
  those contracts were reused rather than duplicated. Seven genuine
  invariant-partition gaps were added for the diagnostic model, symbol/type
  registries, textual ACTION analysis, expression operators, edge-qualified
  inputs, ANY_BIT partial access, and user-defined type semantics, contributing
  70 explicit behavior partitions. Static routing accounts 770/770 tests:
  536 reuse existing invariants and 234 route to the seven new invariants.
  Targeted local validation checks all seven schemas, canonical case families,
  structured deltas, direct oracle fragments, refreshed source-registry
  coverage, and 407 repository-wide unique invariant IDs; `git diff --check`
  passes. This is authority progress only: all 770 tests remain
  `reviewed_nonmapping`, and catalog binding, focused execution, reachability,
  mutation sensitivity, cross-platform proof, broad gates, and
  `VERIF-P18-003` remain open. Proof is local; no compilation, test execution,
  conformance scan, coverage trace, or broad workspace gate ran.

  runtime-harness contract-authority progress (2026-08-01): the exact
  782-test `reviewed_nonmapping` population across all 34
  `crates/trust-runtime/src/host/harness/` contract-test modules is now
  semantically routed with zero leftovers. The public runtime-semantics
  specification already owns the detailed source-compilation, project
  assembly, configuration/access, coercion/codec, lowering, execution, and
  harness lifecycle transactions; its registry coverage was refreshed rather
  than duplicating those rules. Four new `specified` invariants contribute 51
  behavior partitions for source assembly, configuration/access assembly,
  typed-value protocol handling, and host source lowering. Static routing
  accounts 782/782 tests: 376 source-assembly, 191 source-lowering, 97
  configuration/access, 76 value/protocol, 21 existing TestHarness API, and 21
  existing process-image tests. Targeted local validation checks all four
  schemas, canonical case families, structured deltas, direct oracle
  fragments, refreshed source coverage, and 411 repository-wide unique
  invariant IDs; `git diff --check` passes. This is authority progress only:
  all 782 tests remain `reviewed_nonmapping`, and catalog binding, focused
  execution, reachability, mutation sensitivity, platform proof, broad gates,
  and `VERIF-P18-003` remain open. Proof is local; no compilation, test
  execution, conformance scan, coverage trace, or broad workspace gate ran.

  runtime-control contract-authority progress (2026-08-01): the exact
  717-test `reviewed_nonmapping` population across all 25
  `crates/trust-runtime/src/control/` contract-test modules is now semantically
  routed with zero leftovers. The public runtime-engine specification already
  owns the detailed control-request, source/endpoint, authorization,
  configuration, I/O/variable mutation, status/historian, communication, and
  fleet-topology transactions; its registry coverage was refreshed rather
  than duplicating those rules. Five new `specified` invariants contribute 52
  behavior partitions for authentication/operation policy, request/source
  boundaries, live configuration atomicity, mutation/query handling, and
  fleet projection. Static routing accounts 717/717 tests: 395 route to those
  five new invariants and 322 reuse the existing communication schema,
  discovery/browse, apply-mutation, and control-state invariants. Targeted
  local validation checks all five schemas, canonical case families,
  structured deltas, direct oracle fragments, refreshed source coverage, and
  416 repository-wide unique invariant IDs; `git diff --check` passes. This
  is authority progress only: all 717 tests remain `reviewed_nonmapping`, and
  catalog binding, focused execution, reachability, mutation sensitivity,
  platform/hardware proof, broad gates, and `VERIF-P18-003` remain open. Proof
  is local; no compilation, test execution, conformance scan, coverage trace,
  or broad workspace gate ran.

  runtime-host contract-authority progress (2026-08-01): the exact 481-test
  `reviewed_nonmapping` population across the 23 non-harness
  `crates/trust-runtime/src/host/` contract-test modules is now semantically
  routed with zero leftovers. Existing debug-control, DAP-session,
  helper-evaluation, metrics, simulation, and operator-console authority is
  reused for 342 tests. Four genuine gaps add 36 behavior partitions for the
  symbolic boundary protocol, pairing/TLS lifecycle, package-registry
  artifact integrity, and optional debug trace sink; the public runtime,
  developer-workflow, and debug specifications already contained the detailed
  rules and only their source coverage was refreshed. Static routing accounts
  481/481 tests with zero unrouted identities. Targeted local validation checks
  all four schemas, canonical case families, structured deltas, direct oracle
  fragments, refreshed source coverage, and 420 repository-wide unique
  invariant IDs; `git diff --check` passes. This is authority progress only:
  all 481 tests remain `reviewed_nonmapping`, and catalog binding, focused
  execution, reachability, mutation sensitivity, platform/security proof,
  broad gates, and `VERIF-P18-003` remain open. Proof is local; no compilation,
  test execution, conformance scan, coverage trace, or broad workspace gate
  ran.

  runtime-web contract-authority progress (2026-08-01): the exact 233-test
  `reviewed_nonmapping` population across all 13
  `crates/trust-runtime/src/web/` contract-test modules is now semantically
  routed with zero leftovers. The public runtime-engine specification already
  owns the detailed Web IDE utility, configuration-UI workspace, web
  authentication, POST admission, shared helper, and setup/I/O projection
  rules; its registry coverage was refreshed rather than duplicating those
  rules. Five new `specified` invariants contribute 45 behavior partitions:
  74 tests route to the IDE utility contract, 58 to configuration-UI routes,
  47 to authentication and POST admission, 30 to setup/I/O projection, and 24
  to shared helper grammar. Targeted local validation checks all five schemas,
  canonical case families, matching coverage dimensions, structured deltas,
  refreshed source coverage, 425 repository-wide unique invariant IDs, and
  `git diff --check`; static routing accounts 233/233 identities with zero
  invalid destinations. This is authority progress only: all 233 tests remain
  `reviewed_nonmapping`, and catalog binding, focused execution, reachability,
  mutation sensitivity, browser/platform proof, broad gates, and
  `VERIF-P18-003` remain open. Proof is local; no compilation, test execution,
  conformance scan, coverage trace, or broad workspace gate ran.

  runtime-I/O contract-authority progress (2026-08-01): the exact 98-test
  `reviewed_nonmapping` population across all six
  `crates/trust-runtime/src/io/` contract-test modules is now semantically
  routed with zero leftovers. The public runtime-engine specification already
  owns the detailed process-image interface, registry/provenance, GPIO, and
  shared Modbus/MQTT point-map rules; its registry coverage was refreshed
  rather than duplicating those rules. Four new `specified` invariants
  contribute 42 behavior partitions: 25 tests route to process-image
  interface transactions, 20 to driver registry/provenance, 26 to GPIO
  parameter/backend handoff, and 27 to typed point maps. Targeted local
  validation checks all four schemas, canonical case families, matching
  coverage dimensions, structured deltas, refreshed source coverage, 429
  repository-wide unique invariant IDs, and `git diff --check`; static routing
  accounts 98/98 identities with zero invalid destinations. This is authority
  progress only: all 98 tests remain `reviewed_nonmapping`, and catalog
  binding, focused execution, reachability, mutation sensitivity, physical
  GPIO/external-protocol proof, broad gates, and `VERIF-P18-003` remain open.
  Proof is local; no compilation, test execution, conformance scan, coverage
  trace, or broad workspace gate ran.

  runtime-language contract-authority progress (2026-08-01): the exact
  157-test `reviewed_nonmapping` population across the seven
  `crates/trust-runtime/tests/*contract.rs` language/stdlib modules is now
  semantically routed with zero leftovers. The public lexical, data-type,
  variable, standard-function, standard-function-block, and runtime-semantics
  specifications already own the detailed rules; their registry coverage was
  refreshed rather than duplicating those rules. The existing edge-declaration
  invariant exactly owns nine execution tests. Five new `specified` invariants
  contribute 52 behavior partitions for literal materialization (23 tests),
  conversion boundaries (26), core/helper functions (48), function-block
  runtime state (24), and user-type materialization (27). Targeted local
  validation checks all five schemas, canonical case families, matching
  coverage dimensions, structured deltas, refreshed source coverage, 434
  repository-wide unique invariant IDs, and `git diff --check`; static routing
  accounts 157/157 identities with zero invalid destinations. This is
  authority progress only: all 157 tests remain `reviewed_nonmapping`, and
  catalog binding, focused execution, reachability, mutation sensitivity,
  broad gates, and `VERIF-P18-003` remain open. Proof is local; no compilation,
  test execution, conformance scan, coverage trace, or broad workspace gate
  ran.

  runtime-config/core/VM contract-authority progress (2026-08-01): the exact
  100-test `reviewed_nonmapping` population across runtime configuration
  contracts, runtime-core accessors/state, and VM module/local-initialization
  modules is now semantically routed with zero leftovers. The public runtime
  engine and runtime-semantics specifications already own the detailed rules;
  their registry coverage was refreshed rather than duplicating those rules.
  Three new `specified` invariants contribute 29 behavior partitions: 27 tests
  route to the configuration value model, 41 to runtime core state/accessors,
  and 32 to VM module/local initialization. Targeted local validation checks
  all three schemas, canonical case families, matching coverage dimensions,
  structured deltas, refreshed source coverage, 437 repository-wide unique
  invariant IDs, and `git diff --check`; static routing accounts 100/100
  identities with zero invalid destinations. This is authority progress only:
  all 100 tests remain `reviewed_nonmapping`, and catalog binding, focused
  execution, reachability, mutation sensitivity, broad gates, and
  `VERIF-P18-003` remain open. Proof is local; no compilation, test execution,
  conformance scan, coverage trace, or broad workspace gate ran.

  runtime-tail contract-authority progress (2026-08-01): the exact 70-test
  `reviewed_nonmapping` population across bytecode helper, program-model,
  runtime-subsystem, CURRENT_DT, value-display, and borrowed-reference modules
  is now semantically routed with zero leftovers. Eight bytecode tests reuse
  the exact previously authored lowering, constructor, literal-index,
  resource-counter, return/reference, and numeric-domain invariants. The
  public runtime and standard-function specifications already own the other
  detailed rules; their registry coverage was refreshed. Five new `specified`
  invariants contribute 31 behavior partitions for program-model structure
  (18 tests), runtime subsystem helpers (17), CURRENT_DT host-clock behavior
  (5), value display (12), and borrowed reference traversal (10). Targeted
  local validation checks all five schemas, canonical case families, matching
  coverage dimensions, structured deltas, refreshed source coverage, 442
  repository-wide unique invariant IDs, and `git diff --check`; static routing
  accounts 70/70 identities with zero invalid destinations. This is authority
  progress only: all 70 tests remain `reviewed_nonmapping`, and catalog
  binding, focused execution, reachability, mutation sensitivity, broad gates,
  and `VERIF-P18-003` remain open. Proof is local; no compilation, test
  execution, conformance scan, coverage trace, or broad workspace gate ran.

  final debug/portable-core contract-authority progress (2026-08-01): the last
  26 tests in the frozen 4,245-test `reviewed_nonmapping` authority denominator
  are now semantically routed with zero leftovers. One portable-core unary
  test reuses the exact existing portable-operations invariant. The public
  debug-adapter specification already owns lifecycle and request ordering; one
  new `specified` invariant contributes 11 behavior partitions for the 25 DAP
  lifecycle tests. Targeted local validation checks the schema, canonical case
  families, matching coverage dimensions, structured deltas, refreshed source
  coverage, 443 repository-wide unique invariant IDs, and `git diff --check`;
  static routing accounts 26/26 identities with zero invalid destinations.
  Across all completed authority batches, static semantic routing now accounts
  for the complete frozen 4,245/4,245 `reviewed_nonmapping` population with no
  unrouted identity. This closes the specification/invariant-authoring
  denominator only: the tests still retain their catalog disposition until the
  consolidated mapping transaction, and focused execution, reachability,
  mutation sensitivity, platform/hardware/UX proof, broad gates, and
  `VERIF-P18-003` remain open. Proof is local; no compilation, test execution,
  conformance scan, coverage trace, or broad workspace gate ran.

  recovered static authority refresh (2026-08-12): an exact clean source
  freeze found 8,690 active scanner facts and 17,465 code facts. The complete
  76-finding code-identity ledger was reviewed with zero unclassified items;
  six exact stale identities were transferred, 15 body digests were rebound,
  and nine singular-owner facts were registered against their existing gaps.
  Three new test facts are now catalog-bound: two runtime-clock behaviors use
  `TRUST_RUNTIME_CLOCK_DISPATCH_001`, while the canonical LSP severity-key
  collision uses `LSP_CONFIGURATION_NORMALIZATION_002#behavior:12`. The
  alias-only collision partition remains explicit P18-006 test debt. The
  current denominator closes exactly as 4,457 mapped plus 4,233 nonmapping,
  and the reviewed route inventory and 203-batch execution plan cover exactly
  those 4,233 nonmapping identities with 59 mapped companions on routed paths.
  Scanner, denominator, route, plan, and companion identities are unique and
  equal at every required boundary. A clean `trust-builder` candidate at
  `649643786636d1034297a17f8809f7d8e539b337` then regenerated 17,465 live
  code facts with zero findings and zero blocking findings, and reproduced the
  route-plan digests. This is authority, catalog, and static
  planning progress only; global P18-003/P18-005 closure, focused execution,
  reachability, mutation, and all later proof rows remain open. No Rust test,
  Cargo build, broad validator, `just fmt`, `just clippy`, or `just test-all`
  ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-12-p18-recovered-static-authority-refresh.md`.

  focused trace resolver repair (2026-08-12): the closed target-profile table
  is now authoritative before generic target fallback. A dedicated shard owner
  copies each test's target and module prefix before grouping and separates
  default-feature from no-default-feature commands. The original focused red
  exposed three target/name failures; a second detached baseline red exposed
  source-derived feature classification and same-package mixed-profile prefix
  loss. The repaired candidate passes the complete 48-test focused Python gate.
  A resolver-only nextest-list preflight resolves exactly 4,233/4,233 requested
  identities, including 4,231 default-feature and two no-default-feature rows,
  with zero unsupported identities and without executing a coverage test. The
  earlier 3,997 attempts are not reusable because all 29 instrumented
  executable digests changed; the next authenticated trace therefore starts
  fresh only after the static candidate is frozen. This repairs the trace
  harness only; semantic mapping review, authenticated trace completion,
  reachability, and all downstream Phase 18 rows remain open. No broad
  workspace test, `just fmt`, `just clippy`, or `just test-all` ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-12-p18-focused-trace-resolver-repair.md`.

  supply-chain/platform authority closeout (2026-08-12): the final 11
  `supply_chain_platform` code gaps are now specification- and test-bound.
  Ten runtime bundle-builder functions reuse the reviewed source/dependency
  contract, `RT_BUILD_COMMAND_001`, and exact existing builder tests. The
  Windows Cargo build script now has an explicit 8 MiB linker-stack contract,
  `RT_WINDOWS_STACK_LINK_001`, and one std-only behavior lock covering
  non-Windows, Windows MSVC, and Windows non-MSVC output partitions. The local
  focused behavior lock passed 1/1, the four-file atomic mapping transaction
  passed the complete live 17,488-fact identity validator, and the historical
  aggregate supply-chain gap closed with zero remaining affected code IDs.
  No production code changed. Native Windows linking/execution, current
  catalog execution, reachability, mutation sensitivity, later platform
  proof, broad gates, and `VERIF-P18-003` globally remain open. No broad/full
  workspace test, `just fmt`, `just clippy`, or `just test-all` ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-12-p18-supply-chain-platform-authority-closeout.md`.

  release-evidence guard authority progress (2026-08-12): the release
  manifest, annotated-tag, exact-SHA/main-push workflow, published release,
  Latest marker, and unique required-asset guards now share
  `REL_RELEASE_EVIDENCE_GUARD_001` and one catalog-visible two-partition
  behavior lock. An expected assertion red exposed the wrapper's missing
  prover artifact; the paired green passes 1/1, and the complete focused
  release suite passes 50/50 locally and on the bootstrapped isolated
  `trust-builder` checkout. Scanner, denominator, catalog, gate inventory,
  suite routing, case provenance, and exact slice metadata bindings are green.
  The new shell entrypoint is explicitly review-bound across the verification
  and release areas with its current risk acknowledgement. A clean 17,489-fact
  full report then proposed exactly three singular release groups; the atomic
  review accepted all 21 facts for the new invariant, withheld the other 31
  proposal groups, and closed the now-empty historical release gap. The public
  release-chain invariant remains unchanged, so mocked focused tests do not
  claim an actual published release. Global metadata debt still blocks the
  producer-authentic lock and complete planner; the new invariant remains S0
  with that debt explicit. `VERIF-P18-003`, `VERIF-P18-005`, reachability,
  mutation, and the final broad gates remain open. No P18-016 broad gate ran.
  Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-12-p18-release-evidence-guard-authority.md`.

  portable-core authority-reuse progress (2026-08-12): the complete live
  31-group existing-authority proposal was semantically reviewed rather than
  accepting singular path matches mechanically. Six exact groups bind nine
  initializer, operator, default-value, `SIZEOF`, date-time, and stable-error
  facts to six existing portable-core invariants and thirteen cataloged
  behavior locks. The other 25 groups and 192 candidate facts remain withheld
  as ambiguous, mixed-semantics, or claim-mismatch debt. The atomic two-file
  transaction passed live reconstruction; a clean detached `trust-builder`
  report at `8cab239452b7fe83dbb52b4192d1341e0f50fbcf` accounts 17,489 facts with
  zero findings, 13,837 `spec_gap`, and 3,392 `specified`. All 13 exact
  cataloged nextest selections and 40 focused authority-tool tests are green.
  The six invariants remain S0: producer-authentic lock proof, reachability,
  mutation, global P18-003/P18-005 closure, and later proof rows remain open.
  No P18-016 broad gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-12-p18-portable-core-authority-reuse.md`.

  portable numeric-kind authority progress (2026-08-13): a behavior-preserving
  module split separates portable kind/widening ownership from numeric
  conversion normalization while preserving the public API and cataloged test
  identities. The guarded refresh removed four stale pre-split identities and
  registered four moved successors; because path and qualified symbol changed,
  the exact-successor guard correctly rejected automatic transfer. A complete
  25-group semantic review then accepted only the coherent four-fact
  `numeric/kind.rs` group for `RT_CORE_NUMERIC_KIND_WIDENING_001` and retained
  the other 24 prior withholds. The clean post-authority report at
  `aae1beef07947569f8df1c475faf7fce3723c8a3` accounts 17,489 facts with zero
  findings, 13,836 `spec_gap`, and 3,393 `specified`, retiring one net gap.
  Pre/post and final exact numeric behavior locks pass, as do 76 focused
  metadata-tool tests and the no-default core check. Assertion reachability,
  mutation sensitivity, `VERIF-P18-003`/`005`, and the later candidate rows
  remain open. No P18-016 broad gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-13-p18-numeric-kind-authority.md`.

  portable task-model authority progress (2026-08-13): a
  behavior-preserving module split separates task configuration, mutable
  scheduling state, and readiness evaluation while preserving the public API
  and seven cataloged task-test identities. The guarded refresh removes five
  stale facts and registers five moved successors. A complete 24-group review
  accepts only the coherent two-fact `task/state.rs` group for
  `RT_CORE_TASK_STATE_INITIALIZATION_001` and retains the other 23 prior
  withholds. A focused red/green repair then permits already accepted
  `implemented` invariants to enter the same review-only reuse queue without
  weakening its oracle, catalog, complete-review, or atomic-apply guards. The
  expanded 25-group review restores `RT_TASK_READINESS_001` to the moved
  evaluator and result type and withholds all other groups. The clean
  post-TaskConfig report at `73292072` accounts 17,491 facts with zero
  findings, 13,835 `spec_gap`, and 3,396 `specified`; the exact task selector
  passes 7/7, the fresh readiness trace observes 6/6, the exact TaskConfig test
  passes 1/1, and the final focused authority/transaction suite passes 61/61.
  TaskConfig now has exact specification, code, invariant, and test authority.
  `VERIF-P18-003`/`005`, reachability, mutation, and later candidate rows remain
  open. No P18-016 broad gate ran. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-08-13-p18-task-model-state-authority.md`.
- [ ] `VERIF-P18-005` Eliminate all 4,233 current `reviewed_nonmapping` test
  facts.
  After the owning `P18-003` authority is accepted, catalog-map each to that
  accepted invariant and an executable suite, bind governed gate/fuzz/device
  campaigns to equivalent executed proof contracts, or remove it with
  independently reviewed no-loss evidence. No nonmapping or inventory-only
  disposition is final.

  Mapping-accelerator implementation checklist:

  - [x] Derive focused per-test execution traces on `trust-builder` with exact
    `DISC_*` and `CODE_*` attribution; unsupported adapters remain explicit.
  - [x] Generate `C1` through `C3` quarantined candidates plus a separate
    specification-demand route without turning coverage, names, paths, or
    mutation kills into proof.
  - [x] Validate that pending or rejected proposals cannot affect any catalog,
    denominator, coverage, debt, claim, proof, or zero-equation count.
  - [x] Require exact per-test assertion-to-oracle confirmation, with two
    distinct reviewers for high-risk invariant mappings.
  - [x] Apply accepted batches through one generated, recoverable catalog and
    denominator transaction; reviewers never author IDs, hashes, counts, or
    ledger rows.
  - [x] Prove zero false accepted mappings on a hidden-ground-truth focused
    pilot before expanding the trace adapters or running the final campaign.
    Retained evidence:
    `docs/internal/testing/evidence/plc-verification-program/2026-07-24/mapping-accelerator-ground-truth/`.
  - [x] SOLID: keep trace execution, candidate ranking, quarantine validation,
    semantic review, and catalog mutation in separate modules.
  - [x] KISS: use one canonical proposal/decision path and the existing scanner,
    code-spec map, invariant, catalog, and denominator sources.
  - [x] DRY: reuse existing identity, schema, digest, live-join, and canonical
    rendering helpers instead of duplicating verification vocabularies.
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
