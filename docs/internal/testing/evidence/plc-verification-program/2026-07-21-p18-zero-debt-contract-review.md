# Phase 18 Zero-Debt Contract Review

## Verdict

`BLOCKED`

Phase 18 names every required proof class and correctly rejects broad-suite,
scanner, coverage-percentage, simulator, HTTP-only, and implementation-derived
substitutes. Its baseline arithmetic is exact. It is not yet an unambiguous,
executable zero-debt contract, however: several clauses still permit debt to be
renamed or skipped, the transition from Phase 17 is undefined, and the final
review is ordered before evidence it is required to accept.

No checklist row is authorized by this review. After the required contract
edits below, a fresh independent review may authorize and close
`VERIF-P18-000`; the first implementation row would then be
`VERIF-P18-001`. This review does not authorize `VERIF-P17-012`, any Phase 17
or Phase 18 stop row, product implementation, broad gates, merge, or release.

## Independence and reviewed state

- Reviewer: Codex independent review agent; the reviewer did not author the
  Phase 18 edits.
- Canonical source and review checkout:
  `/home/johannes/projects/trust-platform`.
- Branch: `plc-verification-program`.
- HEAD: `ee00258c42cadd98ac70a5c6523957f5ee44fe7c`.
- Review input: the complete uncommitted six-file Phase 18 diff plus
  `2026-07-21-p17-independent-final-acceptance.md`.
- `AGENTS.md` SHA-256:
  `2808d20386ee2604bd20136f4b9b767686a491ba27f981d5cc94f1f6ed4f3be3`.
- Complete current `.codex/skills/**` tree: 25 files, SHA-256 aggregate
  `6d88257949fc4c831783adb388fc9adceaa4d2df30912aebe3ee914c8499def9`.
  Four entries are ignored generated Python bytecode caches; the 21 durable
  instruction/source files have aggregate SHA-256
  `4faf27313a75bb7ea372a193cc0a446dd66167ad8e6bf941b8bf3f24b016e4b3`.
- Private hook SHA-256:
  `be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d`.
- Worktree at review start: exactly the six named contract files were modified,
  and the Phase 17 independent verdict was untracked. No test, contract, source,
  metadata, or generated file was changed by this review before this verdict.

The review applied `trust-test-authoring`, `st-lsp-solid`,
`trust-architecture-automation`, `trust-lsp-iec`,
`trust-ci-release-gates`, `trust-release-hygiene`, `trust-hmi-contracts`,
`trust-vscode-quality`, `vscode-ui-acceptance`, and `vscode-capture` to the
proof and acceptance classes they own.

## Cheap validation and baseline arithmetic

No tests or broad gates were run. Read-only validation established:

- `git diff --check` passes for all six contract files;
- `verification/matrix.toml` and
  `verification/spec-document-reviews.toml` parse as TOML;
- the four changed reviewed documents exactly match their registered SHA-256
  values;
- all `VERIF-P18-000` through `VERIF-P18-015` and
  `VERIF-STOP-018A` through `VERIF-STOP-018E` identifiers occur exactly once;
- the root matrix now routes the root PLC verification checklist through the
  verification-tooling gates.

The current raw baselines reproduce the stated headline counts:

| Population | Exact count |
| --- | ---: |
| code facts | 15,229 |
| `specified` | 56 |
| `spec_gap` | 15,138 |
| `inherits_spec` | 30 |
| `generated` | 2 |
| `non_behavioral` | 2 |
| `vendored_external` | 1 |
| historical facts in eleven `SPEC_GAP_P17_HISTORICAL_CODE_*` records | 15,133 |
| `CURRENT_DT` gap facts | 3 |
| private-hook gap facts | 2 |
| discovered test facts | 4,038 |
| `catalog_mapped` | 261 |
| `reviewed_nonmapping` | 3,777 |
| no reviewed spec/invariant binding | 3,687 |
| gate-inventory-owned | 59 |
| ignored-test-registry-owned | 23 |
| fuzz-program-owned | 8 |

The Phase 18 introduction should call 15,229 the complete scoped code
denominator, not the production denominator: it includes 227 Rust and 1,903
Python/shell verification-tooling facts.

## What the contract gets right

- The final objective is semantic and executable proof, not row accounting.
- Specifications must precede expected-result tests; IEC, PLCopen, product,
  protocol, and vendor authority are distinguished from implementation text.
- Code, invariant, test, suite, and evidence joins must be bidirectional and
  exact.
- Test removal requires reviewed no-loss evidence.
- Assertion strength, source coverage, mutation, fuzz/property, concurrency and
  time, unsafe/FFI, real hardware/endurance, rendered UX, platform, and public
  proof are all explicit, non-substitutable obligations.
- Mutation timeouts/build failures/skips and hardware absence/simulation/skips
  remain debt.
- `ARCH-VERIF-P18-01` correctly requires separate focused validators and
  artifact contracts instead of one god gate.
- The stop rows reject blanket waivers, lowered denominators, broad-gate
  substitution, and implementation-side final acceptance.

## Required contract edits

### P18-C01 — Require zero live `spec_gap` dispositions

Affected text: `VERIF-P18-000`, `VERIF-P18-002`, `VERIF-P18-003`,
`VERIF-STOP-018B`, the root checklist purpose chain, and the Phase 18 mirror in
the code-spec plan.

`VERIF-P18-000` currently requires zero *open or actionable* `spec_gap` facts.
That permits a live code mapping to remain `spec_gap` after its gap record is
marked closed or otherwise non-actionable. Final acceptance must instead
require zero live code facts whose disposition is `spec_gap`, regardless of the
gap record's lifecycle status. Closed gap records may remain as history, but no
live code identity may terminate in them. Likewise, zero
`registered_backlog` must be defined as zero live facts whose referenced gap
has that resolution status, not merely zero records in one ledger.

Add an exact enforcing count and a negative fixture proving that changing a gap
record to `closed` without rebinding the affected live facts to a specified
chain still fails. Clarify in the root purpose diagram that a spec-gap path is
an interim blocking route and never final proof.

### P18-C02 — Close the owned-ignored and skipped-test escape

Affected text: `VERIF-P18-000`, `VERIF-P18-004`, `VERIF-P18-012`, and
`VERIF-STOP-018B`.

The final count currently rejects only **unowned** ignored tests, while
`VERIF-P18-004` allows a test to bind to an ignored-test contract with
"equivalent ownership and evidence." The current denominator contains 23
ignored tests, including hardware, performance, exhaustive OSCAT, and OpenOT
lanes. Those tests could be renamed from `reviewed_nonmapping` to an owned
ignored disposition without ever executing. The hardware row closes this
escape only for hardware claims.

Require every proof-bearing test or campaign to execute successfully in every
named applicable authority lane before final acceptance. An ignored, skipped,
filtered-out, unregistered, undiscovered, unavailable, timed-out, or
harness-failed case remains blocking even when it has an owner or registry
entry. A test may avoid execution only by reviewed removal with no-loss
evidence; inventory ownership alone is not proof. Add ignored/skipped cases to
`VERIF-STOP-018B` and add negative fixtures for owned-but-never-executed tests,
zero-test filters, and skip-as-success wrappers.

Gate and fuzz entrypoints may retain their specialized inventory records, but
their required commands/campaigns must run and produce the evidence promised by
their mapped contracts.

### P18-C03 — Make the P17-to-P18 dependency order executable

Affected text: `VERIF-P17-012`, `VERIF-P18-000` through
`VERIF-P18-015`, and both phase stop-row sets.

`VERIF-P17-012` remains open and still requires a "final"
`fmt`/`clippy`/`test-all` milestone, while Phase 18 assumes that Phase 17 is
enforced and reserves another final milestone. The board does not say whether
Phase 18 may begin before P17-012, whether the P17 broad milestone is deferred,
or which row owns the independent Phase 17 denominator acceptance. This can
produce either an illegal phase jump or two expensive broad milestones that
prove different trees.

State the transition explicitly. The recommended order is:

1. accept the Phase 17 result only as a denominator/enforcement baseline;
2. accept and freeze the corrected Phase 18 contract;
3. build the deterministic Phase 18 ledger;
4. execute specification and proof burn-down in declared dependency slices;
5. close all adequacy/environment rows;
6. freeze one zero-debt source candidate;
7. run one final broad and external proof sequence for that candidate.

Name which earlier rows are hard prerequisites for every later row and which
may run in parallel. A finding in coverage, mutation, fuzz, concurrency,
hardware, UX, or platform work must reopen the owning spec/test/code slice and
invalidate downstream evidence. Do not call any pre-Phase-18 broad run the
program's final milestone.

### P18-C04 — Fix the circular final-review and public-proof sequence

Affected text: `VERIF-P18-015` and `VERIF-STOP-018E`.

`VERIF-P18-015` requires the independent reviewer to return an accepting final
verdict and only then runs `fmt`/`clippy`/`test-all`, strict preflight, the
GitHub matrix, and public release proof. `VERIF-STOP-018E` simultaneously
requires the final independent verdict to cover all evidence tiers. Both cannot
be true.

Split candidate acceptance from final closure or reorder the row. Require all
of `P18-001..014` closed first, then freeze one clean candidate and run focused
gates, final remote broad gates, exact strict preflight, and the GitHub
candidate matrix. Require the validated candidate SHA to become the exact main
and annotated-tag source without source or metadata changes, complete the
public release/asset/checksum/Marketplace claim chain that applies, and only
then obtain the commit/tag-bound independent final verdict over the raw rows and
all artifacts. Any source, validator, metadata, instruction, base, or evidence
contract change restarts the affected focused review and invalidates later
candidate evidence.

### P18-C05 — Define machine-checkable producers, budgets, and stop criteria

Affected text: `VERIF-P18-001`, `VERIF-P18-006` through
`VERIF-P18-015`, and `ARCH-VERIF-P18-01`.

The rows name proof categories but do not yet name canonical manifests,
schemas, producer/validator commands, toolchain pins, artifact identities,
negative fixtures, or per-row dependency/status rules. In particular,
"complete" fuzz/property/differential campaigns have no predeclared seed,
corpus, duration/iteration/coverage budget, plateau or stop condition;
"reviewed soak duration" can be selected after the run; and "supported
architectures/features" has no canonical claim-derived matrix. A one-second
campaign or a manually curated subset could therefore satisfy the prose.

Create a dedicated Phase 18 execution board and canonical machine-readable
ledger(s). For every row/slice record at least:

- exact denominator identities and owner;
- prerequisite specification/decision and invariant;
- producer and independent validator module;
- pinned tool and command, environment/topology, seed/corpus/shards, and
  predeclared duration/iteration/coverage/soak stop criteria where applicable;
- expected numerator/denominator and zero-debt acceptance expression;
- canonical artifact path, schema, source SHA, input closure, and digest;
- known-bad fixture proving the validator fails when the proof is absent,
  skipped, weakened, or stale;
- CI/release route and reopen/invalidation rules.

Keep these as separate per-responsibility validators and reports. The current
`implementation-board.md` is already 2,051 lines after this addition, while the
root checklist says the program was split to avoid another 2,000-line god-file.
Move Phase 18's executable detail to a dedicated board and leave a pointer and
high-level stop gates in the implementation board.

### P18-C06 — Prevent claim removal from lowering the matrix

Affected text: `VERIF-P18-014` and `VERIF-STOP-018D`.

`VERIF-P18-014` permits an unavailable platform/combination to be removed from
product claims, while the stop row forbids lowering platform obligations to get
green. Add a reviewed scope-retirement protocol: identify every public/package/
configuration claim, record the product authority and compatibility decision,
remove the implementation/package/configuration/docs claim consistently,
record changelog/version/deprecation impact, and complete independent release
verification. The claim-derived matrix denominator remains frozen until that
retirement is accepted. Tool or runner unavailability alone may not justify
claim removal.

### P18-C07 — Remove or prove the unsupported `time.rs` metric

Affected text: `VERIF-P18-006`.

The exact statement that 35 of 55 specified `time.rs` facts were not exercised
has no cited coverage, mutation, trace, or catcher-removal artifact in the
Phase 17 evidence. The raw map contains 55 unique specified facts: 48 link to
the arithmetic test and eight link to the split test, with `register` in both
sets. Static inspection shows the cataloged commands call all registered
non-clock functions and all five split families through their dispatch paths.
That mapping does not itself prove dynamic reachability or assertion strength,
but it also does not establish the claimed 35-fact deficit.

Either attach a source-SHA-bound per-function execution artifact that derives
35/55, with its command and methodology, or replace the number with the valid
general finding: Phase 17 proved semantic linkage, not dynamic reachability or
mutation sensitivity, so Phase 18 must measure both for every mapped fact.

### P18-C08 — Define every structural disposition's final rule

Affected text: `VERIF-P18-000` and `VERIF-P18-003`.

`VERIF-P18-000` allows `inherits_spec`, `generated`, `non_behavioral`, and
`vendored_external`, but `VERIF-P18-003` revalidates only inheritance,
generated, and vendored boundaries. Add `non_behavioral` and define each final
rule separately. A `non_behavioral` result needs source-derived proof that the
fact cannot affect shipped behavior, risk, packaging, or proof decisions; a
rationale string is insufficient. Generated facts need exact generator and
input closure; vendored facts need provenance plus an executable product
boundary; inherited facts need an acyclic exact parent chain ending in a
specified, executed, mutation-sensitive boundary. Add negative fixtures for
each terminal rule.

## Required next action

Edit only the contract surfaces needed to close P18-C01 through P18-C08,
synchronize their registered document digests and matrix routing, and request a
fresh independent contract review. Until then, no Phase 18 implementation row
is authorized.

This verdict creates no product proof, resolves no specification gap, maps no
test, accepts no skipped lane, and promotes no invariant.
