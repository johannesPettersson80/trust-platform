# Phase 18 Zero-Debt Execution Board

Status: zero-debt contract accepted by commit-bound independent R3 review.
Findings `P18-C01` through `P18-C08` and `P18-R2-01` are resolved.

Current row: `VERIF-P18-003`, starting with exact slice
`P18-CODE-CODE_F4ABB9ACC5495574EA09AD38CFC84F2BDE1D0BACA961B3CEAF9D9E44992E3F53`.
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
  progress only: assertion reachability, mutation sensitivity, the broad
  remote gates, and `VERIF-P18-003` globally remain open. Evidence:
  `docs/internal/testing/evidence/plc-verification-program/2026-07-22-p18-retain-snapshot-insert.md`.
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
