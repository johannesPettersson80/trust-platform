# Phase 18 OpenOT assertion-oracle closure

Date: 2026-07-29

## Scope and result

This coherent batch completed semantic assertion review and focused execution
for the exact 38-test OpenOT population present at source checkpoint
`02f5311d053ba5dc3a2686c161853218a492af0c`:

- 32 existing accepted catalog mappings were rebound to the current complete
  assertion/source closure;
- six `reviewed_nonmapping` discoveries received accepted mappings;
- all 620 unique assertions were classified exhaustively: 293 are
  product-oracle assertions and 327 are setup or preconditions;
- no assertion was accepted as a non-oracle observation, and no unresolved
  helper or assertion-empty inventory remained;
- 37 exact `openot_telemetry` tests and the one exact fenced cross-process
  `openot_capstone` test passed;
- all 14 structural coverage cells across the five owning OpenOT invariants are
  catalog-covered; and
- the live denominator moved from 4,401 = 3,983 `catalog_mapped` + 418
  `reviewed_nonmapping` to 4,401 = 3,989 `catalog_mapped` + 412
  `reviewed_nonmapping`.

The review also removed one inherited overmapping. The
`openot_telemetry_authoring_sampling_policy_round_trips_definition` assertions
prove canonical authoring translation, but do not independently prove runtime
event-definition resolution. Its accepted contract now binds only
`RT_OPENOT_AUTHORING_TRANSLATION_001`. The separate typed-audit-log test
continues to cover the event-resolution behavior, so the correction loses no
behavior coverage and removes the catalog's cross-area validation error.

This is P18-003/P18-005 slice progress only. It does not close the remaining
412 nonmapping facts, dynamic reachability, mutation adequacy, later
environment campaigns, or the final broad candidate gate.

## Exact checkout and bootstrap

- Canonical agent-file source:
  `/home/johannes/projects/trust-platform`
- Canonical branch: `plc-verification-program`
- Exact source checkpoint:
  `02f5311d053ba5dc3a2686c161853218a492af0c`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-oracle-b636380d`
- Builder host: `scena-rust-builder`
- Builder checkout mode: detached HEAD

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the canonical checkout and verified byte-for-byte before
the evidence campaign:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills normalized aggregate
  253377375e0009b82f732dfd26758e46b173fcedec8e0bf7bd85460946ea37e6
```

The focused builder preflight reported 35 GiB free on `/home/johannes` and
1.6 GiB free on `/tmp`. That is sufficient for this focused Python/OpenOT
campaign; no broad Rust gate ran.

## Semantic review

The exact assertion inventory contains:

```text
38 tests
620 unique assertions
0 unresolved helpers
0 assertion-empty identities
```

The review assignment is exhaustive:

```text
293 oracle_bound
327 setup_or_precondition
0 non_oracle
```

Setup assertions remain recorded but do not count as product-oracle proof.
Five helper-origin assertion identities changed because the frozen helper
source moved; those identities were translated only when the source path,
function, assertion kind, expression digest, and expression uniquely matched.

The accepted transaction refreshes five invariant files, the catalog,
denominator, proposal queue, decision ledger, and one commit-bound review lock.
The decision diff was independently audited at root-record granularity:

```text
32 refreshed discoveries
6 introduced discoveries
64 superseded proposal/decision root records
76 replacement/new proposal/decision root records
6,500 unrelated root records byte-for-byte identical
```

The machine-readable review lock is
`verification/p18/review-locks/openot-38-oracle-closure-lock.json`.

## Exact focused execution

On exact source checkpoint `02f5311d`:

```text
OpenOT telemetry shard
  selected: 37
  passed:   37
  failed:   0
  elapsed:  117.354 seconds

OpenOT fenced cross-process capstone
  selected: 1
  passed:   1
  failed:   0
  elapsed:  37.193 seconds
```

The exact live scanner found 4,401 facts across 764 source files with zero
warnings and zero errors.

Retained artifact digests:

```text
assertions-38.json
  bc2603e763f9b6d6023ce6a1fdb1b2099d4214cee75fc6c2d41612dcec6c35e6
assignments-38.json
  da3f41d80c176874119225e14b047b2fb76276fc9008967e31854172e972f612
live-scan.json
  18c4c1e9eb37fc434e084ee074ffd2b8a0585dcf6071795eca733e59d7a3f317
openot-telemetry-37.jsonl
  e584c775510fe90ed640ce7c0495ad280a3486d87d520bff83dc7846e8f5be38
openot-telemetry-37.results.json
  db68ee3498d6e85e67ed03cfc76703d229eb3e88df0965a15af03e8e4890c2da
openot-telemetry-37.execution.json
  4c1455c6ab140999c82cb740f3a92a19598f9251a5bd15f60622558575f2bd11
openot-capstone-1.jsonl
  f8250069868eb619eca6b8d289de337fd92467e1e33d140d52a825bf768ecf27
openot-capstone-1.execution.json
  bd4a711bb9f46319f2a6e82a49964650d880ab4b424ba2c83b3e608547038cb2
```

## Validation and staged cadence

The focused semantic transaction validator passes after the area correction.
The decision-ledger byte audit passes, `git diff --check -- verification`
passes, and every lock-bound metadata digest matches its live file.
After syncing only this transaction to the manually bootstrapped builder
checkout, the remote semantic transaction validator also passes and the
focused mapping suite passes 72/72:

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

The repository-wide catalog-staleness gate remains red from the accepted,
still-open Phase 18 metadata/proof backlog. After the area correction, its only
line containing `OpenOT` is the pre-existing
`TRUST_HIR_OPENOT_AUTHORING_001` unsupported `specified` status; no fact in
this 38-test transaction contributes an OpenOT mapping-staleness error. The
global red is retained as debt, not reported as a passing gate.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
