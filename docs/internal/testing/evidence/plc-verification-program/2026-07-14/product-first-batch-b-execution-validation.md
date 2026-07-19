# Product-first Batch B execution validation

Date: 2026-07-14

Validated source bytes: `dbf9ee04ec1745c1d6c08da540de1ee6f188034b`

## Result

Batch B converted three written gaps into four focused product tests and found
two runtime defects before the tests passed:

1. statement-boundary debugger dwell consumed the armed cycle watchdog and
   output-commit deadlines;
2. `control.debug_enabled` required only Engineer, while an operation omitted
   from the policy inherited Viewer authority.

The runtime now excludes only measured statement-pause dwell from the current
cycle deadlines, requires Admin to activate debugging, and requires Admin for
every future unclassified control operation. The tests and fixes are recorded
separately in `debug-pause-watchdog-fix.md` and
`control-authorization-fix.md`.

## Test census

The mechanical scanner measured four new Rust facts:

- Rust facts: 3,097 to 3,101;
- all scanner facts: 3,892 to 3,896;
- mapped scanner facts: 89 to 93;
- unmapped scanner facts: unchanged at 3,803.

The live catalog join passed with 98 committed records against 3,896 scanner
facts. Only the two owning census tripwires were updated; no scanner,
validator, schema, suite, or gate behavior changed.

## Broad validation

The `trust-builder` preflight found 19 GiB available under `/home/johannes`,
below the broad-gate target. Only the generated 64 GiB shared Cargo target was
removed. The retry preflight reported 83 GiB available under
`/home/johannes` and 6.8 GiB under `/tmp`.

At a clean detached checkout of the validated source, with Rust 1.97.0 and
Cargo 1.97.0, this command exited zero in 791.12 seconds:

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform" && \
  export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" \
  TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp" && \
  just fmt && just clippy && just test-all'
```

`just clippy` emitted only the pre-existing `trust-lsp` `question_mark`
advisory. The checkout remained byte-clean. The four explicitly required
runtime verticals then passed against the same source and builder target:

```text
cargo test -p trust-runtime --test api_smoke
# 3 passed

cargo test -p trust-runtime --test debug_control
# 20 passed

cargo test -p trust-runtime --test complete_program
# 1 passed

cargo test -p trust-runtime --test runtime_reliability
# 4 passed
```

This is broad validation, not approved broad proof. The four ordinary Rust
tests do not emit bound same-run case artifacts, so no broad proof row was
created and all three invariants remain at S0.

## Report refresh

All 15 existing report generators were run independently from pristine
worktrees at `92f68757a2b72c9b209e711d8b8729d15bb40075` with timestamp
`2026-07-14T18:13:00+02:00`. That commit differs from the broadly validated
product checkpoint only by refreshed Python report-baseline assertions. Each
generated pair passed its owning at-rest validator before import. The final
digests are recorded in the evidence index and in the report-refresh commit.

## Remaining work

- `DEBUG_PAUSE_001`, `DEBUG_AUTH_001`, and `SEC_AUTHZ_001` still require the
  frozen baseline-seed execution-ready review, producer-authentic proof, and a
  causal broad gate before any proof-level promotion.
- `SPEC_GAP_DEBUG_PAUSE_WATCHDOG_001`,
  `SPEC_GAP_DEBUG_AUTHORIZATION_001`, and
  `SPEC_GAP_CONTROL_AUTHORIZATION_MATRIX_001` remain open at `test_mapped`.
- The next product vertical must be selected from a written high-risk gap with
  missing tests; it must start with a focused red test, not another control
  plane expansion.

## Boundaries

- No validator, schema, suite, approved proof producer, workflow, board row,
  skill, or agent instruction changed.
- No spec gap was closed and no invariant was promoted beyond S0.
- Runtime behavior changed only where the two focused red tests required it.
- Version 0.24.41 and the changelog were updated with the pause-watchdog and
  authorization fixes; tag and release remain deferred until merge to main.

## SOLID/KISS/DRY review

- Pause accounting remains owned by `DebugControl`; execution backends consume
  one runtime deadline helper instead of duplicating pause logic.
- Authorization stays in the existing policy module and dispatcher boundary;
  no parallel policy table or new abstraction was introduced.
- The four regression tests are split by owning product boundary and reuse the
  existing runtime/control test harnesses.
