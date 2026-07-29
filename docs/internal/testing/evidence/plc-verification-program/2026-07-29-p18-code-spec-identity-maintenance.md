# Phase 18 code-spec identity maintenance

Date: 2026-07-29

Accepted metadata commit:
`0b2495babdbc5604a74f2239859feb223352be25`

## Outcome

The clean source scan at
`4faa9a082ce446a463f2cd1c2555597c4d5d1899` contained 17,178 code
facts and 795 findings. The authenticated proposal partitioned every finding
into 48 missing mappings, 165 body-digest rebinds, 40 stale identities, and
36 semantic review groups, with zero unclassified findings.

One reviewed, live-reconstructed, compare-and-swap transaction:

- mapped 47 new facts directly to explicit historical `spec_gap` records;
- mapped one new fact through the exact stale predecessor for
  `matches_breakpoint`;
- accepted all 165 exact-identity body-digest rebinds;
- transferred 19 stale identities to exact successors;
- removed 21 stale identities with no live successor;
- rewrote the code map, risk ledger, and gap ledger as one canonical
  replacement set.

At accepted commit
`0b2495babdbc5604a74f2239859feb223352be25`, the clean report contains
17,178 live facts and 17,178 mapped facts. There are zero missing mappings,
zero digest rebinds, zero stale identities, and zero unclassified findings.
The remaining 306 findings are wholly partitioned into 33 semantic review
groups.

This closes the identity-maintenance queue only. The accepted report still
contains 15,492 `spec_gap` facts, so `VERIF-P18-003` remains open. This batch
does not establish specification authority, product-test proof, execution,
reachability, mutation sensitivity, adequacy, or final broad-gate proof.

## Identity and risk review

The reviewer accepted all 48 missing-mapping decisions only after rebuilding
the complete source proposal from the live checkout. Forty-three new
verification-tool facts remain explicitly mapped to the historical
verification gap. One new `matches_breakpoint` fact inherits the reviewed
mapping of its exact stale predecessor.

The four cross-area facts received explicit semantic ownership:

- `DebugControl::begin_cycle` is owned by `trust-runtime` in
  `control_security`;
- `DebugAdapter::projected_threads` is owned by `editor` in
  `editor_safety`;
- `buildGraph::topologyIsProven` is owned by `hmi` in `hmi_ui`;
- `ResourceRunner<C>::restart_with_retain_store` is owned by
  `trust-runtime` in `runtime_safety`.

Each remains a `spec_gap`; selecting its owner is not a claim that its
behavior is specified or tested. Risk acknowledgements bind the exact code
identity, body digest, owner, area, and observed signals, and expire on
2026-10-29. The complete decision set is frozen in
`verification/p18/review-locks/code-spec-identity-maintenance-lock.json`.

## Transaction and rejection evidence

The source transaction is bound by:

- report:
  `sha256:d74c1f3ecbd31b5613b11400f454259b19896586635a33dba556678d1bce2005`;
- source-input digest:
  `sha256:8c74aacfdeedbf9ab80d6d2445f2a28430ad24886dcc123de56c5ecb6aaabffa`;
- proposal:
  `sha256:0b70d71733dc928d0c130f5fc99ff04e24393b1ca36d21f8aa52a1bfb6ec4ea8`;
- operation set:
  `sha256:830425f1fd0868b16a03bfcbba237766677f6866297998a6bb771f7093a8d344`;
- reviewed selection:
  `sha256:d2f4ec9a762d7885824635f8ad177776341094bf9f2a3b6e6057d3cca3e72ada`;
- application manifest:
  `sha256:a75210414b9a3a8d556d0c958c219d71a3c5157ec75753189c4b06bb2177c243`.

Before the accepted application, the complete live proposal was rebuilt in
check mode and rebuilt again immediately before projection and application.
The first projected replacement set was rejected because `git diff --check`
found an extra blank line at EOF in all three generated TOML files. None of
those bytes were copied to the canonical checkout. The renderer was fixed,
the source report and authenticated selection were regenerated, and the
complete transaction was repeated from the frozen source.

The accepted transition changed the canonical digests as follows:

| Metadata | Before | After |
| --- | --- | --- |
| code map | `sha256:7e1ab1517f39db67d2536e0ce0cf2ab28248ea55f86978161f3946c958923ea3` | `sha256:8db45eb3b9f92ff04c50b74298f3bdbbaf46eaa1486ebfb2547f3b1472c54097` |
| risk ledger | `sha256:e953bae8fb2f3068b4ea75014083cbd4bf27ffca575a9e29cfd33b54279423d7` | `sha256:58ff3e0373b7fe2cb6ad8fdb1e09513ef08369274510c5b11b00db32d0d9ad76` |
| gap ledger | `sha256:c7abfb5766712304ae0e91f352169209336e61c3c68290a24768b361efbd7e23` | `sha256:0bd4681bfcdcdfc02ca1fdfce3a2a8e76a9fcc8a5a4fc613da325f08d5c3e281` |

## Test-first tool proof

The acceptance tool was built through focused red-green slices:

1. `test_projection_contract_is_available` failed because the projection
   contract did not exist, then passed after the projection boundary was
   implemented.
2. `test_application_contract_is_available` failed because the application
   contract did not exist, then passed after live reconstruction and
   compare-and-swap application were implemented.
3. `test_metadata_gate_registers_refresh_acceptance_tests` failed because the
   new module was not invoked, then passed after registration in the real
   metadata gate.
4. The live queue exposed a pre-mapped successor transfer that the initial
   projection rejected. The focused successor case reproduced the exact
   failure and passed after transfer support was added.
5. The generated EOF case reproduced the three malformed TOML endings and
   passed after canonical rendering was corrected.

The complete focused suite:

```text
python3 -m unittest \
  scripts.verification.code_spec_refresh_tests \
  scripts.verification.code_spec_refresh_selection_tests \
  scripts.verification.code_spec_risk_tests \
  scripts.verification.metadata_validator.code_spec_tests \
  scripts.verification.spec_gap_closure_tests
```

passed 45/45 on the clean detached builder checkout.

## Accepted remote proof

Canonical checkout:
`/home/johannes/projects/trust-platform`

Clean validation checkout:
`trust-builder:/home/johannes/projects/trust-platform-p18-marker-0d31ff12`

Bootstrap:

- remote host: `scena-rust-builder`;
- destination HEAD:
  `0b2495babdbc5604a74f2239859feb223352be25`;
- root `AGENTS.md`: exact match to the remote primary checkout;
- `.codex/skills/**`: 25/25 files, exact recursive match.

The accepted report is
`sha256:a219f806db25eed0417060eb03c5cacac843711f4efd4229fc310c9afb4ea397`
with input digest
`sha256:dce2380428f00d560ab1c43bc818a239d0b4e97c802bf2bdf34e3f9fde0ece12`.
Its dispositions are 15,492 `spec_gap`, 1,651 `specified`, 30
`inherits_spec`, two `generated`, two `non_behavioral`, and one
`vendored_external`.

The post-transaction proposal is
`sha256:a4561c26f9fda0c7e0a9dfefaa460d59928727cccc2fd63b2219c6c4fbcc8f02`
with operation-set digest
`sha256:aede969659f527ec7a590a03f7290d2194d43cd650c1bf767f48a70c00c621a6`.
It has zero identity-maintenance proposals and partitions all 306 findings
into 22 invariant-to-test relation groups, ten cross-area decision groups,
and one test-to-invariant relation group.

The metadata validator retained the exact accepted downstream-debt baseline:
1,610 findings,
`sha256:cdb6cbc9d85f22f8cf593d99ea8ed47cd7e69147f684064ba4953068bb6c1cbb`.
The complete changed-path planner exited 6 on the same known global Phase 18
metadata debt (1,877 findings), with output digest
`sha256:e355e9ff2e0344f2f960c772b486ba32670a135ae063ea49029605d6cdac7f55`.

No `just fmt`, `just clippy`, `just test`, `just test-all`, coverage campaign,
mutation campaign, or final broad gate ran.
