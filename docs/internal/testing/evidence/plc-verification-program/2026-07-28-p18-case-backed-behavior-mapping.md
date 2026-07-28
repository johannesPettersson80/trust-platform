# Phase 18 case-backed behavior mapping

Date: 2026-07-28

## Scope and result

The zero missing-authority result made 422 specified behavior slots visible as
unbound exact behavior references. This campaign does not assume that every
unbound slot requires a new test. It first joins the slots to current committed
case tables, existing mapped runners, and retained per-case execution evidence.

An initial structural join found 162 slots with some case-file association.
Fail-closed semantic and provenance checks accepted only 101 slots:

- 101 exact invariant behavior references;
- 37 invariants;
- 37 existing catalog-mapped runners;
- 105 exact case IDs;
- 37 retained passing execution records;
- assignment digest
  `cb4a4f71406292b32a6d4c3aa0697d1a0d216fe21482c11272e465dc1c87fd18`.

The accepted join requires all of the following:

- the case file names the same invariant;
- its `source_digest` equals the current invariant execution-contract digest;
- the cataloged `case_file_digest` equals the current file bytes;
- the case partition equals the invariant behavior partition, or a hand-owned
  state-machine scenario equals the reviewed `equals` partition;
- every present outcome, delta, error, no-partial-apply, fault-surface, and
  oracle field is identical;
- at least one exact case is retained;
- a committed evidence row with the same case-file digest records every
  assigned case as passed with exit status zero.

Multiple exact cases may legitimately prove one behavior partition. For
example, a denial behavior can contain separate viewer and operator cases.
Neighboring partitions, oracles, or outcomes do not match.

## Rejected case authorities

Fifteen invariant case tables had stale invariant execution-contract digests
and were excluded:

- `DEV_COMMIT_SCOPE_001`;
- `DEV_TEST_DISCOVERY_001`;
- `IEC_PARSE_RECOVER_001`;
- `IEC_PREC_001`;
- `IEC_TIMER_001`;
- `RT_RELOAD_001`;
- `RT_SAFE_DEADLINE_001`;
- `RT_SAFE_RESTART_001`;
- `RT_SAFE_STOP_001`;
- `RT_UNSIGNED_INTEGER_MATERIALIZATION_001`;
- `SEC_AUTHZ_001`;
- `VM_SEAM_DECLARED_TYPE_001`;
- `VM_SEAM_ENC_001`;
- `VM_SEAM_STRING_BOUND_001`;
- `VM_SEAM_VALID_001`.

`EDIT_RENAME_001#behavior:3` also has no exact case in its otherwise-current
case table. Three other `EDIT_RENAME_001` behaviors were mapped; this one
remains open. None of these rejected slots was inferred from names, neighboring
tests, or stale output.

## Test-first transaction proof

Before the transaction module existed, the exact focused command

```text
python3 -m unittest \
  scripts.verification.case_backed_behavior_assignments_tests
```

failed while importing the missing
`scripts.verification.case_backed_behavior_assignments` module. After the
minimal matcher, planner, catalog updater, evidence rebinder, semantic lock,
and recoverable apply path were implemented, all eight initial tests passed.

An idempotence test was then added before its helper. It failed because
`applied_plan_from_documents` did not exist. The helper now reconstructs and
validates the same frozen plan after application, so crash recovery or a
second invocation cannot reinterpret the already-applied transaction.

The final focused command combines the transaction tests with the frozen gap
equation tests:

```text
python3 -m unittest \
  scripts.verification.case_backed_behavior_assignments_tests \
  scripts.verification.frozen_catalog_gaps_tests

Ran 11 tests
OK
```

It passes locally and in the isolated `trust-builder` checkout.

The first green implementation was 928 lines. Under the established
SOLID/KISS/DRY size guard it was split, without changing behavior, into a
723-line selection/transaction owner and a 241-line proof/document helper.
The same 11 behavior-lock tests remained green after the split.

The required planner ran for the broad 52-invariant candidate set, again for
the exact catalog/evidence transaction after the proof-digest consequence was
identified, and once more against the final split-module changed-path set.
Each run accepted `--intent test-refactor` and exited 6 on the known aggregate
metadata debt. The final planner invocation reported the first 20 findings plus
1,279 more because four legacy red/green evidence records without the newer
explicit binding field each produced duplicate stale-proof findings. A focused
expected-red test then required all four records to be included; both assertions
failed at 68 updates before the fix and passed at 72 afterward. A read-only
metadata-validator comparison after the fix restored the exact 1,291-finding
pre-transaction baseline, with zero selected-evidence or selected-proof-digest
findings. The remaining baseline includes legacy statuses, stale evidence, and
invalid old case-family metadata. These are planner/harness findings, not
expected-red product evidence.

## Reused execution and proof-contract rebinding

No product test was rerun merely to add catalog behavior references. Every one
of the 37 selected runners already had retained zero-exit evidence with the
current case-file digest and all 105 assigned case IDs recorded as passed.
The semantic lock names the exact reused evidence row, command, producing
commit, platform, case digest, and passed case IDs for every runner.

Adding `mapping_behavior_refs` changes the catalog portion of the current
proof contract even though the executable command, case file, invariant
behavior, and test result do not change. The transaction therefore recomputes
the proof-contract digest for all 72 affected current-contract evidence rows:
68 with an explicit `current` binding plus four legacy red/green rows whose
execution-contract-v2 digest makes their current binding unambiguous. It does
not rewrite historical/source-revision bindings, execution status, case
results, commands, commits, artifacts, or case digests.

## Builder and atomic apply proof

Proof environment:

- local canonical checkout:
  `/home/johannes/projects/trust-platform`;
- local branch: `plc-verification-program`;
- local starting HEAD:
  `9da88eb0d28a48c93432b3b4859e28a3c8cbcc27`;
- remote canonical agent-file source:
  `/home/johannes/projects/trust-platform`;
- isolated remote destination:
  `/home/johannes/projects/trust-platform-p18-remaining-142`;
- remote canonical and destination Git base:
  detached `3479807f26f56b8347d588d658cd9301d6735050`;
- evidence host: `trust-builder` (`scena-rust-builder`);
- the exact local source snapshot was synchronized without deleting the remote
  checkout;
- destination `AGENTS.md` and complete `.codex/skills/**` were manually
  recopied from the remote canonical source and verified byte-for-byte before
  validation.

The builder independently produced the same counts and assignment digest,
passed all 11 focused tests, applied the transaction, and validated the
post-map equations. The two large metadata outputs were byte-identical after
local apply:

- `verification/test-catalog.toml`, SHA-256
  `775d497f7ec95cbc4ebb98880bce1cddb97ba081570d4255d48817cbdeee3537`;
- `verification/evidence-index.toml`, SHA-256
  `ab842935d1694bfdb69d390bce71565510f6e5ff07f252df240ff4bf5e5fcc1e`;
- `verification/p18/review-locks/case-backed-behavior-101-semantic-lock.json`,
  SHA-256
  `d6ba9a27b26a3f1f7a3bc33543b4c43c3087ac343bad3bb33d44cee6b5ef0c64`.

The builder first recorded its detached validation-base commit in the semantic
lock. The lock was corrected to the canonical local source commit
`9da88eb0d28a48c93432b3b4859e28a3c8cbcc27`, copied back to the builder, and
the 11 tests plus frozen equations were rerun. After the legacy current-binding
correction, those 11 tests and the equations were rerun once more. The final
lock bytes then
matched locally and remotely. No mapping, case, evidence, or digest content
differed.

The three-file update was applied through the existing symlink-safe,
recoverable mapping transaction with exact-original preconditions and
postcondition validation:

- `verification/test-catalog.toml`;
- `verification/evidence-index.toml`;
- `verification/p18/review-locks/case-backed-behavior-101-semantic-lock.json`.

## Frozen equations

The post-transaction report is retained at
`target/gate-artifacts/verification/p18-case-backed-behavior-101/frozen-gaps-after-local.json`.

| population | before | after |
| --- | ---: | ---: |
| frozen live tests | 4,393 | 4,393 |
| `catalog_mapped` | 3,883 | 3,883 |
| `reviewed_nonmapping` | 510 | 510 |
| missing specification authority | 0 | 0 |
| owned nonmapping | 510 | 510 |
| specified gap-open cells | 216 | 216 |
| unbound specified behavior slots | 422 | 321 |
| exact-behavior-mapping debt invariants | 31 | 31 |

This is exact behavior-mapping progress using existing executable proof. It
does not count as 101 newly written tests and does not close `P18-005`,
`P18-006`, or `P18-004` globally. The 321 remaining slots must undergo the
same existing-proof refinement before any residue is treated as genuine
test-writing demand.

No broad workspace `just fmt`, `just clippy`, `just test`, or `just test-all`
was run. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
