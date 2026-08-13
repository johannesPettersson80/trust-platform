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

## Metadata-integrity entrypoints

The adjacent exact-source batch found that
`VERIFICATION_METADATA_INTEGRITY_001` named the whole verification contract,
while the reuse route correctly requires `SOURCE#fragment`. The focused test
failed for that missing fragment. Commit `76834847` binds the already-existing
`#verification-metadata-model` heading; the same test and the full reuse suite
then passed 11/11.

The clean report produced two singular groups for only
`scripts/validate_verification_metadata.py::<entrypoint>` and
`scripts/verification_metadata_gate.sh::<entrypoint>`. The complete 26-group
selection accepts those two for `VERIFICATION_METADATA_INTEGRITY_001` and
retains the 24 prior withholds. The atomic application is
`sha256:60409e658a818e7c7341c0e4ca72f768222eafddb2bfccac429e9c07d77302e2`;
commit `066619f0` records the mapping checkpoint. Its final report is
`sha256:4f9e9650201491a8b607f4d81d2ebf88d48b4756af50b1a2e3d38ddbd86a7d54`:
17,491 facts, zero findings, 13,718 `spec_gap`, and 3,513 `specified`.
Producer-authentic lock evidence and the final remote gate remain explicit
invariant debt.

## Exact gate-surface entrypoint routing

The broad `scripts` source reference of
`VERIFICATION_GATE_SURFACE_EXECUTION_001` cannot safely drive path-wide reuse:
five Python gate paths contain 21 internal helper facts in addition to their
five entrypoints. Commit `763e9ce0` adds a focused code-identity provider. It
joins invariant test ID to mapped catalog discovery, unique gate-inventory
row, live gate-script scan, and one exact Python or shell entrypoint. Workflow
jobs and same-path helpers cannot enter this route. The expected missing-
provider red and paired green pass; the live repository fixture resolves 29
entrypoints (24 shell and 5 Python) and zero helpers.

The identity transaction transfers the signature-changed proposal-builder
fact without losing its existing authority, rebinds two changed bodies, and
registers six new provider facts in the historical verification gap. Its
application is
`sha256:d6228c0829faeccd0a31fa57674dc668849a3b9be941fbbd11d4c42d37ccdb94`;
commit `2b417235` is the clean 17,497-fact checkpoint.

The clean report is
`sha256:a8a89df4bd1c1afe75cb1b1bbf74feb7b00fc5b0a7a1c52d59a7829d64153ef5`.
The complete proposal is
`sha256:5d6dd82500e471873337b33bb1fa07002e510d54c82a2aa217f2b16daa8e0556`
with operation-set digest
`sha256:64a4ac4640c1146410d9d492342897ab68dccd350a7ea023be2c78a40af5ba55`.
The independently audited selection is
`sha256:5bd3f1616621833cb22b7a47c6412b107dd6de0e51779ac1bb469d944b609b33`:
53 complete decisions, 29 accepted singular entrypoint groups, and the prior
24 withholds unchanged. The atomic application is
`sha256:8e92c41d4e8f0faaf9e42b5aa0c476c6380e1ad701f7d0b1b365ebddf03c8d01`;
commit `20841002` records the exact mappings.

After the promotion, the repository behavior-lock fixture was corrected to
reconstruct the reviewed pre-promotion dispositions rather than expecting live
facts to remain gaps. All 38 focused gate/reuse/inventory/reassignment tests
and diagram drift pass. The final source-bound report at `24ff8fdc` is
`sha256:f688320a63380b9e4da5e05f93fb7cb2fb5f8f65d3a7d06a5ab8ddff1cccb072`:
17,497 facts, zero findings, 13,695 `spec_gap`, and 3,542 `specified`.

The gate invariant remains `S0`; this batch does not claim source-bound native
execution, GitHub job artifacts, transitive product proof, or the final remote
candidate gate.

## Gate-route provider authority

The provider introduced six new behavior-bearing facts. An expected focused
red proved its source path was absent from the recoverable authority contract;
the paired green binds it to
`VERIFICATION_CODE_SPEC_REFRESH_QUARANTINE_001`. Proposal generation after the
29 entrypoint promotions then exposed a lifecycle error: a completed specified
entrypoint was treated as a missing route. A second expected red and paired
green make an exact already-specified entrypoint a no-op while still failing on
missing or duplicate identities.

The provider body refresh application is
`sha256:ea1b6c68c7a0597bae86545fec1dc544586a92a26c0f486dbe41af21d97f24ee`.
At the clean `344ef76b` checkpoint, the complete authority proposal contains
the unchanged prior 24 groups plus one singular six-fact provider group. An
independent audit verified the exact source, authority, test, risk, report,
proposal, and selection bindings. The atomic six-fact application is
`sha256:a26633853c8c69c62cb54196171ea6029cceb7419f0e8047501a2e7eacfbf5ff`;
commit `047462ae` records the authority checkpoint.

The final report is
`sha256:98f737bca512e8468693b42adc889fe1321cfd769abe2499672e21375532ea7a`:
17,497 facts, zero findings, 13,689 `spec_gap`, and 3,548 `specified`. The
focused gate-route, authority-reuse, gate-inventory, and catalog-reassignment
suites pass 40/40 and diagram drift is zero. This still closes only P18-003
authority debt for the selected facts.
