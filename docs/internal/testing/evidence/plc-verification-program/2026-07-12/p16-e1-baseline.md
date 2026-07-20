# Phase 16 Execution Slice 001 Baseline

Date: 2026-07-12
Branch: `plc-verification-program`
Clean source revision: `18e7da19ef9a63f5f7582910791a50fa0923662f`
Proof posture: non-proof baseline observation

## Reproduction

The clean baseline contains the shipped TOF defect at
`crates/trust-runtime/src/stdlib/fbs/timers.rs`: after the off-delay reaches
`PT`, `timing` becomes false; the next executed call with `IN = FALSE` enters
the idle branch and resets stored `ET` to zero. Existing shipped-path tests in
`iec_timers.rs` and `fb_timers_full.rs` encode that zero on the following scan.

IEC 61131-3 Ed.3 section 6.6.3.5.5, Table 46, Figure 15(c) instead depicts ET
reaching PT and holding there through the remaining low-input interval until
the next rising input. This is a source-level reproduction only, not red proof;
`prove.py` red remains required before the product edit.

## Builder Preflight

The initial `trust-builder` audit found 18 GiB free under `/home/johannes` and
4.2 GiB under `/tmp`. One unrelated 47 GiB generated target cache at
`$HOME/.cache/codex-targets/trust-platform-ads-windows-fix-20260712-c` was
inactive and removed. Source worktrees were not deleted. The repeated preflight
reported 64 GiB free under `/home/johannes` and 4.2 GiB under `/tmp`, satisfying
the configured broad-gate threshold.

## Boundaries

- No product file changed in the baseline.
- Readiness closure commits `b85e53c87`, `624144386`, and `ffef52d71` remain
  accepted.
- The lifecycle foundation commit `18e7da19e` is frozen and unmodified.
- No suite, approved proof producer, validator, schema, board row, workflow,
  skill, or agent instruction changed.
