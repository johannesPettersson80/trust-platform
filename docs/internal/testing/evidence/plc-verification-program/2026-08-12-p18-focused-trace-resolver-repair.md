# Phase 18 focused trace resolver repair

Date: 2026-08-12

Original baseline source: `9720a014f7e473a34a58c5f56704cc72271cb108`

Feature-shard red baseline: `b33063421824b702eeba3a0ee7fbeb2f83ffb7b9`

Resolver implementation candidate: `7fdd5d82c9517141bc25e781fadda4115e601732`

## Resolver repair red green and static rebinds

The exact 4,233-identity focused coverage trace resolved and executed 3,997
tests, but rejected 236 before execution. The failures had one shared cause:
the generic resolver ran before the frozen Rust target profiles. It selected
the `trust-lsp` library instead of its binary for 230 tests and discarded the
module prefix needed to distinguish six duplicate runtime terminal names.

The original focused red baseline ran the five directly owning Python test
modules:

```text
python3 -m unittest \
  scripts.verification.coverage_trace_resolver_tests \
  scripts.verification.coverage_trace_cli_tests \
  scripts.verification.focused_mapping_context_tests \
  scripts.verification.focused_trace_authentication_tests \
  scripts.verification.authority_route_execution_plan_tests
```

On unchanged original production code, 46 tests ran and exactly the three
intended assertions failed: LSP binary ownership, target-prefix preservation,
and duplicate terminal-name resolution. Compile, dependency, harness, timeout,
and unrelated failures were absent.

The minimal repair makes the existing closed target-profile table authoritative
before the generic fallback, carries its module prefix through nextest listing,
and resolves the exact prefixed nextest test name. A dedicated
`coverage_trace_shards.py` owner copies the closed target, prefix, and feature
mode onto each fact before grouping; default-feature and no-default-feature
commands cannot share a shard.

Two additional behavior slices were established against the detached
`b3306342` baseline. The source-derived no-default-feature classifier failed
because it did not exist, and the same-package mixed-profile test failed because
the `discover` row inherited the stale `browse_symbols` prefix. Both passed on
the repaired candidate. The full focused command now passes 48/48:

```text
Ran 48 tests in 0.037s
OK
```

The resolver-only preflight constructed exact nextest-list shards and stopped
before any coverage execution. It resolved the current authority request as:

```text
requested=4233 resolved=4233 unsupported=0
default_features=4231 no_default_features=2
```

The candidate-bound code-spec refresh preserves all affected verification
functions under `SPEC_GAP_P17_HISTORICAL_CODE_VERIFICATION_001`. Identity
maintenance does not create specification authority, test mapping,
reachability, or execution proof. Its final source-bound report, proposal,
review selection, application receipt, and zero-finding reconstruction are
retained under `target/gate-artifacts/code-spec-refresh/` and
`target/gate-artifacts/verification/` on the clean builder candidate.

## Focused trace continuation boundary

The earlier trace's 3,997 completed attempts are evidence only. Comparison of
the trace manifests against the current candidate found that all 29
instrumented executable digests changed, so secure reuse is invalid even though
the resolver change itself is Python tooling. The corrected trace must start in
a fresh artifact root after the static candidate is frozen, authenticate the
complete 4,233-identity request, and execute every identity against one bound
source/tool/profile closure.

The fresh candidate executed all 4,233 exact identities successfully. During
final trace reconstruction, inspection showed that the lazy coverage mapping
exported every test once to discover observed paths and a second time to build
the trace; production candidate authentication would repeat both passes. The
first reconstruction was stopped at 2,177/4,233 exports without deleting or
changing any completed attempt, raw profile, manifest, executable, or request.

A focused regression then counted coverage-map reads and failed `1 != 2` on
the unchanged implementation. The repair filters LLVM export to the governed
mapped Rust source set, observes the actually executed paths while document
assembly consumes each export once, and writes an authenticated receipt only
after schema and live-fact validation. The receipt binds the canonical trace,
exact request and resolved-nextest identities, raw profiles, executables, LLVM
tools, and covered source bytes. Candidate generation revalidates those bytes
without Cargo, nextest, profdata, or llvm-cov replay. The five focused modules
covering trace orchestration, document assembly, LLVM tools, receipt
authentication, and candidate CLI pass 59/59 locally after the expected red.
No Rust behavior or executable input changed, so the 4,233 completed attempts
remain the exact reusable execution evidence for the resumed one-pass
reconstruction.

The first optimized resume also exposed that raw-evidence capture reread a
shared executable once per test identity, reaching 310 GB of logical reads
before LLVM export. It was stopped with all attempt evidence intact. A focused
two-attempt/shared-binary regression failed with three reads instead of one;
the capture now snapshots and hashes each unique executable once and reuses
those bytes for every manifest and LLVM-tool binding. The focused set passes
60/60 after this second expected red.

No broad workspace test, `just fmt`, `just clippy`, `just test-all`, full
verification wrapper, mutation campaign, or final Phase 18 gate ran. The
hour-long broad gate remains reserved for the frozen `VERIF-P18-016` candidate
after all preceding rows close.

## Verification-owned authority transaction

The reviewed verification contract now explicitly owns the private pre-tool-use
hook and focused trace control plane. The exact transaction promotes 67 live
facts: 12 private-hook facts and 55 focused-trace facts. Every promoted fact is
bound to its exact contract fragment, invariant, catalog test, current body
digest, and risk acknowledgement when risk-bearing. The private-hook gap has no
remaining affected code identity and is closed as specification-authority debt;
its reachability, mutation, and final-candidate proof obligations remain open in
`VERIFICATION_PRIVATE_HOOK_001` and are not implied by this closeout.

Focused validation ran on `scena-rust-builder` in the bootstrapped validation
checkout `/home/johannes/projects/trust-platform-p18-trace-schema-52814dee-r1`
with the candidate files synchronized from the primary checkout:

```text
python3 -m unittest \
  scripts.verification.coverage_trace_cli_tests \
  scripts.verification.coverage_trace_document_tests \
  scripts.verification.coverage_trace_tools_tests \
  scripts.verification.focused_trace_authentication_tests \
  scripts.verification.mapping_candidates_cli_tests

Ran 63 tests in 0.045s
OK

scripts/private_hook_contract_gate.sh

Ran 14 tests in 2.365s
OK

scripts/focused_trace_control_plane_gate.sh

Ran 63 tests in 0.043s
OK
```

The focused trace behavior lock includes receipt atomic commit, idempotent
rewrite, refusal to overwrite different evidence, and deterministic observed
source projection without authority-metadata mutation. The repository-wide
metadata validator still reports the pre-existing Phase 18 global debt outside
this batch; batch-local findings are evaluated separately and do not convert
that global failure into a pass.

## Code-spec report zero-finding repair

The first clean report after the focused-trace and private-hook authority
transaction found nine metadata-maintenance identities: three stale scanner
identities with already mapped live replacements, two newly scanned shell gate
entrypoints, three changed focused-trace functions, and one risk-acknowledgement
expiry mismatch. The stale identities were removed from the map, risk register,
and historical-gap population; the live gate entrypoints and changed functions
were bound to their accepted focused-trace or private-hook contracts with exact
body digests and risk signals. This transaction does not claim execution,
reachability, mutation adequacy, or Phase 18 closure.

A stricter bidirectional registry check then exposed 1,650 additional stale
members in `verification/spec-gaps.toml`: 1,557 identities whose live map
disposition is now `specified` and 93 identities absent from the live map. The
focused red observed 15,899 unique registry members against 14,249 live
`spec_gap` facts. Removing exactly those stale members produced an exact
14,249-to-14,249 equality with an empty symmetric difference. This is registry
consistency maintenance only; it does not promote, specify, or prove any fact.
