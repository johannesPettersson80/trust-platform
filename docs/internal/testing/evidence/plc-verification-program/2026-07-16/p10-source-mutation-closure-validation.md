# Phase 10 source-mutation closure validation

Date: 2026-07-16

Mutation execution source checkpoint:
`56f68f2bbdb12c655f681668c5a5fddda4f4d659`

Final report source checkpoint:
`81f475234f199a5299f7442c8314e0b6f1d30696`

Final validation checkpoint:
`796ef95c8b8908ff3e56d9dad58a215a8144c7c6`

## Outcome

This batch executed the four reviewed source-only Phase 10 mutation shards on
`trust-builder`. Together with the existing bytecode-validator pilot, the
generic Phase 10 report now records five measured shards, six caught mutants,
zero survivors, zero unviable mutants, zero timeouts, and zero infrastructure
errors. The delivered connector-projection shard remains planned because no
delivered-binary execution was performed.

No survivor required a new product test, and no clean baseline exposed a
product defect. The batch therefore made no runtime, compiler, parser, HIR,
connector, editor, or other product-behavior change. Association IDs remain
traceability labels; they do not claim which individual test killed a mutant.

## Tests-first tooling

The source mutation executor was introduced in small, single-responsibility
modules for execution, artifact validation, and runner orchestration. Focused
tests first failed for the missing modules and then pinned:

- exact reserved artifact paths and clean full-SHA source identity;
- closed schema and semantic-digest drift checks;
- exact shard, selector, source, command, runner, and tool bindings;
- raw build/test exit, timeout, output, and duration consistency;
- infrastructure-first result classification and impossible-phase rejection;
- source restoration and package cleanup; and
- generic Phase 10 report consumption of bound execution artifacts.

The first clean runtime-conversion selector was genuinely unviable because its
replacement required `Value: Default`. That diagnostic is retained as
`p10-runtime-value-conversion-mutation-attempt-01-unviable.json`. A tests-first
manifest change replaced it with a compiling generated identity-comparison
mutation before any accepted four-shard evidence was recorded.

## Measured source shards

| Shard | Result | Artifact SHA-256 |
| --- | --- | --- |
| Runtime value conversion | 1 caught | `dcfe38dad50f4e2008bec764335ff920c63065731b2015d9f8d4e834ec637940` |
| HIR subrange diagnostics | 1 caught | `d9a2c0af24cf9ec69b5a0c7f3109a0c02ed2152d27fc7e8fd5c69e2e71ee22d8` |
| Parser recovery | 1 caught | `3330e2600c1ea938bf57ebf67b4126ae1372d4ad8447032cb46832dab3a55c8e` |
| Retain/restart | 1 caught | `2d7d09ba12e20aa791c467fdce969c45dd7c25e6e75f8d2333df10680d30ca9c` |

Each accepted artifact was produced from the same clean execution commit with
cargo-mutants 27.0.0 in an isolated archive and a dedicated generated target.
The generic report JSON SHA-256 is
`e893d67e1514c6bb5e117e2e57e5a391e01fb9cec4f21e5b829f90578fa3770c`.

The first operational attempt was discarded before evidence because two
runner processes overlapped on one generated target. Both were stopped, the
target was removed, and the isolated clone was verified clean before accepted
execution resumed.

## Final remote validation

The final battery ran on clean `trust-builder` checkout
`~/projects/trust-platform-mutation-execution` at
`796ef95c8b8908ff3e56d9dad58a215a8144c7c6`, with repo `AGENTS.md` and the
required `trust-remote-builder` and `st-lsp-solid` skills copied byte-for-byte
into the ignored bootstrap paths. The toolchain was
`rustc 1.97.0 (2d8144b78 2026-07-07)` and
`cargo 1.97.0 (c980f4866 2026-06-30)`.

- `just fmt`: passed in 2.25 seconds with no diff.
- `just clippy`: passed in 87.65 seconds across all targets and features.
- `just verification-veryquick`: passed in 620.92 seconds, including 801/801
  focused Python tests, metadata validation of 551 records, the bounded Rust
  suites, 13/13 bytecode-validator tests, and the selected conformance case.
- `just test-all </dev/null`: passed in 566.66 seconds with 3,200 passed,
  27 ignored, and zero failed across 234 reported result groups.
- `python3 scripts/check_verification_tooling_selftests.py`: 33/33 production
  catcher fixtures passed in 58.13 seconds.
- `python3 scripts/validate_mutation_program_report.py` against the installed
  2026-07-16 JSON and Markdown: passed.
- `python3 scripts/check_diagram_drift.py`: passed.
- `git diff --check` and the full untracked-aware clean-tree check: passed.

The first `test-all` invocation used a mixed terminal wrapper: stdin remained
a PTY while stdout and stderr were redirected. The trust-dev commit test then
selected its interactive confirmation path and failed with `not a terminal`.
The exact test passed under CI-equivalent non-TTY stdin, and the full gate then
passed under the same non-TTY condition. This was a validation-wrapper error,
not a product or test failure; no code was changed to hide it.

The builder began with 63 GiB free on `/home/johannes` and 4.3 GiB on `/tmp`.
After validation, only generated mutation, gate-target, temporary, and sccache
outputs created or used by this batch were removed. The builder was restored
to 76 GiB free; no source checkout or unrelated target was deleted.

## Generated report rebind

All 15 affected report families were regenerated in pristine worktrees from
`81f475234f199a5299f7442c8314e0b6f1d30696` with timestamp
`2026-07-16T10:18:30Z`. Every generator and production at-rest validator
exited zero, the remote staging hashes matched the local files, and the
worktrees remained clean.

| Report | JSON SHA-256 |
| --- | --- |
| Test-class completeness | `022f5547f92a22dd5215cd12e111d1f78e22c4da153a3657021ba2de2ea9ad33` |
| Coverage-matrix gaps | `943bf0cedfe3486e0b540348a58f57c7fc815031221cd6a0b130491f604663f8` |
| Malformed-input coverage | `d8e23593a16bd4e8143ec3ee851443659e0335bb224f8af2f3e60cb3d5dd22e3` |
| Unmapped-test debt | `fac7b772efde31b3818769ed53639a864b2c5a37c180429b26f51a92cd649ad1` |
| Test-refactor assessment | `e8fbf2a7417ff0d378caafb7cb3b8ca7deaa8b048751ce337a1e89eb0dea926b` |
| Ignored-test inventory | `4865d4a42d706fe6b878215f397b80e993930f924aa0af29991684a6fe84ca64` |
| Phase 5 suite audit | `4cb44a47078e66cee845cdf80dda3c3824afcd0ff4e1207e04572f1abfd4b221` |
| Invariant-seed audit | `eb67dae5c24cad772ad28c2b7a3b5a6062d58883dd813e006f2bade8bd866067` |
| Specification completeness | `49d16c1b47640bd0c2a24b2c97cf51b4238a38bbdf47dbc3b3f03df1bf182219` |
| Requirement/oracle audit | `3daad31e9cf65a7a02b59a18629b2a83b0e178adc007ee1fb13b7f6e845616ff` |
| Conformance alignment | `4becf0921e227cbc9bd10fbecc2f878bf3718732553a6d4799bce2658a77de78` |
| Runtime-anomaly audit | `40909ea532b306e30695104a96ef7f80761c3b0ee44876b4d1e0f32033c21b12` |
| Fuzz-program audit | `58f0213d7b9e4b0496b4203b8363544532f12ecf469a4c4c0994986aad0a9cc7` |
| Mutation program | `e893d67e1514c6bb5e117e2e57e5a391e01fb9cec4f21e5b829f90578fa3770c` |
| Specification-source audit | `3bb6dee66a77139d09521375e0f507f19db1c9f6ba56c735fa53fc15585624ad` |

## Honest remaining posture

- The connector-projection mutation shard remains planned and unmeasured.
- No mutation survivor exists in the five measured shards.
- The report records no coverage campaign, proof, invariant promotion,
  spec-gap closure, or release-safety claim.
- Current measured program debt remains visible: 63/80 required coverage
  slots are missing, 20/53 invariants are not specified, 24/53 invariants lack
  an eligible oracle, 3,784/3,966 scanner facts are unmapped, and conformance
  alignment remains 0/21 explicitly linked.
- CI, workflows, suites, approved proof producers, skills, agent instructions,
  and enforcement posture are unchanged.
