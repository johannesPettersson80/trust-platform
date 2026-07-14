# Force lifecycle batch validation

Date: 2026-07-14

Final source bytes: `cb1ece912a4274a7e436f2b92a046cb8882dd9ca`

## Result

Six previously missing force-lifecycle boundaries now have focused product
tests: pause/resume, non-terminating disconnect, release, deliberate stop,
fault recovery, and authorization-token change. All six focused tests passed.
No product defect was observed, so this batch contains no product behavior
change and no manufactured red proof.

## Focused validation

On a clean `trust-builder` worktree at
`4c97a8461032df65d3e10f04d1e9c14ff7422b5e`:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test force_lifecycle_boundaries
```

Result: 5 passed, 0 failed.

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --lib \
  control::tests::auth_token_change_preserves_force_until_authorized_release \
  -- --exact
```

Result: 1 passed, 0 failed.

The unchanged producer-bound runner also passed 1/1:

```text
CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate \
  cargo test -p trust-runtime --test force_lifecycle \
  force_lifecycle_trace_cases -- --exact
```

## Broad validation

After clearing only the generated shared target, the builder preflight reported
87 GiB available under `/home/johannes` and 6.7 GiB under `/tmp`. This command
then exited zero:

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform" && \
  export CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" \
  TMPDIR="$HOME/.cache/codex-targets/trust-platform-gate-tmp" && \
  just fmt && just clippy && just test-all'
```

- `just fmt`: completed and exposed one formatting-only delta in the new
  integration-test file.
- `just clippy --all-targets --all-features`: passed after that formatting step,
  with the pre-existing `trust-lsp` `clippy::question_mark` advisory.
- `just test-all`: passed after that formatting step, including all six new
  force-lifecycle tests.
- The formatting-only delta was committed as `cb1ece91`; a subsequent clean
  builder checkout at that commit ran `just fmt` with no change. Therefore the
  source bytes compiled and tested by `clippy` and `test-all` are byte-identical
  to the final committed source.

This run is broad validation, not approved broad proof. The reviewed broad
producer correctly requires every linked test to emit a bound case artifact;
the six new ordinary Rust tests do not. No `broad-remote-gate.py v1` evidence
row was created, and `RT_SAFE_FORCE_001` remains S0.

## Remaining blocker

The written source oracle cannot replace the current gap oracle without
changing the proof-contract digest and invalidating the historical authentic
red/green pair. The frozen verification control plane has no reviewed migration
for that transition. `SPEC_GAP_RUNTIME_FORCE_LIFECYCLE_001` therefore remains
`test_mapped`, with `source_oracle_proof_contract_transition` and a case-backed
broad gate recorded as explicit debt.

## Boundaries

- No runtime product behavior changed.
- No suite, approved proof producer, CI workflow, validator, schema, skill, or
  agent instruction changed.
- The existing proof-bound case file and its red/green evidence are unchanged.
- The new tests use existing runtime/debug/control APIs and add no production
  abstraction or ownership change.

## SOLID/KISS/DRY review

- The integration file owns runtime lifecycle boundaries; the control unit
  test owns token-rotation request semantics.
- Shared setup stays in one small integration-test harness function.
- No production module grew and no duplicate product implementation was added.
