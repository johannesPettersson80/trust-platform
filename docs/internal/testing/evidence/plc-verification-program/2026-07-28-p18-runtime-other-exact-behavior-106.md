# Phase 18 runtime-other exact-behavior campaign

Date: 2026-07-28

## Scope and result

This coherent campaign reviewed all 106 frozen behavior demands left in the
runtime-other authority group after the ADS campaign. It bound those demands
to 196 exact catalog tests across 27 executable command groups and added the
one live breakpoint-suppression test that the frozen denominator had omitted.

The accepted transaction records:

- 106 reviewed behavior demands;
- 196 exact assigned catalog tests and 230 assignment links;
- 163 tests whose exact behavior mapping already existed;
- 34 newly exact-mapped tests, including the newly discovered breakpoint test;
- five additions to the frozen denominator;
- 12 regenerated proposal snapshots and accepted decisions;
- seven changed assertion-source closures;
- 15 updated mapping-provenance records;
- 45 touched invariant files;
- zero missing specification authority;
- 90 residual confirmed missing behavior tests.

The frozen denominator now contains 4,399 unique live facts: 3,918 are
`catalog_mapped` and 481 retain an explicit owned `reviewed_nonmapping`
disposition. The 90 residual behavior demands span 29 specified invariants.
This is large-batch progress across `P18-003` through `P18-006`; it does not
close any of those rows globally.

## Exact checkouts and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Local branch: `plc-verification-program`
- Exact execution source commit:
  `1dc849081acc06da759b56888930a36b3c6eeee2`
- Accepted metadata transaction commit:
  `d7df94e2de0df6dc7b50d4b0fad1f55080561f99`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-runtime-other-106-1dc84908`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

Before execution and before the final focused validation, the destination
`AGENTS.md` and complete `.codex/skills/**` directory were manually copied
from the canonical checkout and verified byte-for-byte. The final focused
validation checkout was clean at the exact metadata transaction commit.

## Test-first transaction proof

The mapping transaction was developed under focused tests. The initial
command was:

```text
python3 -m unittest \
  scripts.verification.runtime_other_behavior_mapping_tests
```

Before implementation it failed at the requested behavior boundary: the
expected 106-demand transaction contained zero assignments and the required
proposal, decision, catalog, denominator, and invariant replacements were
absent. No compile, dependency, harness, timeout, or unrelated failure was
counted as the red result.

After the transaction was applied, the existing proof and campaign tests
produced a second honest red: the live resolver reclassified the 33 promoted
tests as existing, changing the reconstructed pre-transaction count from 163
to 196 and removing their former discovery IDs from the campaign. Applied
state reconstruction was then bound to the immutable semantic lock instead of
silently changing the accepted campaign.

The paired local green command was:

```text
python3 -m unittest \
  scripts.verification.runtime_other_behavior_mapping_tests \
  scripts.verification.runtime_other_behavior_proof_tests \
  scripts.verification.runtime_other_behavior_campaign_tests
```

Result: 10/10 focused tests passed. The same command passed 10/10 on
`trust-builder` at the exact clean metadata commit in 8.756 seconds.

The transaction compiler validated every generated replacement before its
single recoverable apply step. It also ran canonical mapping-decision
validation across the complete decision file with zero transaction failures.
If any replacement or post-apply equation had failed, every touched metadata
file would have been restored.

The verification-tooling implementation is split by responsibility:

- `runtime_other_behavior_mapping.py`: preparation, validation, and atomic
  orchestration;
- `runtime_other_behavior_mapping_review.py`: proposal, decision, and
  assertion-review refresh;
- `runtime_other_behavior_mapping_support.py`: canonical rendering and
  replacement support;
- `runtime_other_behavior_proof.py`: immutable pre-transaction reconstruction
  from the semantic lock.

No module exceeds 1,000 lines.

## Exact builder execution

At execution source commit
`1dc849081acc06da759b56888930a36b3c6eeee2`, the unified campaign passed
196/196 exact selected tests in 27 command groups. The normalized campaign
digest is:

```text
sha256:d433d5f04a9e0d7a8522ccacc8a6496a4e97ad56a789285c08206f8c634a46f1
```

The live assertion inventory contains the exact per-test assertion locations
and expressions used during semantic review. Its digest is:

```text
sha256:372875f0b20b42ba993ae641c6107e32746e9c47a9a8df686d6493fc38b9fbae
```

The live scan digest is:

```text
sha256:4daf291d7578736aef3efeb29ddbc8e6c59dacdad47d4dfbd6307bffbef51aeb
```

The additional live test
`breakpoint_stop_suppression_is_identity_scoped` was discovered as
`DISC_1D27FB7DCC4AAEE15C39`. Its reviewed authority is
`DEBUG_CONTROL_EXECUTION_001#behavior:2`: one breakpoint identity emits at
most one stop per scan. Its four direct assertions contain no unresolved
helper. The exact builder command passed 1/1, with normalized campaign digest:

```text
sha256:f44178dbc136c630a44b07eacb4e0ab019ad53f27a50d75cf857cc91fd4abfd5
```

Changing selected assertion sources also required current review closure.
Exact builder inventories cover:

- two restart/watchdog inventories with 15 assertions,
  `sha256:13ec5a0a48790555b7f2f507152d897f7857e28b9c609f06f77555d31dfeb8a5`;
- four behavior-refresh inventories with 13 assertions,
  `sha256:7b6fe920d2f9b159bc1429cf381d4b1a2274501fe61cddea3645665937b2e500`;
- the breakpoint inventory,
  `sha256:9e07c6a3e63d9083d0f2a12f1ff786627cfa644e0f8840af3a7f70718bc934d3`.

The durable machine-readable campaign, assignment, live-scan, assertion, and
raw command artifacts are retained under:

```text
docs/internal/testing/evidence/plc-verification-program/2026-07-28/
  p18-runtime-other-exact-behavior-106/
```

Their accepted digests and the complete 196-test assignment are also bound in
`verification/p18/review-locks/runtime-other-106-semantic-lock.json`.

## Scoped and global validation state

The frozen catalog-gap analysis at metadata commit `d7df94e2` reports:

- 4,399 frozen live tests;
- 3,918 `catalog_mapped`;
- 481 owned `reviewed_nonmapping`;
- zero missing-specification-authority existing tests;
- 90 confirmed missing behavior tests;
- 216 specified `gap_open` coverage cells;
- 29 invariants still requiring exact behavior mapping.

The mandatory planner was run before implementation and exited 6 on the
unchanged repository-wide Phase 18 metadata baseline, beginning with
`VM_SEAM_DECLARED_TYPE_001` and 1,298 additional findings. The final exact
builder `check_test_catalog_staleness.py` invocation likewise exited 1 on the
still-open global Phase 18 metadata/proof baseline. Those failures are retained
as downstream board debt; neither focused campaign success nor this evidence
entry converts them into passing proof.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
