# Phase 18 singular-area code-spec refresh

Date: 2026-07-29

Accepted metadata commit:
`86e7b67d1a50c9df9f96e25a8802d1c535895deb`

## Outcome

The frozen clean code scan at
`667511e13be16603a04eac4ffa120ca373cdde6a` reported 17,135 live
behavior-bearing code facts, including 1,755 facts absent from
`verification/code-spec-map.toml`. The authenticated refresh proposal
partitioned all 4,209 findings and exposed 1,750 missing facts with exactly
one reviewed area candidate plus five facts with multiple area candidates.

One deterministic transaction added only the 1,750 singular-area facts as
explicit `spec_gap` mappings. It also:

- added each fact to the corresponding historical gap's
  `affected_code_ids`;
- added one exact body-, owner-, area-, risk-, and expiry-bound ledger record
  for each of the 915 risk-bearing facts;
- retained the exact five cross-area facts as `missing_spec`;
- left all 165 body-digest rebind proposals, 40 stale identities, and 36
  semantic review groups unaccepted.

The accepted clean builder report contains 17,130 mapped live facts and five
unmapped live facts. Its semantic dispositions are 15,447 `spec_gap`, 1,648
`specified`, 30 `inherits_spec`, two `generated`, two `non_behavioral`, one
`vendored_external`, and five `missing_spec`.

This refresh closes no specification or proof debt. It makes the live
inventory current so later `VERIF-P18-003` authority batches can operate on
the complete denominator.

## Selection and risk review

The accepted selection rule was:

> Select a `missing_mapping` proposal only when its fact area is not
> `unmapped` and `area_candidates` is exactly the one-element array containing
> that same area.

The 1,750 selected facts split as follows:

- verification: 1,680
- runtime safety: 59
- compiler IEC: 3
- bytecode/VM: 2
- control/security: 2
- editor safety: 2
- PLCopen/devtools: 1
- protocols: 1

For the 915 selected facts carrying scanner-derived risk signals, the owning
area reviewed the exact body digest and complete risk-signal set. Each
acknowledgement expires on 2026-10-29. An acknowledgement records risk
awareness only; it does not specify behavior, suppress a risk, or promote an
invariant.

The complete selection is bound by
`verification/p18/review-locks/code-spec-refresh-missing-1750-lock.json`.
Its selected-row digest is
`sha256:8b00f466c3da295f0a0b62ca0b87e1022b6ae6ed5cb501ea6144bd8b71576861`.

## Withheld cross-area facts

These five identities remain `missing_spec` because choosing among their live
area candidates requires semantic review:

- `CODE_12529488FB28AD44368D757E3A823175B9793BED086360A75F80F4FAD1F4AD50`
  — `trust_runtime::debug::breakpoints::matches_breakpoint`
- `CODE_1E6C72B1F6F9957D36401CCA2F25CC9E8E61268AB971570E480ADB5D44807D00`
  — `trust_runtime::debug::control::DebugControl::begin_cycle`
- `CODE_34AAC26C55C189E9D09CFD2BBA753D60C13B989C9D90AA00B755E401F7034BA1`
  — `trust_debug::adapter::handlers::threads::DebugAdapter::projected_threads`
- `CODE_861021D61F6561C125BFAE8EBC5F9B46B069FE15476E8D25DB03EF3C2A1BB447`
  — `editors::vscode::src::networkCanvas::webview::layout::buildGraph::topologyIsProven`
- `CODE_B9041EE3336C09EEAFFF72802D7446D6DD27BA5F8B33BBCD65FAEB4A686B814A`
  — `trust_runtime::scheduler::ResourceRunner<C>::restart_with_retain_store`

## Evidence

Canonical checkout:
`/home/johannes/projects/trust-platform`

Clean validation checkout:
`trust-builder:/home/johannes/projects/trust-platform-p18-marker-0d31ff12`

Bootstrap at the accepted commit:

- remote host: `scena-rust-builder`
- destination HEAD:
  `86e7b67d1a50c9df9f96e25a8802d1c535895deb`
- root `AGENTS.md`: exact match to the remote primary checkout
- `.codex/skills/**`: 25/25 files, exact recursive match
- private hook:
  `sha256:be49797cabae8807d39c41e3c0d75b1dd527ada4e2c8889120aab6d7f7ec911d`

Expected red baseline:

- source report:
  `sha256:5bd17a14a6ec0ab1f9f8807133ecfff3b6b7312a71a0e4ea0e85c7d46a8c8c97`
- authenticated refresh proposal:
  `sha256:a8e420cc3216406516991b239d8547f4381ad94c9bbb45a6fe9c83eef3fbedea`
- proposal operation set:
  `sha256:2dc0bd4a726ccb7dc2b34ae3bf5f8ad82eee5090eb0852db867072acd7832eef`
- 17,135 live facts
- 1,755 `missing_spec`
- 13,697 `spec_gap`
- 4,209 findings

Focused accepted proof:

- `python3 -m unittest scripts.verification.code_spec_risk_tests
  scripts.verification.code_spec_refresh_tests
  scripts.verification.metadata_validator.code_spec_tests
  scripts.verification.spec_gap_closure_tests`
  passed 35/35 on `trust-builder`.
- `python3 scripts/code_spec_coverage_gate.py --mode report --out-dir
  target/gate-artifacts/p18-code-spec-refresh-86e7` produced 17,135 facts,
  709 findings, 17,130 mapped facts, and exactly five `missing_spec` facts.
- accepted report digest:
  `sha256:fd7ddd68c1e448abb9e5f6eff053f484b85d1f8771d9f63b2ae49e7fcf0b542b`
- regenerated post-refresh proposal:
  five missing mappings, 165 digest rebinds, 40 stale identities, 36 semantic
  groups, zero unclassified findings; digest
  `sha256:9a5a7078ba0a01fec467f65bf058c22de30c8e639b5cb1c71d2f37349cdbe449`
- `scripts/validate_verification_metadata.py` retained the exact pre-batch
  1,610-finding log byte-for-byte:
  `sha256:cdb6cbc9d85f22f8cf593d99ea8ed47cd7e69147f684064ba4953068bb6c1cbb`.
- the complete changed-path planner remained blocked only by the known global
  Phase 18 metadata ledger: exit 6, digest
  `sha256:e355e9ff2e0344f2f960c772b486ba32670a135ae063ea49029605d6cdac7f55`.

No `just fmt`, `just clippy`, `just test`, `just test-all`, coverage campaign,
mutation campaign, or final broad gate ran.
