# Phase 18 residual-authority focused trace

Date: 2026-07-29

## Outcome

The retained focused Rust trace for the 22 withheld code-spec
authority-reuse groups completed after the sibling binary-root resolver
repair. The trace is diagnostic execution evidence. It does not override the
accepted semantic review, turn coverage into specification authority, or
close a Phase 18 reachability row for code facts that still have no accepted
specification.

The exact clean builder source was
`218d47ff4e581d2e49484e4b394ef6e3646da033`. Its code-spec report contains
17,227 live facts, zero findings, 15,185 `spec_gap`, and 2,007 `specified`
facts. The report SHA-256 is
`10e767ec9d23a8b092a79a8c5f9cfd9e3cef72d848a9596fdc1a845c529c4a5c`;
its input digest is
`1a4ef2eede9038fae3ed6010fe3c24fbe6ead7ccb630ef59e4ff17eb303ad3af`.

The exact residual proposal SHA-256 is
`36938566e7280ad2204930af7f8e419b4e6a78d4ecb3a8a0a0cee750b65970a1`.
It contains 22 groups and 283 facts: 71 facts with ambiguous authority and
212 facts with a singular path candidate that the accepted review withheld
for mixed semantics or a direct claim mismatch.

## Builder and retained execution

Canonical checkout:
`/home/johannes/projects/trust-platform`

Clean validation checkout:
`trust-builder:/home/johannes/projects/trust-platform-p18-marker-0d31ff12`

- host: `scena-rust-builder`;
- branch: detached;
- HEAD: `218d47ff4e581d2e49484e4b394ef6e3646da033`;
- root `AGENTS.md`: exact match to the remote primary checkout;
- `.codex/skills/**`: 25/25 files, exact recursive match.

The proposal resolves to 189 unique cataloged Rust discovery identities.
The retained trace produced 183 observed LLVM source-coverage records and six
explicit `no_nextest_identity` records. It contains 1,798 unique covered code
identities and 19,350 test-to-code coverage edges.

The trace SHA-256 is
`077491cdab44379fedf89ae89cdb9d71f3b8085a08a2cca0bc5e7fd4026d15fd`;
its input digest is
`5ffaec4dfabd3ccecce3da9a076da1776ddc6161668c63397fdc26efd1bd4c9b`.
The raw-evidence digest is
`c00097a437c5e5f9bb74a83b0317fd066a452c2fa4d4aca984774736adee0522`;
the observed-source projection digest is
`ce04539c674139e9720b4c3fdb94578d1967a0a460695cd24934ed7324c76ec4`.

All 403 retained attempt/profile files, including 220 `.profraw` files, have
a latest modification time of `2026-07-29T19:50:31.730392+00:00`. The final
trace was written at `2026-07-29T20:30:34.747485+00:00`. Therefore the
corrected resume reused the existing executions; it did not repeat the
183-test campaign.

The six explicit unsupported identities are:

- `DISC_016A7FD66DC071E76D88`,
  `ads_validate_live_cli_requires_ads_wire_feature_without_reading_files`;
- `DISC_2A71C823139F87D5184A`,
  `string_value_size_counts_character_elements`;
- `DISC_37EB78BF38C80DB04EFB`,
  `ads_import_symbols_cli_requires_ads_wire_feature_without_faking_results`;
- `DISC_66249B74D0B590FB23C7`,
  `ads_discover_cli_requires_ads_wire_feature_without_faking_results`;
- `DISC_77757C7D508029CEF91D`,
  `ads_doctor_cli_requires_ads_wire_feature_without_faking_results`;
- `DISC_E454ABA5331BBBD3FE22`,
  `ads_browse_cli_requires_ads_wire_feature_without_faking_results`.

No unsupported identity is treated as execution proof.

## Residual-group classification

Of the 283 withheld candidate facts, candidate-invariant tests dynamically
reached 97 and did not reach 186:

| Accepted withholding reason | Facts | Reached | Not reached |
| --- | ---: | ---: | ---: |
| `ambiguous_authority` | 71 | 32 | 39 |
| `mixed_semantics` | 199 | 65 | 134 |
| `claim_mismatch` | 13 | 0 | 13 |
| **Total** | **283** | **97** | **186** |

Only two complete path groups were reached:

- both `crates/trust-plcopen/src/plcopen/import.rs` facts, but three accepted
  invariants remain equally viable path candidates, so authority is still
  ambiguous;
- all 15 `crates/trust-plcopen/src/plcopen/shims_metrics.rs` facts, but the
  existing vendor-library-shim claim still does not specify every metric and
  helper semantic in the file.

Coverage therefore confirms the accepted fail-closed decisions rather than
reversing them. The trace also gives the next specification-and-test batches
an exact prioritization surface: 186 candidate facts currently lack dynamic
reachability from the proposed authority tests, while the 97 reached facts
still require fact-specific semantic authority before that execution can
count as proof.

## Boundary

This batch fixes and completes one focused diagnostic campaign. It changes no
code-spec disposition, invariant, catalog mapping, gap record, or proof level.
`VERIF-P18-003`, `VERIF-P18-006`, the bidirectional chain, formal
reachability, mutation, adequacy, environment, final candidate, release, and
independent-review rows remain open. No `just fmt`, `just clippy`,
`just test`, `just test-all`, or other broad workspace gate ran.
