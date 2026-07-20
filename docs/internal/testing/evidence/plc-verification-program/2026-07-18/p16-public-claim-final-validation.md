# Phase 16 Public-Claim Closeout Final Validation

- implementation commit: `3975e89346ed2526831102fd627e41996a50a974`
- validation date: `2026-07-18`
- local platform: Linux aarch64 (Raspberry Pi)
- broad-gate platform: `trust-builder` Linux x86_64
- Rust: `1.97.0 (2d8144b78 2026-07-07)`
- Cargo: `1.97.0 (c980f4866 2026-06-30)`

## Result

The P16-002 closeout validates with all 35 registered specification gaps closed.
The metadata graph contains five G1 public-claim proof anchors. `PLAT_PATH_001`
and `PLAT_VSCODE_001` remain implemented rather than validated because native
path and native VSIX execution are still explicit test debt.

No product/runtime behavior changed in this slice. The only implementation bug
found was in existing verification tooling: the specification-source audit's
reviewed release-topic table still required four now-closed gaps to remain open.
The tests-first fix is commit `fac00d48c2030553e6d39fc8af231fe24ae8790a`.

## Focused Validation

On a clean builder checkout at the implementation commit:

- `python3 scripts/run_verification_focused_tests.py`: 853 tests passed in
  968.912 seconds.
- `scripts/verification_metadata_gate.sh`: passed with 743 metadata records.
- `python3 scripts/check_verification_tooling_selftests.py`: 33 of 33 fixtures
  passed.
- All 15 affected report generators and at-rest validators passed from clean
  isolated worktrees; their committed digests are recorded in
  `p16-public-claim-report-rebind.md`.

## Broad Validation

The builder disk preflight initially showed 62 GiB free under `/home/johannes`
and 4.6 GiB under `/tmp`. Only inactive generated caches were removed during
the run; no source checkout or non-generated file was deleted.

- `just fmt`: passed in 2.250 seconds.
- `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate CARGO_INCREMENTAL=0 just clippy`:
  passed in 2 minutes 23.952 seconds.
- The first `just test-all` attempt stopped at
  `openot_capstone_fenced_cross_process` because the isolated checkout did not
  have the sibling `open-ot-ref` worktree expected by that test. The spawned
  producer exited before writing its readiness marker; a direct producer run
  exposed the missing path rather than a product failure.
- A clean sibling worktree was created at the dependency-pinned OpenOT revision
  `137f0e765f085c262651f479be35298b836ac891`. The targeted capstone test then
  passed (1 passed, 3 filtered out), and the same test passed inside the broad
  retry (1 passed, 3 ignored).
- `CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate CARGO_INCREMENTAL=0 just test-all`:
  passed in 10 minutes 32.825 seconds.

The local and builder validation checkouts were clean after their respective
commands. CI enforcement, suite definitions, approved proof producers, product
behavior, and public claim scope were not expanded by this validation record.
