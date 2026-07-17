# Five-invariant case-backed proof batch validation

Date: 2026-07-17

Case and runner foundation checkpoint:
`56b9b2436b8e7a6dc5c7e6d69487616005bf0831`

Promotion checkpoint:
`3a53c0c1b936464ed515300b591b541f2b4c1e01`

Final report source checkpoint:
`808f66c12bd20358ed71c0524743f6e8456e3737`

Final report/evidence checkpoint before this record:
`f68da5f6a0ae98b5fc5a7fb07a8813ee0a8adf70`

## Outcome

This batch added 43 executable, case-backed assertions for five existing
written contracts that lacked targeted proof:

- parser recovery and bounded diagnostics: 15 cases;
- bounded STRING binding and truncation: 20 cases;
- subrange assignment and rejection behavior: 3 cases;
- bound process-image I/O behavior: 2 cases; and
- cold, warm, and automatic restart storage behavior: 3 cases.

All 43 cases passed against unchanged product behavior. The batch found missing
executable assertions, not a product defect, so it did not manufacture a red
result or modify runtime/compiler behavior. The five invariants moved from S0
to evidence-supported G1:

- `IEC_PARSE_RECOVER_001`;
- `IEC_STRING_001`;
- `IEC_SUBRANGE_001`;
- `RT_SAFE_IO_001`; and
- `RT_SAFE_RESTART_001`.

The invariant registry now contains 54 records: 14 at S0, 31 at G1, and 9 at
G2. The implementation-board phase rows remain open because the batch does not
claim area completeness, broad proof, or closure of the remaining specification
and coverage debt.

## Tests and case binding

The new case-backed runners are cataloged as:

- `TEST_IEC_PARSER_RECOVERY_TRACE_001`;
- `TEST_IEC_STRING_BINDING_TRACE_001`;
- `TEST_IEC_SUBRANGE_TRACE_001`;
- `TEST_RUNTIME_IO_BOUND_TRACE_001`; and
- `TEST_RUNTIME_RESTART_STORAGE_TRACE_001`.

Their committed case-file digests are:

| Test | Case-file SHA-256 |
| --- | --- |
| Parser recovery | `249d695260024e2d85615721af523ff57d80be2673791680c3d1b4d41823e040` |
| STRING binding | `63179ef133c5b8deeeee5cfa430bc7249e79be86b1334591829a990608aca97c` |
| Subrange | `dd0eb893cf48b944114f28f9f9b63e9946ee25a7f18c388e648e62a721632acf` |
| Bound I/O | `5651bab7a19964e2f3e037947628d2fd32c8b6a737fea523b53a5ba0aead009b` |
| Restart storage | `1696fc81dafdb5ba446beaf83545a1f9521511ebd645ca3753208957aefa9a34` |

The parser, STRING, and subrange runners execute through public syntax/HIR
interfaces. The I/O runner exercises addressed values through the runtime
process-image boundary. The restart runner executes the scheduler restart
storage path in its owning test-only module. No duplicate production
implementation was introduced.

## Authentic proof records

`prove.py v1` wrote five clean lock baselines and five clean descendant lock
comparisons:

- `EVID_TEST_IEC_PARSER_RECOVERY_TRACE_001_LOCK_BASELINE` /
  `EVID_TEST_IEC_PARSER_RECOVERY_TRACE_001_LOCK_COMPARE`;
- `EVID_TEST_IEC_STRING_BINDING_TRACE_001_LOCK_BASELINE` /
  `EVID_TEST_IEC_STRING_BINDING_TRACE_001_LOCK_COMPARE`;
- `EVID_TEST_IEC_SUBRANGE_TRACE_001_LOCK_BASELINE` /
  `EVID_TEST_IEC_SUBRANGE_TRACE_001_LOCK_COMPARE`;
- `EVID_TEST_RUNTIME_IO_BOUND_TRACE_001_LOCK_BASELINE` /
  `EVID_TEST_RUNTIME_IO_BOUND_TRACE_001_LOCK_COMPARE`; and
- `EVID_TEST_RUNTIME_RESTART_STORAGE_TRACE_001_LOCK_BASELINE` /
  `EVID_TEST_RUNTIME_RESTART_STORAGE_TRACE_001_LOCK_COMPARE`.

Every pair has distinct run IDs, matching case-file and execution-contract
digests, clean full commit revisions, valid ancestry, and all-passing per-case
summaries. No broad-gate evidence was relabeled as targeted proof, and no
invariant was promoted beyond G1.

## Defects and corrections

No product defect was reproduced in this batch. Focused implementation exposed
and corrected test/evidence integration issues before proof:

- one parser fixture expected the wrong stable diagnostic;
- the STRING runner initially called a private HIR helper instead of the public
  `analyze` interface;
- the restart runner initially omitted the `RestartMode` import;
- test-only support triggered a dead-code warning; and
- the prover correctly required the new verification-cases development
  dependencies to be committed in `Cargo.lock` before proof.

The final focused gate then found four stale live-census tripwires after the
five new Rust test facts were added. Their expected counts were refreshed from
the live scanner: Rust facts 3,190 to 3,195, total facts 3,985 to 3,990, mapped
facts 201 to 206, and coverage states from 12 gap-open / 43 covered to 5
gap-open / 50 covered. All four regression modules passed after the refresh.
These were verification-test baseline defects, not product behavior defects.

## Validation cadence

Targeted runners and metadata/proof checks ran throughout implementation. Heavy
gates ran once at the end on the isolated clean `trust-builder` checkout
`$HOME/projects/trust-platform-five-s0-final` at
`f68da5f6a0ae98b5fc5a7fb07a8813ee0a8adf70`, using the warmed shared target
`$HOME/.cache/codex-targets/trust-platform-gate`:

- `just fmt`: passed in 2.109 seconds;
- `just clippy`: passed in 24.561 seconds;
- `just verification-veryquick`: passed in 16 minutes 0.626 seconds, including
  812/812 focused Python tests and metadata validation of 648 records; and
- `just test-all`: passed with zero failures in 10 minutes 59.322 seconds.

The remote disk preflight and cleanup followed `AGENTS.md`. Only generated
cache/target output was removed; no source checkout or non-generated file was
deleted. The final gate completed with 3.8 GiB free under `/tmp`. The root
filesystem had 4.5 GiB free after the warmed target expanded, so no additional
heavy gate was started.

## Generated report refresh

All 15 installed report pairs were regenerated from a pristine checkout at
`808f66c12bd20358ed71c0524743f6e8456e3737` with timestamp
`2026-07-17T06:07:00+02:00`. Every generator and production at-rest validator
exited zero.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `68ec93eebec3293d83ea884399f346c2058643363db40e61ce6a66e27a0fc086` |
| Coverage-matrix gaps | `ba239e522212abdd8c74e2bd1f6a338a729475b65d38cadbcfd2f029519eab32` |
| Malformed-input coverage | `ad02b784c38cf52fb7fddbb4cf80c8909b4ee459aee31426b54dc93f7cd87cb7` |
| Unmapped-test debt | `931894db2143ccdf0cc9a8edff64aae768a36c25cbb8bc3687f54a284088ac13` |
| Test-refactor assessment | `68b2689625521333aad722832cb89735e7728fe80d3ed0aabba537210318a55e` |
| Ignored-test inventory | `fca5b78e0d55a8f594fa39b9484f25175a09cf704cbf5ee705abb04e9f8fc541` |
| Phase 5 suite audit | `c20644237a0a9e95f6501a04562190d8f1befa6dde049cf060634880dfa807e7` |
| Invariant-seed audit | `d546e3719f37df0f2226f47b9a0058fa5ba8867df8ec85d34979b0e6e6c4f660` |
| Specification completeness | `7ae97b5e402b4f580920623fee9616b333b413730155b119b89397b7688182fe` |
| Requirement/oracle audit | `9299cbeaeb676f7951aa8efc92142d9afb89ae63e584c114a24541819253b220` |
| Conformance alignment | `26010d5fd2ce09f544049c09759307661c73865be0fe3bb6eacbb1b172433cf2` |
| Runtime-anomaly audit | `47e311a84710080fd1c5890b5dff803168ff9e3d6e26d8b966054b461beff03e` |
| Fuzz-program audit | `b96f9f718a3dd5834decac0b312f5654e3024fd8d3a3f9f64f3151f32ae73cf7` |
| Mutation program | `830a3b50e3acee105d16d67080c382f502a3f6498bd46381a861f3b070d9a007` |
| Specification-source audit | `b6962b6dc036b30768446877e5a52e8e4a755a5259a09a82ef906c2626253817` |

## Honest remaining posture

- Fourteen of 54 invariants remain at S0; 31 are G1 and 9 are G2.
- The required coverage matrix still has 63 of 80 slots missing.
- The hand-owned catalog maps 206 of 3,990 scanner facts. The remaining 3,784
  facts are catalog debt, not automatically missing product tests.
- Fourteen of 54 invariants still lack an eligible specification/oracle
  binding.
- Conformance alignment remains 0 of 21 explicitly linked.
- The bound-I/O invariant still lacks non-Modbus and hardware-lab coverage.
- Broad-gate promotion debt remains visible; this batch claims targeted G1 only.
- CI, workflows, suites, approved proof producers, and enforcement posture are
  unchanged.
