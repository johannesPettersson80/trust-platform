# Phase 18 stale case-authority refresh

Date: 2026-07-28

## Scope and result

The previous exact-behavior campaign rejected fifteen case tables because their
recorded invariant execution-contract digests were stale. This transaction
reconstructed every table from current authority and accepted only the
thirteen whose complete case payload still mapped one-to-one to current
behavior partitions:

- 13 invariant authorities;
- 82 exact cases;
- 54 previously unbound behavior references;
- 59 exact assigned case IDs;
- 16 affected case-backed catalog rows;
- 13 executable runners across 12 source paths;
- six generator-owned tables reproduced from their current invariant inputs;
- seven reviewed hand-owned state-machine tables with refreshed source
  authority;
- assignment digest
  `7647b73d2f76c3347416b085255cf05fb4a9b21f98620406fe7bc4e4c85956d7`.

`RT_SAFE_STOP_001` and `VM_SEAM_VALID_001` remain rejected. Their current
behavior partitions do not equal the existing case scenarios, so changing only
their source digest would falsely bless stale semantics.

The frozen behavior-demand equation moves from 321 to 267 with zero missing
specification authority. This is exact existing-proof refinement across
`P18-004` and `P18-005`; it is not 54 newly written tests and does not close a
Phase 18 row globally.

## Test-first and focused execution proof

The transaction followed focused test-first development:

1. Importing the not-yet-created transaction module failed before
   implementation.
2. Finalization first failed because `finalize_refresh_plan` did not exist.
3. String-map catalog replacement first failed because the shared replacement
   helper did not support the required source-closure map.
4. Post-apply reconstruction first failed because the preceding 101-behavior
   transaction could not tolerate a later, valid reduction from 321 to 267.
5. The source-closure review test then failed with missing
   `source_closure_refresh_decision_count` before the accepted proposal and
   decision refresh existed.

The final local focused command is:

```text
python3 -m unittest \
  scripts.verification.stale_case_authority_refresh_tests \
  scripts.verification.case_backed_behavior_assignments_tests \
  scripts.verification.frozen_catalog_gaps_tests

Ran 23 tests
OK
```

The mandatory planner then ran once against all 38 final changed paths with
`--intent test-refactor`. It accepted the complete path set and exited 6 during
repository metadata validation, reporting the first 20 findings plus 1,194
more from the known Phase 18 baseline. The independent staleness comparison
below is the scoped check that proves this transaction adds none of those
findings.

The regenerated tables deliberately caused six runner-guard failures before
the new scenarios were reviewed in the runners. Those failures were exact
unreviewed-scenario or missing-input guard failures, not compile, dependency,
timeout, or unrelated failures. Minimal runner changes then covered:

- six parser-recovery scenarios;
- multiplicative-before-comparison precedence;
- unsigned below-zero conversion;
- contextual untyped numeric common-type selection;
- the no-accuracy-preserving-common-type rejection;
- bounded `STRING[3]` concatenation;
- explicit `VOID` function return-type fail-closed behavior.

The unified builder campaign passed all 13 runners and all 82 cases. The six
generated authorities were byte-equal to fresh generator output and account
for 60 of the 82 cases.

The transaction implementation is split by responsibility to satisfy the
verification-tooling architecture guard: 792 lines of preparation and
orchestration, 345 lines of execution/evidence finalization, 315 lines of
applied-state reconstruction, and 253 lines of accepted-review refresh. The
same 23 focused tests pass locally and on `trust-builder` after the split.

## Assertion-review closure

Changing the twelve runner sources also changed the source closure of 46
already accepted generated-test mappings. Refreshing only the catalog digests
would leave their accepted proposal snapshots and decisions stale.

The builder therefore extracted 46 current assertion inventories from exact
candidate `4065e7d6b5a417dc603d582b4b62f51b9168f388`. For every selected test, the
assertion IDs, kinds, source paths, functions, expressions, expression
digests, and unresolved-helper population equal the prior accepted snapshot.
Only the source and aggregate inventory digests changed.

One atomic transaction now:

- gives all 46 proposal snapshots new content-addressed proposal IDs;
- regenerates all 46 accepted decision IDs from the unchanged semantic review;
- updates the catalog proposal, decision, reviewed-source, and assertion-source
  bindings;
- preserves invariant selections, behavior selections, assertion
  dispositions, reviewers, commands, expected results, failure modes, and
  suite routing;
- binds the source-closure artifact digest
  `sha256:99cf37107e30cd7e0272b508358cf283e720f53b210e9a7666d65cb47ae825fc`.

Canonical validation of the 46 refreshed accepted decisions reports zero
failures. The required repository staleness validator remains red on 1,217
pre-existing Phase 18 findings, but contains zero findings for the 46 refreshed
test IDs, 13 execution-test IDs, 13 new proof IDs, 12 runner paths, or 13 case
paths. Before the detached builder commit was imported, the same comparison
contained exactly 26 selected findings: the commit and targeted proof-scope SHA
did not resolve locally for each of the 13 records. Importing that exact commit
removed all 26 without changing source or metadata.

## Evidence and proof replacement

The transaction retires 26 superseded current
`execution_contract_v2` records to `source_revision` and appends one current
record for each of the 13 passing runners. Each new record binds:

- the exact builder candidate SHA;
- its catalog execution contract and linked invariant contract;
- exact command and zero exit status;
- current case-file digest;
- ordered per-case passing summary and result digest;
- machine-readable case artifact and artifact digest.

The builder commit object was imported into the local object database under
`refs/codex-validation/p18-stale-case-authority-refresh-54`, so every retained
proof commit and targeted proof-scope SHA resolves locally. The final Phase 18
candidate at `VERIF-P18-016` will supersede intermediate proof with the one
accepted candidate-wide gate.

## Builder and checkout proof

Validation environment:

- canonical checkout and agent-file source:
  `/home/johannes/projects/trust-platform`;
- local branch: `plc-verification-program`;
- local starting HEAD:
  `fcb5e9781a84fc5d3a105ccb12696b2af126e6a5`;
- isolated remote destination:
  `/home/johannes/projects/trust-platform-p18-remaining-142`;
- evidence host: `trust-builder` (`scena-rust-builder`);
- exact detached execution candidate:
  `4065e7d6b5a417dc603d582b4b62f51b9168f388`.

Before editing or execution, the destination `AGENTS.md` and complete
`.codex/skills/**` directory were manually copied from the canonical checkout
and verified byte-for-byte. The builder disk preflight found insufficient
space for the campaign; only the generated
`~/.cache/codex-targets/trust-platform-gate` cache was removed. The resulting
free space was 52 GiB under `/home/johannes` and 3.2 GiB under `/tmp`.

Retained builder artifacts live under:

```text
target/gate-artifacts/verification/p18-stale-case-authority-refresh-54/
```

The authoritative set contains the 13-runner campaign, 82 case results, 13
proof records, generated-authority equality report, 46 source-closure facts,
and 46 assertion inventories. Three earlier invalid attempts remain retained
under `attempt-01` through `attempt-03` and are not proof.

Final canonical metadata digests are:

- `verification/test-catalog.toml`:
  `3495178b983491bbd00e1d3e7188198a4057f6996c4f205d9763ea08c36c4efe`;
- `verification/test-catalog-mapping-decisions.toml`:
  `9b3a7c0c9dbdbaf4e5ee8004e14eecfc6061c6d739d4ba84bc1a7290b8ec248a`;
- `verification/evidence-index.toml`:
  `9ab6e7a2962f5f0ebde8969f269bfeae1a16f73aef7e937e9b622d63c347bc67`;
- executed semantic lock:
  `e65eb2115b119ff3fbb0819f31eff88ebcaf6c4e3ca78669bf6dbc99504b6f1e`.

No broad workspace `just fmt`, `just clippy`, `just test`, or `just test-all`
ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
