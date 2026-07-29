# Phase 18 PLCopen interchange contract ownership

Date: 2026-07-29 through 2026-07-30

## Outcome

The former `crates/trust-plcopen/src/plcopen/contracts.rs` mixed public
interchange declarations across profile, export, import, migration, vendor
shim, and POU semantics. Its 21 code facts could not honestly reuse one
specification authority, and 16 public declaration facts were not dynamically
attributable through line coverage.

The declarations were partitioned into six flattened include files without
changing the crate's public module surface:

- `contracts_profile.rs`;
- `contracts_export.rs`;
- `contracts_import.rs`;
- `contracts_migration.rs`;
- `contracts_shims.rs`;
- `contracts_pou.rs`.

`contracts.rs` retains shared constants and private internal state. The split
gives every public contract declaration one exact semantic ownership path
while avoiding new modules or public import churn.

## Behavior locks

Three direct contract tests were established:

- `contracts_export_target_identity_table_is_stable`;
- `contracts_import_option_defaults_and_explicit_modes_are_stable`;
- `contracts_pou_type_alias_and_keyword_table_is_stable`.

The pre-refactor `trust-plcopen` library baseline passed 22/22 on the clean
builder source. The same 22 tests passed after the ownership split, the three
new contract tests passed 3/3, and the combined crate library set passed
25/25. Focused `cargo fmt -p trust-plcopen -- --check` also passed.

The documented authority was expanded only for exact product contracts:

- export target ID, suffix, label, and serialization table;
- default native-vendor-parity import mode and explicit strict-IEC adapter
  mode;
- accepted POU type aliases plus canonical XML, declaration, and terminator
  table.

These are truST PLCopen product choices, not IEC 61131-3 deviations. The
corresponding product guide and `docs/PLCOPEN_DECISIONS.md` were updated.

## Architecture evidence

`docs/diagrams/architecture/plcopen-interchange-contracts.puml` and its
generated SVG show the new ownership boundary. The diagram was rendered on
`trust-builder`, `docs/diagrams/manifest.json` was refreshed, and the remote
diagram-drift check passed. `ARCH-VERIF-P18-14` records the completed
SOLID/KISS/DRY acceptance review.

## Initial identity maintenance

At `af02c0e895b5136c24843d9b893efb075dfab08c`, the clean code/spec report
contained 17,227 facts and 84 finding messages: 21 new declaration identities
and their 21 stale predecessor identities, with no semantic or unclassified
finding. The reviewed identity transaction removed the 21 stale identities
and registered all 21 successor facts against the existing historical PLCopen
gap before any authority reuse.

At `9a921237a02a6dbb2e558dbc00035745970f1164`, the accepted report contained
17,227 live = 17,227 mapped facts, zero findings, 15,185 `spec_gap`, and 2,007
`specified`. The post-identity refresh proposal was empty. The subsequent
authority transaction and accepted census are frozen separately in:

- `verification/p18/review-locks/plcopen-contract-authority-reuse-lock.json`;
- `docs/internal/testing/evidence/plc-verification-program/2026-07-29-p18-code-spec-authority-reuse.md#plcopen-contract-ownership-follow-up`.

## Import-default proofability follow-up

The exact focused trace at
`77fb808f79b7026af60e941ca3021b86cfe8cc8d` executed all three contract
tests successfully. Export-target identity covered three exact specified code
facts, and POU identity covered four. The import-default test exited zero but
produced no attributable code fact because both defaults were compiler-derived
and therefore had no executable production body. The mapping generator
correctly produced no proposal for that test.

The already-green import-default test was used as a behavior lock for a
behavior-preserving refactor. `Default` is now implemented explicitly for
`PlcopenImportGlobalVarMode` and `PlcopenImportOptions`; the values remain
`NativeVendorParity` and a matching options struct. The same focused test
passed after the refactor, and focused crate formatting stayed clean.

At exact refactor commit
`10a3b67b909886dbf8f35bda2b676f0fe05a26bd`, the clean live scanner found
17,229 facts and exactly six finding messages:

- two new private `Default::default` method facts;
- two stale public-type body digests;
- no stale identity, semantic-review, or unclassified finding.

The reviewed refresh selection:

- registered both new methods temporarily against
  `SPEC_GAP_P17_HISTORICAL_CODE_PLCOPEN_DEVTOOLS_001`;
- accepted the two exact public-type body-digest rebinds;
- changed no risk acknowledgement.

The transaction was bound to:

- source report:
  `sha256:2891a22610899ce54abd0cba6a91a4e1657d3b695e5efe0bb5fc18a3036a5eba`;
- source input:
  `sha256:984bd654f389765bccb566fd1910df1a7964770bb901ddce04f5531348e29615`;
- proposal:
  `sha256:477cdba267e62aeba3f8a5fd4e5aeccbbec798238a5c3adf6ba1a9238a7e9ea3`;
- operation set:
  `sha256:7ab167d9d5d8a6b2c4aa476169a6193e6cfe23eb05be55308bdb61496005acdd`;
- selection:
  `sha256:4083611b0a97a936148bffb8606c4b4c3d56a4a7021e746b720b816f44138cdf`;
- application:
  `sha256:c40b5a0302bdf485c033f0b2f6701a89e0dedc72cc13a20bf23ac174ba74e949`.

The code map moved from
`sha256:2fb0a259134f02e415fd80b5fe1a213be6f1179c1fbd102f48a0d7aa0c3caf84`
to
`sha256:dfa95de431ebff6ffcf2fc5663116af7e912c64729f15edd566abaa925682024`.
The gap ledger moved from
`sha256:da13286bce2567fb7d55cb2eaa1536d42b0572bb4a0f4286a5a3a9d28588dc71`
to
`sha256:fcc647a68431b3527d88d549157d6b00cada25763f1baba96a797cc0de9ef1bd`.

The two temporary method gaps do not claim completion. They must be rebound
to `PLCO_ST_COMPLETE_INTERCHANGE_001` through a separate live
authority-reuse transaction before the import-default trace can become a
mapping candidate.

## Boundary

This work establishes exact PLCopen contract ownership and focused
behavior-lock proof. It does not accept the two pending high-risk test
mappings, select numbered authority for `shims_metrics.rs`, close all PLCopen
specification gaps, or close any execution, reachability, adequacy, candidate,
release, or independent-review row. No broad `just fmt`, `just clippy`,
`just test`, or `just test-all` gate ran.
