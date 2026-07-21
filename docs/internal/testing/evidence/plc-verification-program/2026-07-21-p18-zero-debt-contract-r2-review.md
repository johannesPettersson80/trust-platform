# Phase 18 Zero-Debt Contract R2 Review

## Verdict

`BLOCKED`

The corrected contract resolves all eight findings from the first independent
review and now defines a credible zero-debt objective, substrate, evidence
model, invalidation model, and final candidate-to-public-proof sequence. One
new execution-order defect remains: `VERIF-P18-004` cannot close before
`VERIF-P18-005` and `VERIF-P18-006`, although the declared dependency order
requires it to close first.

Consequently:

- `VERIF-P18-000` may not close from this review.
- Committing the exact reviewed files does not authorize the
  `VERIF-P17-012` / `VERIF-STOP-017` transition.
- `VERIF-P18-001` remains the intended first executable Phase 18 row, but it is
  not authorized by this review.
- After the slice order is corrected and that exact contract receives a fresh
  commit-bound independent acceptance, `VERIF-P18-000` may close; the declared
  Phase 17 transition may then occur, and `VERIF-P18-001` is the first
  executable row.

No implementation row, broad gate, merge, tag, release, or public claim is
authorized by this review.

## Independent review target

- Reviewer: Codex independent R2 review agent; this agent did not author the
  reviewed Phase 18 contract.
- Canonical source and review checkout:
  `/home/johannes/projects/trust-platform`.
- Branch: `plc-verification-program`.
- Base HEAD: `ee00258c42cadd98ac70a5c6523957f5ee44fe7c`.
- Review target: the complete current seven-file uncommitted Phase 18 contract
  and transition diff, plus the Phase 17 independent verdict and first Phase
  18 contract review named below.
- `AGENTS.md` SHA-256:
  `2808d20386ee2604bd20136f4b9b767686a491ba27f981d5cc94f1f6ed4f3be3`.
- Complete current `.codex/skills/**` tree: 25 files, SHA-256 aggregate
  `6d88257949fc4c831783adb388fc9adceaa4d2df30912aebe3ee914c8499def9`.
  The 21 durable non-`__pycache__` files have aggregate SHA-256
  `4faf27313a75bb7ea372a193cc0a446dd66167ad8e6bf941b8bf3f24b016e4b3`.
- Private hook SHA-256:
  `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d`.

The review applied `trust-test-authoring`, `st-lsp-solid`,
`trust-architecture-automation`, `trust-ci-release-gates`,
`trust-release-hygiene`, `trust-lsp-iec`, `trust-hmi-contracts`,
`trust-vscode-quality`, `vscode-ui-acceptance`, and `vscode-capture` to their
respective contract and evidence concerns.

## Exact reviewed contract

| File | SHA-256 |
| --- | --- |
| `docs/internal/testing/checklists/plc-verification-program/phase18-zero-debt-execution-board.md` | `85c220d93dc0fbb8ebfdf74c5bf686cda03cf4fb932464a4f044fe55bc8114a1` |
| `docs/internal/testing/checklists/plc-verification-program/implementation-board.md` | `a4eb6af1adb9cf43c57d68d68420e47e3d8442ef12b845a96bd6aa463657191e` |
| `docs/internal/testing/checklists/plc-verification-program-checklist.md` | `b91c9a94e513fccdc8c3a1e8264314551d88b77ac6ed0348d735ddcd478452d9` |
| `docs/internal/testing/checklists/plc-verification-program/code-spec-coverage-plan.md` | `e3322e4ddaae60c481da4eb48caba4c2b7caa7c79b9b8ea2f58950d310cee795` |
| `docs/internal/testing/checklists/architecture-improvements.md` | `aa4fc13c85bd49a65d4307ced9ca7b8d8bf8f6bc864017918b93154b4feb62d3` |
| `verification/matrix.toml` | `418d3689c6fdc3c7adfffa3e84ec916498df0de533141e4b16a1f78df2afbf33` |
| `verification/spec-document-reviews.toml` | `00c6c443809647f12a82d62c1b5d00d604831021434e639f68370412010c119f` |

Reviewed predecessor evidence:

| File | SHA-256 |
| --- | --- |
| `docs/internal/testing/evidence/plc-verification-program/2026-07-21-p17-independent-final-acceptance.md` | `f2ce76606f68d1c8b384dea906d021fb388abdedf0916050065c33b2de80b6d2` |
| `docs/internal/testing/evidence/plc-verification-program/2026-07-21-p18-zero-debt-contract-review.md` | `119f6f7fc2126e86ec5616ff0412de18ec478f34dbff33f887cd5443ddb112ba` |

Any change to one of the seven contract files changes the review target and
requires a new independent review. A future commit must bind the unchanged
content hashes above or bind a newly reviewed corrected set; the base HEAD by
itself does not contain this uncommitted contract.

## Focused validation

No tests or broad gates were run. Read-only checks established:

- `git diff --check` and `git diff --cached --check` pass;
- both changed TOML files parse;
- all five changed or added Markdown contract documents exactly match their
  registered hashes in `verification/spec-document-reviews.toml`;
- execution rows `VERIF-P18-000` through `VERIF-P18-018` have exactly one
  definition each;
- stop rows `VERIF-STOP-018A` through `VERIF-STOP-018F` have exactly one
  definition each;
- the verification matrix routes both the root PLC checklist and the complete
  PLC verification-program checklist tree through the required checks;
- the changed contract is seven files, 445 insertions, and 19 deletions before
  this evidence file;
- the dedicated Phase 18 board is 310 lines, while the mirror remains a mirror
  rather than the execution authority.

The raw metadata reproduces every baseline number used by the contract:

| Population | Exact count |
| --- | ---: |
| code facts | 15,229 |
| `specified` | 56 |
| `spec_gap` | 15,138 |
| `inherits_spec` | 30 |
| `generated` | 2 |
| `non_behavioral` | 2 |
| `vendored_external` | 1 |
| historical facts in eleven area-wide gap records | 15,133 |
| `CURRENT_DT` gap facts | 3 |
| private-hook gap facts | 2 |
| discovered test facts | 4,038 |
| `catalog_mapped` | 261 |
| `reviewed_nonmapping` | 3,777 |
| no reviewed spec/invariant binding | 3,687 |
| gate-inventory-owned | 59 |
| ignored-test-registry-owned | 23 |
| fuzz-program-owned | 8 |

## First-review finding disposition

| Finding | R2 result | Contract evidence |
| --- | --- | --- |
| P18-C01: live `spec_gap` escape | Resolved | The zero equations reject every live `spec_gap` fact regardless of lifecycle, and a negative fixture covers closing a gap record without rebinding its live facts. |
| P18-C02: owned ignored/skipped escape | Resolved | Ignored, skipped, filtered, undiscovered, unrun, zero-test, timed-out, unavailable, and harness-failed proof remains blocking; ownership is not execution evidence. |
| P18-C03: Phase 17 transition ambiguity | Resolved | The board explicitly orders P17 pilot acceptance, P18 contract acceptance, P17 denominator/enforcement closure, substrate, ledger, slices, one candidate milestone, public proof, and final independent closure. |
| P18-C04: final-review/public-proof cycle | Resolved | Candidate gates are in P18-016, exact main/tag/public binding is in P18-017, and the final raw-evidence verdict is P18-018. |
| P18-C05: missing executable substrate | Resolved | Canonical ledger, schema, budget, claim, CLI, artifact, producer/validator, pin, budget, digest, fixture, and CI-route contracts are named. |
| P18-C06: claim retirement escape | Resolved | Retirement requires a reviewed product/compatibility decision and synchronized code, package, configuration, documentation, changelog/version/deprecation, release, and independent evidence; an unavailable runner cannot retire a claim. |
| P18-C07: unsupported mutation denominator | Resolved | The unsupported historical 35/55 requirement is removed; final acceptance instead requires dynamic reachability and mutation/catcher evidence for every mapped fact. |
| P18-C08: structural disposition ambiguity | Resolved | Exact rules and negative fixtures are defined for `inherits_spec`, `generated`, `non_behavioral`, and `vendored_external`. |

## Blocking finding

### P18-R2-01 — Slice chain closure precedes the tests it requires

The dependency section orders each slice as:

1. `P18-003` specification decisions;
2. `P18-004` complete bidirectional chains;
3. `P18-005` existing-test mapping and `P18-006` missing tests;
4. `P18-007` reachability and assertion strength.

That order is not executable. `VERIF-P18-004` requires a complete
specification -> invariant -> code -> test -> suite -> evidence chain and says
that an unexecuted terminal test fails. But `VERIF-P18-005` is the row that
maps or removes the 3,777 existing nonmapping test facts, and
`VERIF-P18-006` is the row that creates and executes missing behavior tests.
For a slice requiring either operation, the evidence needed to close P18-004
cannot exist until P18-005/P18-006 have run, while those rows are declared to
follow P18-004. This is a hard closure cycle, not merely an opportunity for
parallel work.

Required correction for each slice:

1. close `P18-003` specification and invariant authority;
2. run `P18-005` to map/remove existing tests and `P18-006` to add and execute
   missing tests, in parallel where their inputs permit;
3. close `P18-004` by validating the now-complete bidirectional chain and its
   executed terminal evidence;
4. close `P18-007` with dynamic reachability and assertion-strength proof.

Alternatively, P18-004 could be explicitly reduced to a non-closing chain
skeleton before P18-005/P18-006, with a separate post-P18-006 row that validates
and closes the complete chain. Reordering is simpler and preserves the existing
row meanings.

The same corrected order must be reflected anywhere the phase dependency
sequence is mirrored. `P18-008..015` should remain conditional on the complete
corrected `P18-003..007` slice chain and preaccepted budgets.

## Closure decision

The content corrections for P18-C01 through P18-C08 are accepted. The exact
reviewed contract is nevertheless not executable without contradictory row
closure because of P18-R2-01. Fix that order, update all affected registered
document hashes, commit the corrected contract, and obtain a fresh independent
review bound to the corrected commit and file hashes before closing
`VERIF-P18-000` or authorizing the Phase 17-to-18 transition.
