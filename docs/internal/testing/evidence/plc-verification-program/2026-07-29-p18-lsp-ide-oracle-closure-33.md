# Phase 18 LSP and IDE assertion-oracle closure

Date: 2026-07-29

## Scope and result

This coherent batch completed semantic assertion review and exact focused
execution for 33 portable LSP and IDE tests selected at
`55b79fa53e671bd85b612d9b212252d642c3b768`.

The accepted result is:

- all 33 discoveries are catalog-mapped and leave
  `assertion_oracle_unresolved`;
- all 270 extracted assertions are classified exhaustively: 118 are direct
  product oracles and 152 are setup or preconditions;
- the direct oracle assertions have 118 exact behavior bindings;
- no assertion is accepted as a non-oracle observation, and no unresolved
  helper or assertion-empty identity remains;
- exact focused execution passes 27/27 selected `trust-lsp` tests and 6/6
  selected `trust-ide` refactor regression tests;
- seven bounded editor-safety invariants establish project-configuration,
  document-state, HMI-authoring, cache-identity, presentation-feature,
  telemetry, and source-refactor authority;
- all newly introduced coverage cells reconcile to accepted mappings, so the
  repository-wide `gap_open` count remains 203; and
- the live denominator moves from 4,401 = 4,115 `catalog_mapped` + 286
  `reviewed_nonmapping` to 4,401 = 4,148 `catalog_mapped` + 253
  `reviewed_nonmapping`.

This is P18-003/P18-005 slice progress only. It does not close the remaining
253 nonmapping facts, dynamic reachability, mutation adequacy, the 203
remaining coverage-cell debts, the Windows-only platform lane, or the final
broad candidate gate.

## Specification and invariant authority

No product code changed in this batch. The accepted tests bind the active
`SPEC_LSP_CONTRACT_001` source through these seven new invariants:

```text
IDE_SOURCE_REFACTOR_001
LSP_DOCUMENT_STATE_001
LSP_HMI_AUTHORING_001
LSP_INDEX_CACHE_001
LSP_PRESENTATION_FEATURES_001
LSP_PROJECT_CONFIGURATION_001
LSP_TELEMETRY_001
```

The authority covers:

- exact project roots, local and pinned Git dependencies, lock/offline/host
  policy, vendor defaults, and dependency-graph diagnostics;
- full and incremental document synchronization, lifecycle transitions, and
  bounded closed-document eviction;
- HMI scaffold and binding commands plus descriptor diagnostics;
- cache round-trip, content invalidation, and same-size/same-second collision
  rejection;
- canonical formatting and active-parameter signature help;
- opt-in aggregated JSONL telemetry; and
- coherent semantics-preserving extract, stub, inline, conversion, and
  namespace-move workspace edits.

The pre-existing broad `LSP_CORE_FEATURES_001` metadata findings remain global
Phase 18 debt. This batch does not rewrite its previously accepted mappings or
claim that unrelated authority is valid.

## Platform boundary

Two Windows-only URI tests were deliberately excluded from the portable Linux
batch:

```text
path_to_uri_strips_extended_length_prefix
uri_to_path_decodes_drive_letter
```

The Linux assertion extractor resolved zero syntax nodes for those
platform-gated bodies. They remain explicit `assertion_oracle_unresolved` debt
for the Windows authority/execution lane; they were not silently retired or
represented as Linux proof.

## Exact checkout and bootstrap

- Canonical local program checkout:
  `/home/johannes/projects/trust-platform`
- Canonical local branch: `plc-verification-program`
- Exact accepted selection checkpoint:
  `55b79fa53e671bd85b612d9b212252d642c3b768`
- Remote primary agent-file source:
  `trust-builder:/home/johannes/projects/trust-platform`
- Remote primary source state during bootstrap:
  detached at `3479807f26f56b8347d588d658cd9301d6735050`
- Validation destination:
  `trust-builder:/home/johannes/projects/trust-platform-p18-lsp-55b79fa5`
- Builder host: `scena-rust-builder`
- Validation checkout mode:
  detached HEAD at the exact selection checkpoint

The destination `AGENTS.md` and complete `.codex/skills/**` directory were
manually copied from the remote primary checkout before validation. Exact
verification passed:

```text
AGENTS.md
  76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263
.codex/skills
  diff -qr against the remote primary source: no differences
```

The required destination copies of `trust-test-authoring`,
`trust-remote-builder`, `st-lsp-solid`, and `trust-release-hygiene` were read
before the transaction and focused validation.

The scanner checkpoint records
`dirty:55b79fa53e671bd85b612d9b212252d642c3b768` because the seven new,
reviewed invariant files were present when the source scan was captured. The
transaction preflight independently required exact HEAD `55b79fa5`, exactly
those seven untracked authority files, and no other checkout changes before it
accepted that checkpoint.

## Source and execution chain

Exact execution used `cargo nextest` expressions that name every selected
binary and test identity:

```text
trust-lsp binary
  selected: 27
  passed:   27
  failed:   0

trust-ide refactor_regression
  selected: 6
  passed:   6
  failed:   0

total
  selected: 33
  passed:   33
  failed:   0
```

Ten unrelated ignored performance tests discovered while listing
`trust-lsp` were not selected or counted as passing.

Retained core artifact digests:

```text
assertions-33.json
  347411c90acb039f8682ad8ac6cf050d6c698ef5e3aba6c6d613cda92cc70e34
assignments-33.json
  8900af7c321016f6af6a769b77c56975e5dc65f6ac014d5a8a821cf9acbb00b1
live-scan.json
  4cc677d9700fcc549563d2e44af156ff6a1de1ad441c94aa37bad79aa871c0ef
execution-summary.json
  5fbd78afe1ca014591cdae9dfdcc3542c445a5f79d2097dd3e659a07f0157235
staleness.txt
  45fc605ff356aa891fa1d61214066d83585082e979878638bacce913294241c4
```

The machine-readable review lock is
`verification/p18/review-locks/lsp-ide-oracle-33-closure-lock.json` with
digest
`2d63fe1f7bb84a1f453ad0fc726e8385e08f5e5342e65f203259ab907ade557e`.

## Validation and staged cadence

On the manually bootstrapped exact builder checkout:

- the atomic semantic transaction passes;
- all 11 lock-bound metadata digests and all eight lock-bound artifact digests
  match their live files;
- all seven new invariants have only `covered` catalog-coverage cells;
- the repository-wide `gap_open` count is exactly 203;
- `git diff --check` passes;
- an independent root-record audit proves exactly 33 discoveries and 66
  proposal/decision records were added while all 6,828 pre-existing root
  records remained byte-for-byte identical; and
- the focused mapping suite passes 72/72:

```text
python3 -m unittest \
  scripts.verification.mapping_catalog_updates_tests \
  scripts.verification.mapping_proposals_tests \
  scripts.verification.mapping_review_batch_tests \
  scripts.verification.mapping_review_tests \
  scripts.verification.mapping_transaction_tests \
  scripts.verification.test_catalog_denominator_tests \
  scripts.verification.mapping_toml_tests \
  scripts.verification.metadata_validator.mapping_metadata_tests
  72/72 pass
```

The repository-wide catalog-staleness gate remains red with 1,620 accepted
global Phase 18 metadata findings. Full isolation checks 106 batch identifiers
and finds zero target-linked findings. That global red remains debt and is not
reported as a passing gate.

The mandatory complete changed-path planner was also run with `--intent docs`
and every one of the 14 changed paths in this batch. It exits 6 before
emitting a plan because the same 1,620 global metadata findings block
planning; the planner stderr has digest
`674406f637ff94adb590caf9b37ec8b26ad1262a08e86cac09e95c5af98b4e6b`.
The same 106-identifier isolation finds zero batch-linked planner findings.
The planner's global red is retained as open Phase 18 debt and is not treated
as a passing result.

The canonical local checkout contains unrelated uncommitted runtime edits.
Their complete binary diff digest remained
`01458d43c50cd7dfe5cc6eb4649ac1a5a7b8595e61e26d2ce3e76c0b52de4fb4`
before and after the accepted metadata copy. Those user changes were neither
overwritten nor included in this transaction; the clean exact builder result
is the authoritative source-bound proof.

No workspace-wide `just fmt`, `just clippy`, `just test`, or `just test-all`
command ran. Those remain reserved for the single frozen candidate at
`VERIF-P18-016`.
