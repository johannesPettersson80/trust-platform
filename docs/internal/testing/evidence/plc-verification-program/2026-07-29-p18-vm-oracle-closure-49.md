# Phase 18 VM and bytecode assertion-oracle closure

Date: 2026-07-29

## Scope and result

This coherent batch completed semantic assertion review and exact focused
execution for the remaining 49-test VM, bytecode, and SIZEOF population at
source checkpoint `ad2d99526a47954e2946d455e5e156960c657c5b`.

The accepted result is:

- all 49 discoveries are catalog-mapped and leave
  `assertion_oracle_unresolved`;
- all 396 extracted assertions are classified exhaustively: 162 are direct
  product oracles and 234 are setup or preconditions;
- the 162 oracle assertions have 174 exact behavior bindings;
- no assertion is accepted as a non-oracle observation, and no unresolved
  helper or assertion-empty identity remains;
- exact focused execution passes all 27 selected runtime-library tests, all 16
  selected `bytecode_vm_core` integration tests, and all six selected
  `sizeof_semantics` tests;
- one bounded `VM_STACK_EXECUTION_001` invariant records the existing bytecode
  specification authority for accepted opcode routing and canonical stack-VM
  result semantics; and
- the live denominator moves from 4,401 = 3,989 `catalog_mapped` + 412
  `reviewed_nonmapping` to 4,401 = 4,038 `catalog_mapped` + 363
  `reviewed_nonmapping`.

This is P18-003/P18-005 slice progress only. It does not close the remaining
363 nonmapping facts, dynamic reachability, mutation adequacy, the remaining
coverage-cell debt, or the final broad candidate gate.

## Assertion-authority correction

The first generated review classified bytecode-body `contains` assertions as
route setup and mapped three result-bearing tests to the broad
`RT_EXECUTION_LANGUAGE_CONFORMANCE_001` authority. Full staleness isolation
rejected that draft with six catalog-area findings across those three tests.

The complete batch was rebuilt from the unchanged source checkpoint rather
than patching those rows in place. The accepted review:

- recognizes all 25 bytecode-body opcode observations as product oracles;
- raises the oracle count from 137 to 162 and lowers setup/preconditions from
  259 to 234;
- maps opcode-route and stack-execution observations to the bytecode-native
  `VM_STACK_EXECUTION_001` contract backed by
  `SPEC_BYTECODE_FORMAT_001`; and
- removes `RT_EXECUTION_LANGUAGE_CONFORMANCE_001` and
  `RT_CORE_BYTECODE_SCHEDULER_001` from this batch's selected authorities.

The final staleness isolation reports zero findings containing any of the 49
test identities or seven selected invariants.

## Exact checkout and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Canonical branch: `plc-verification-program`
- Exact source checkpoint:
  `ad2d99526a47954e2946d455e5e156960c657c5b`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-vm2-ad2d9952`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the canonical checkout and verified before editing or
validation:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills normalized aggregate
  253377375e0009b82f732dfd26758e46b173fcedec8e0bf7bd85460946ea37e6
```

The focused builder preflight reported 31 GiB free on `/home/johannes` and
3.1 GiB free on `/tmp`. This was sufficient for the focused campaign and
reused warmed build output; no broad workspace gate ran.

## Exact focused execution

On the exact source checkpoint:

```text
trust-runtime library shard
  selected: 27
  passed:   27
  failed:   0

bytecode_vm_core integration shard
  selected: 16
  passed:   16
  failed:   0

sizeof_semantics integration shard
  selected: 6
  passed:   6
  failed:   0

total
  selected: 49
  passed:   49
  failed:   0
```

The exact live scanner found 4,401 facts across 764 source files with zero
warnings and zero errors.

Retained artifact digests:

```text
assertions-49.json
  3b68c18efc19da0601cde147a20b38ac670d629a291784da8c99271b820cfa7b
assignments-49.json
  0966ed14c51cdeac45e40f33b822f75aa257d7d8e1589743acaea8886fd0a270
live-scan.json
  c98808bbb4ef48de8656c4f0708c6d45e19713d9ec89c13a5328445fa2fab18f
execution-summary.json
  9a033ab353ed0f170b3910e1d77852089b5d7273cd3e4356af19a0bc9eb2b57c
trust-runtime JSONL
  b982315d2cc8713890b2215f55126ae771bbf56ceafcdcb92e102fa61e8b9873
bytecode_vm_core JSONL
  1812cf2949355ad5e998b6eaaabdeaa6cfc28386617cc57bce6067c683eb3478
sizeof_semantics JSONL
  000d54d7b53fade8010ef3184b84a270e7221960f19a82b4e405952a563fdd5e
```

The machine-readable review lock is
`verification/p18/review-locks/vm-oracle-49-closure-lock.json`.

## Validation and staged cadence

On the manually bootstrapped exact builder checkout:

- the atomic semantic transaction validator passes;
- every one of the 11 lock-bound metadata digests matches its live file;
- `git diff --check -- verification` passes;
- the independent root-record audit proves exactly 98 proposal/decision
  records were added and all 6,576 pre-existing records remained
  byte-for-byte identical; and
- the focused mapping suite passes 72/72:

```text
python3 -m unittest \
  scripts.verification.mapping_proposals_tests \
  scripts.verification.mapping_review_tests \
  scripts.verification.mapping_catalog_updates_tests \
  scripts.verification.mapping_review_batch_tests \
  scripts.verification.mapping_toml_tests \
  scripts.verification.mapping_transaction_tests \
  scripts.verification.test_catalog_denominator_tests \
  scripts.verification.metadata_validator.mapping_metadata_tests
  72/72 pass
```

The mandatory planner and repository-wide catalog-staleness gate remain red
from the accepted open Phase 18 backlog. The final staleness run reports 1,620
global findings and zero findings tied to this transaction's exact test or
invariant population. That global red remains debt and is not reported as a
passing gate.

The canonical local checkout also contains unrelated uncommitted runtime
edits. Its semantic replay correctly rejects stale source closure at
`crates/trust-runtime/src/config/tests.rs`; those user changes were neither
overwritten nor included in this transaction. The clean exact builder result
is the authoritative proof for this source-bound batch.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
