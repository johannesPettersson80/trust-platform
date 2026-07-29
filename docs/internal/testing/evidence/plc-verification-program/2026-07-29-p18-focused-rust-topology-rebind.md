# Phase 18 focused Rust topology rebind

Date: 2026-07-29

This evidence records the test-first repair required to resume the retained
189-test focused coverage trace. It is verification-harness work only. It does
not change product behavior, close the affected code fact's specification gap,
or promote any Phase 18 proof row.

## Failure and focused red-green proof

The retained trace executed its selected tests, then failed while projecting
observed Rust paths:

```text
focused coverage trace failed: crates/trust-runtime/src/bin/trust-runtime/ads.rs is orphaned from every live Rust target
```

The live target resolver recognized Cargo's
`src/bin/<name>/main.rs` convention but omitted the equally valid
`src/bin/<name>.rs` root when a selected module lived below
`src/bin/<name>/`.

Expected red before implementation:

```text
python3 -m unittest scripts.verification.mapping_live_code_facts_tests.MappingLiveCodeFactsTests.test_covered_file_uses_live_topology_and_single_file_extractor

ValueError: crates/example/src/bin/trust-runtime/ads.rs has no live Cargo target root in crates/example/Cargo.toml
Ran 1 test in 0.011s
FAILED (errors=1)
```

Paired green after the minimal resolver change:

```text
python3 -m unittest scripts.verification.mapping_live_code_facts_tests.MappingLiveCodeFactsTests.test_covered_file_uses_live_topology_and_single_file_extractor

Ran 1 test in 0.004s
OK
```

The complete six-test live-topology module passed locally. The adjacent
coverage-trace, focused-trace-authentication, mapping-candidate, and live-code
fact suites passed 51/51 both locally and on the manually bootstrapped
`trust-builder` checkout at
`fbfe94092dc5a319067a8cf027551b84750f3590`.

The pre-implementation planner exited `6` on the already-open global Phase 18
metadata ledger (1,877 findings); it reported no change-local planner error.
No broad workspace test, format, clippy, or `test-all` gate ran.

## Selection and risk review

The frozen clean builder report at
`fbfe94092dc5a319067a8cf027551b84750f3590` contains 17,227 live facts and
exactly two findings. Both findings belong to
`CODE_CAE72BA5D605AE19C6A27FF1B029DC7F1ED0759CC379AB73EB9CAD84694AF2D2`,
the `_live_rust_target_roots` function:

- reviewed body digest changed;
- risk-acknowledgement body digest changed.

The report SHA-256 is
`a83db440045294912d04e281bcbdf75c17452c42bc8593baf5182e40f2538a84`;
its Markdown SHA-256 is
`65293ab4d1b9017426687979f8a37e4e605488eed8921982b16e0c5771005b3f`.
The source input digest is
`6baebfb16736b99d784bbeba190908210fb19cba6b3da51b4630c2dd1a9ecbaa`.

The semantic and risk owners accept the single quarantined digest rebind
`CODEMAP_CAND_C174C541AFD29945EFBD7658E689CAB5DFA934D9E6EF4EF52DE9131F14D18FE6`.
The implementation adds only the missing `src/bin/<name>.rs` candidate beside
the existing `src/bin/<name>/main.rs` candidate. Existing file checks and
ambiguous-topology rejection remain unchanged. Signature, identity, owner,
area, visibility, `filesystem_or_environment` risk signal, `spec_gap`
disposition, and historical gap binding remain unchanged. This review does not
invent specification or proof authority.

The new reviewed body digest is
`253126980baeb4b40436bd16d1b770b1b589e8c201e7f3a45f3f135030605a73`.
The proposal SHA-256 is
`de228606d062b36e81bf8c975efd58553c64fd73789ad88e273283e18a0b9e02`;
the selection SHA-256 is
`38f9889f26ebe435905767d1153fa28fc3eed346d9dd4a4307c940739fcfcaf9`;
and the operation-set digest is
`252eed70a5771080f4245e4ad62c7374d01d3e82b6af77e642fc013982bb824f`.

The write-free reconstruction and recoverable atomic application both
accepted one rebind, zero transfers, zero removals, and zero direct missing
mappings. The application SHA-256 is
`da82b843f28a4911f348d6c64e7e0c56a823299c6d2db7d41d0041b1efbcb975`.
The code-map digest changed from
`17cea30baed1115d78700723d6cb8a97b2739c24a5cec693caaa780e668f70ee`
to
`4dab48ddbe77f08f9dd121b333343ee2c0bbf31f2f1927a72c15aa6ad0784b4c`;
the risk ledger changed from
`c88a4fe78226a4cab8635a972008c7516304f6b006533717970471b107d0d80d`
to
`9791b8d1faa1f4592520b70a37fa9aab4664c54af7b0e007e8967d7232b1b337`.
The gap ledger remained byte-identical at
`401e26a43a5d2c6572f7799ca6d602d40c088066385d82dbbb7d1579d9909c93`.

Clean post-commit report and resumed trace evidence are recorded separately
after the exact metadata commit is frozen.
