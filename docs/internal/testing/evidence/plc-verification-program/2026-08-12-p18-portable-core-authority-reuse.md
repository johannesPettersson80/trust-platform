# P18 portable-core authority reuse

Date: 2026-08-12
Active rows: `VERIF-P18-003`, `VERIF-P18-005`

## Scope

This slice reviews the complete live existing-authority proposal after the
release-evidence closeout and accepts only exact portable-core matches. It does
not infer authority from a shared file, path, name, test, or implementation.

The clean pre-application report at detached `trust-builder` validation SHA
`bdbd62d4b2865069d695f0359c6bc68d3bcd5f57` contains 17,489 live facts, zero
findings, 13,846 `spec_gap` facts, and 3,383 `specified` facts. The review-only
proposal contains 31 groups and 201 candidate facts: 105 singular, 96
ambiguous, and 13,645 facts with no candidate.

## Semantic review

Six singular groups, containing nine exact facts, are accepted:

| Source surface | Facts | Accepted invariant | Executable locks |
| --- | ---: | --- | ---: |
| portable initializer catalog | 1 | `VM_CORE_INITIALIZER_CATALOG_001` | 1 |
| portable unary and binary operator kinds | 2 | `VM_CORE_PORTABLE_OPERATIONS_001` | 5 |
| portable default-value error | 1 | `VM_CORE_PORTABLE_DEFAULT_VALUES_001` | 2 |
| portable `SIZEOF` error | 1 | `VM_CORE_PORTABLE_SIZEOF_001` | 1 |
| date-time calculation mode, constant, and error | 3 | `VM_CORE_DATETIME_CALCULATION_001` | 3 |
| stable runtime error-code type | 1 | `RT_CORE_STABLE_ERROR_IDENTITY_001` | 1 |

The other 25 groups and 192 candidate facts are withheld. All 96 ambiguous
facts remain `ambiguous_authority`. Singular groups are withheld when a path
contains mixed semantics or the proposed claim does not govern every grouped
symbol. In particular, variable collection is broader than the integer
initializer contract, local initialization is broader than reference
resolution, member visibility is not call binding, publication-path safety is
not ST interchange fidelity, and generic runtime lifecycle helpers are not
execution-backend selection. No partial group is forced into an invariant.

The reviewed selection is
`sha256:790d517b5ac2b4f85eb6644850f3c822b20fe76d7195e194594856bf9713c843`.
The source proposal is
`sha256:1b567358884a40f43a1b83d8fb403b7907c45a6a3f7ebd4748168a36c035402e`
with operation-set digest
`sha256:2997bef1996f29043e20086579a7c7b48d048bb877deee6afa0a80bf78b62cfb`.

## Atomic application and report

The complete 31-group selection passed check mode, then live-reconstructed and
applied one two-file compare-and-swap transaction. The code-map digest changed
from `sha256:7c433f203060fd43e00394c4fc4a9dd93250d13a205875e941c7bd4574bc7aff`
to `sha256:4a3500640c770c02e5a3b753b71fb6fc689af48d6d1aff45442c43d89c6a78a9`;
the gap-ledger digest changed from
`sha256:a9481a2646d927cc005fb4948c9f8727fb22f001992bc2e6150f2aa1919d4b91`
to `sha256:ae9bb9895270127e3f3af4a953800ac628a588c004f6593552b7fd86b0110fa0`.
The application artifact is
`sha256:1a195f5ef7562fe5de160bab82329652264201c609eb142cc97705d9d9e1b4b9`.

The first exact post-application detached validation SHA was
`6a7e43842165840276f47b5c5e049d3ad5909345`; its clean full report was
`sha256:165a50d5fc1a911a4fbcf3dda582b92a6f703cf4b8e621d13be4a65f017a6a9d`.
After recording the invariant and evidence metadata, the final evidence-bearing
candidate at `8cab239452b7fe83dbb52b4192d1341e0f50fbcf` regenerated the authenticated
report as
`sha256:9e96b93945b4ae9ff84b06273d254831105c0ec3b0d461595f2fb029f68178f4`:
17,489 facts, zero findings, zero blocking findings, 13,837 `spec_gap`, and
3,392 `specified`. Its residual review-only proposal is
`sha256:a5c884445cba13dc0fcf03f6a4c3c366286ef32713eab7c95b861a4bca76ea82`
with operation-set digest
`sha256:990fb2fa532adbf87fcf8439c7362872b0beabfb5b447b6049aba0ef277bdc0b`.
It retains exactly 25 withheld groups and 192 candidate facts: 96 singular and
96 ambiguous; 13,645 facts have no existing-authority candidate.

## Focused execution

On `scena-rust-builder` at final candidate
`8cab239452b7fe83dbb52b4192d1341e0f50fbcf`, with Rust 1.97.0, Cargo 1.97.0,
nextest 0.9.140, and the shared warmed target, every exact cataloged selection
named by the six invariants passed: 13/13 tests. This covered initializer
identity, five operator partitions, two default-construction partitions, the
complete `SIZEOF` matrix, three date-time boundary partitions, and exhaustive
stable error-code text identity. The focused authority-selection,
authority-reuse, and code-spec contract modules also passed 40/40.

This execution confirms current behavior locks only. It does not mint a
producer-authentic P18 lock, prove assertion reachability or mutation
sensitivity, or promote any invariant above S0.

## Bootstrap and boundary

Canonical source was `/home/johannes/projects/trust-platform`, branch
`plc-verification-program`, local base HEAD
`9b9cb84f41f6375ece6e346dff5e7324558a4104`. The isolated destination was
`trust-builder:/home/johannes/projects/trust-platform-p18-release-ec413525`.
Its temporary pre-application tree matched the local tree exactly;
`AGENTS.md` and the complete `.codex/skills/**` tree matched the canonical
hashes before evidence production.

`VERIF-P18-003` and `VERIF-P18-005` remain globally open. The 13,837 no-authority
facts, 25 withheld candidate groups, 4,233 nonmapping-test denominator recorded
by the active board, producer-authentic execution, reachability, mutation,
hardware/UX/platform proof, and later candidate rows remain debt. No
`just fmt`, `just clippy`, `just test-all`, or P18-016 broad gate ran.
