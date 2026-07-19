# Final Fourteen-Invariant Proof Validation

Date: 2026-07-17

Specification, cases, implementation, and release checkpoint:
`fc651193d0ea662c0f8b061c7d2670e37ad788de`

Final lock-proof checkpoint:
`8e7a30e3d60c7ff3b52a77c5e1da399bc22e539e`

Invariant-promotion checkpoint:
`c350d4be`

Final 15-report source checkpoint:
`9b746400755d6186acbcef217d78749a388cf7a4`

Remote broad-gate checkpoint:
`69099a6d6c2293feaead17aa51fec4c1e0d37d1f`

Public specification wrapper checkpoint:
`b878d4f66ff57ef969e28acbe670b6a67a6df836`

Specification-audit rebind checkpoint before this record:
`2e751ec5ad3381d2e90d728ad22dbef12920ef92`

## Outcome

This batch completed the last 14 S0 invariant verticals. It added 14 cataloged,
hand-authored case tables containing 19 executable cases for developer
workflows, connector status, release evidence, behavior locks, platform paths,
VSIX target identity, artifact provenance, and dependency exceptions.

`prove.py v1` wrote 14 clean `lock_baseline` rows and 14 clean descendant
`lock_compare` rows. Every pair has distinct run IDs, clean full revisions,
valid ancestry, matching case and execution-contract digests, and an
all-passing case summary. These records lock passing written behavior. They do
not manufacture red evidence or claim that a pre-baseline defect has a causal
red/green proof chain.

The invariant registry now contains 54 records:

- 45 `implemented` invariants at G1;
- 9 `implemented` invariants at G2; and
- 0 invariants at S0.

`VERIF-P16-003` and `VERIF-P16-005` are complete. `VERIF-P16-004` remains open
because this batch does not relabel passing lock evidence as red/green proof.
`VERIF-P16-002` remains open for four public release claims.

## Normative contracts and tests

The new normative sources are:

- `docs/specs/22-developer-workflows.md`;
- `docs/specs/23-connector-status.md`; and
- `docs/specs/24-release-evidence.md`.

The 14 new catalog rows are:

- `TEST_DEV_COMMIT_SCOPE_TRACE_001`;
- `TEST_DEV_TEST_DISCOVERY_TRACE_001`;
- `TEST_UI_CONNECTOR_STATUS_TRACE_001`;
- `TEST_RELEASE_PLATFORM_MATRIX_TRACE_001`;
- `TEST_RELEASE_SOURCE_BUILD_TRACE_001`;
- `TEST_RELEASE_HARDWARE_CLAIM_TRACE_001`;
- `TEST_RELEASE_CONFORMANCE_STATUS_TRACE_001`;
- `TEST_RELEASE_VERSION_CHAIN_TRACE_001`;
- `TEST_RUNTIME_BEHAVIOR_LOCK_TRACE_001`;
- `TEST_DEBUG_BEHAVIOR_LOCK_TRACE_001`;
- `TEST_PLATFORM_PATH_TRACE_001`;
- `TEST_VSIX_TARGET_IDENTITY_TRACE_001`;
- `TEST_ARTIFACT_PROVENANCE_TRACE_001`; and
- `TEST_DEPENDENCY_EXCEPTION_TRACE_001`.

The cases run through the owning product or release-tooling paths. There is no
parallel implementation of the behavior under test.

## Defects found and fixed

The new tests and audits found three shipped behavior or supply-chain defects:

1. `trust-dev commit` could absorb a caller-owned pre-staged path inside the
   selected project. The command now aborts before index or history mutation;
   root commit refuses every pre-staged path, and dry-run/cancel remain
   non-mutating.
2. The VS Code connector projection accepted arbitrary backend state and
   health strings. It now accepts only the canonical closed vocabulary and
   rejects unknown values instead of rendering them as valid status.
3. The VS Code dependency graph contained 13 npm advisories, including two
   critical and six high. Direct updates and compatibility-tested overrides
   reduce `npm audit --audit-level=low` to zero; CI and release packaging now
   execute the audit.

The source-build audit also corrected a verification checker that assumed only
three OpenOT packages. It now validates the complete shipped public-Git subset
and rejects sibling/path sources for every OpenOT package.

Final validation found and fixed three non-product integration issues:

- stale specification-topic postures after seven specification gaps closed;
- stale scanner and coverage census expectations after the 14 new catalog
  rows and test facts were added; and
- missing public wrapper pages for specs 22-24, which made strict MkDocs links
  fail even though the lightweight link checker passed.

## Gap and claim posture

The specification-gap register contains 35 rows:

- 31 are closed; and
- 4 remain `spec_updated`.

The four open release-claim rows are:

- `SPEC_GAP_PLATFORM_SUPPORT_MATRIX_001`;
- `SPEC_GAP_BEHAVIOR_LOCKED_PUBLIC_CLAIM_001`;
- `SPEC_GAP_SOURCE_BUILD_PUBLIC_CLAIM_001`; and
- `SPEC_GAP_HARDWARE_PUBLIC_CLAIM_001`.

Each has an explicit required-spec row with `blocks = "release_claim"`.
Targeted tests and G1 invariants therefore cannot silently authorize those
public claims.

## Generated report binding

Fourteen report pairs were generated from clean commit
`9b746400755d6186acbcef217d78749a388cf7a4` with timestamp
`2026-07-17T12:19:00+02:00`. Their production validators passed at generation
and at rest. The strict-docs fix changed only the specification-source audit
closure, so that report alone was regenerated from clean commit
`b878d4f66ff57ef969e28acbe670b6a67a6df836` with timestamp
`2026-07-17T13:44:17+02:00`.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `51a515e22ab2568c0cb144b5bf33e4109bd1294b56489f36dce552ff90150bdf` |
| Coverage-matrix gaps | `e2a090bd42c8b2cd79593137ebabce118d1e66753a7ebb4ab29a129e03d3f43b` |
| Malformed-input coverage | `897deffc6c75b7c6aabfd9fcdc9b336125b0f45c09f5e622364cf99474217d54` |
| Unmapped-test debt | `b555664b5f0e69801e6c61cfa2f95b05c179e07b24f4f5a568da5a3a6ccfd750` |
| Test-refactor assessment | `243c5eb6a653545ceb25e1fc2c42517b8682a444cdb1c5ddaab2a628ee009ae8` |
| Ignored-test inventory | `269850f9d341161da6c48100defe9c8ec35d0cbadef40b20d67f111aee14f0f5` |
| Phase 5 suite audit | `9a075c23dbfa7f0c93a50460960d72e32ff0e12b2b4173368eeeafacf10f660b` |
| Invariant-seed audit | `480e315cbec214a6c79e82cc865836eb232128cd272719d732be0dadf8444d21` |
| Specification completeness | `27c7b64f0ec1d4dcd6c4a4a3905239613e17923fdb6350099a5e926b1f628f43` |
| Requirement/oracle audit | `e37b94fed0e181194343c8c742dede9951cafc4bc53814887ee4c279b4b99c46` |
| Conformance alignment | `ca767c0cc9267ef744e4a568339b5578bc5a10da79415c8cd4166d46971c10d7` |
| Runtime-anomaly audit | `91b09e3cae680e08ae659155fd626c87bfc4272d12aca81f236a3b4ef6f0c62a` |
| Fuzz-program audit | `9aa332167c6fffc2c8ea0cc6e6cd0b8798a25e677a8ebea0be7bcba962997229` |
| Mutation program | `4cde11fbbeb2608654adf3eb82638de9dc2d7a32230d712e97d81d6f57e2e1fe` |
| Specification-source audit | `b5770c610a62ce184ae1cd51fb1ffbbabf1b25573f7d88d39dc797bac7de2aa5` |

## Final remote validation

Heavy validation ran once at the end on the isolated `trust-builder` checkout
`$HOME/projects/trust-platform-final14-final`.

- `just fmt`: passed in 3 seconds;
- `just clippy`: passed in 2 seconds on the warmed target;
- `just verification-veryquick`: passed in 1,063 seconds, including 813
  focused Python tests and metadata validation of 709 records;
- `CARGO_INCREMENTAL=0 just test-all`: passed with zero failures in 444
  seconds;
- explicit `api_smoke`, `debug_control`, `complete_program`, and
  `runtime_reliability` verticals: 3 + 20 + 1 + 4 tests passed in 85 seconds;
- `npm ci`, `npm audit --audit-level=low`, `npm run lint`, and
  `npm run compile`: passed, with zero npm vulnerabilities;
- `xvfb-run ... npm test`: 458 VS Code extension tests passed in 33 seconds;
- `mkdocs build --strict`: passed after the missing wrappers were fixed;
- public-doc asset and search checks: passed; and
- `python3 scripts/check_diagram_drift.py`: passed.

The first broad `test-all` attempt exhausted the generated target filesystem
without a test failure. Per `AGENTS.md`, work stopped, only generated targets
were removed, disk was rechecked, and the non-incremental retry above was run
with a durable log and exit-code file. The first Electron attempt similarly
failed before extension load because there was no X display; the required Xvfb
retry passed all 458 tests. Neither infrastructure failure is counted as a
product test failure.

The builder's newer PlantUML binary rewrote unrelated SVG metadata. Those
unrelated generated bytes were discarded; the committed manifest/source drift
check passed and no diagram source changed after the earlier reviewed release
evidence-flow update.

## Honest remaining debt

- Conformance alignment remains 0 of 21 explicitly linked.
- Runtime-anomaly coverage has 6 test-gap classes.
- Fuzz-program coverage has 6 surface gaps.
- The hand-owned catalog maps 220 of 4,009 scanner facts; 3,789 facts remain
  catalog debt, not automatically missing product tests.
- The ignored register contains 27 ignored and 2 conditional observations.
- Seven expected-result tests remain unbound in the specification-completeness
  report.
- Four public release claims remain blocked as listed above.
- No invariant is promoted beyond the evidence actually recorded; this batch
  ends at 45 G1 and 9 G2.
