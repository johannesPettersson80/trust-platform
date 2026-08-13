# P18 verification authority-tool contract

Date: 2026-08-13
Active row: `VERIF-P18-003`

## Scope and authority

This behavior-preserving verification slice binds fourteen exact authority,
review, selection, and recoverable-transaction source paths to the existing
`VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001` contract. The authority is
`SPEC_VERIFICATION_PROGRAM_CONTRACT_001#code-to-spec-refresh-proposal-quarantine`
and the existing reciprocal mapped test is
`TEST_VERIFICATION_METADATA_GATE_001`.

The accepted paths are the seven thin authority CLI adapters plus
`catalog_authority_reassignment.py`, `code_spec_authority_candidates.py`,
`code_spec_authority_contract.py`, `code_spec_authority_review.py`,
`code_spec_authority_selection_support.py`,
`code_spec_authority_selection_test_support.py`, and
`mapping_transaction.py`. Mixed-semantics `mapping_catalog_updates.py`, the
metadata-validator graph, report extractors, and unrelated gate/inventory
tooling were deliberately excluded.

## Test-first gate registration

The focused registration assertion first failed because
`scripts.verification.code_spec_authority_candidates_tests` was not executed by
`scripts/verification_metadata_gate.sh`. Commit `2d65bc7e` registers that
module together with the authority-review, authority-selection, and mapping-
transaction suites. The same assertion then passed. The complete focused
ownership suite passed 65/65 on `scena-rust-builder`:

```text
python3 -m unittest \
  scripts.verification.catalog_authority_reassignment_tests \
  scripts.verification.code_spec_authority_candidates_tests \
  scripts.verification.code_spec_authority_reuse_tests \
  scripts.verification.code_spec_authority_review_tests \
  scripts.verification.code_spec_authority_selection_tests \
  scripts.verification.mapping_transaction_tests
```

## Identity refresh and reviewed selection

The gate registration changed only the metadata-gate entrypoint body digest.
The reviewed one-rebind refresh was applied atomically and committed as
`8622b0dd`; no fact was added, removed, or promoted by that refresh.

The clean report at `8622b0dd` contains 17,491 facts and zero findings. The
authority-reuse proposal is
`sha256:28c686c5dd0b0e6a1cb40b8bac39955cd8873aaecf474d3252fd34e73d51f8db`
with operation-set digest
`sha256:da1cede28166782311b258fd916a6a3805460c2c10d4471ccc8813db30da1bed`.
The complete reviewed selection is
`sha256:bf34c6c85be4ad2a63638cd14bc6b06d3f42b9f606deef9f2bf60daa26203764`.
It carries forward all 24 prior ambiguity, mixed-semantics, and claim-mismatch
withholds and accepts only 14 singular verification groups containing exactly
115 facts.

An independent read-only audit verified the report, proposal, selection,
source commit, operation plane, exact path and fact sets, reciprocal authority,
and all 51 current risk acknowledgements. It caught and blocked a draft review
reference to an untracked future evidence file. The corrected selection binds
the existing normative metadata-model anchor; a fresh live `--check` passed
before mutation.

The atomic application is
`sha256:4af7b26951c57be23544c9be3c958325378a18bfce8a8cb82240beda24118312`.
It reviewed 38 groups, accepted 14, withheld 24, promoted 115 mappings, changed
only `verification/code-spec-map.toml` and `verification/spec-gaps.toml`, and
produced code-map digest
`sha256:a55e22db4f7f7e74ad795cd3e6310be8e76661c76970c2d797c87dcb5242421c`.
Commit `07e58204` records the recoverable authority checkpoint.

## Final focused evidence and boundary

The final source-bound report is
`sha256:d16ef8b1009725f60f5d90f490830f874d144c2cb280bd576eaba5aeaf2bf29c`:
17,491 facts, zero findings, 13,720 `spec_gap`, and 3,511 `specified`. The
verification area now contains 3,515 remaining gaps. The focused suite passes
65/65 and diagram drift is zero on the isolated builder.

This closes only code-to-authority mapping debt for the selected tooling. It
does not establish new product behavior, execution reachability, mutation
sensitivity, adequacy, or broad candidate proof. `VERIF-P18-003` remains open,
and no `VERIF-P18-016` broad gate ran.

## SOLID, KISS, and DRY review

- SOLID: proposal construction, semantic review, selection validation, and
  recoverable mutation remain separate source owners.
- KISS: the existing exact-path authority-reuse route was used without a new
  promotion mechanism.
- DRY: the canonical invariant, specification fragment, mapped test, risk
  acknowledgements, live reconstruction, and atomic transaction were reused.

## Bootstrap

Canonical source and local destination:
`/home/johannes/projects/trust-platform`, branch
`plc-verification-program`. Isolated validation destination:
`trust-builder:/home/johannes/projects/trust-platform-p18-portable-d53471c2`.
Both matched each frozen commit. `AGENTS.md` matched
`sha256:76576b5464443f4128a67c325527d23e59da46c019da7b79f39454dabc250263`;
the complete `.codex/skills/**` aggregate matched
`sha256:3f84c064eff30aca31d55c51034a05a7e2ae4a53d2c69fb6f1827585b7ac73a7`.
