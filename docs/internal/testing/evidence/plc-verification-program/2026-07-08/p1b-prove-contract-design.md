# P1B prove.py Contract Design

Date: 2026-07-09

Branch: `plc-verification-program`

Scope: `VERIF-P1B-008A` only.

## What Changed

- Added the `prove.py` contract to
  `docs/internal/testing/checklists/plc-verification-program/metadata-evidence-traceability.md`.
- Defined catalog binding by `test_id`, case-file digest, and generated
  case-artifact digest.
- Defined the workspace-root default artifact path:
  `target/gate-artifacts/cases/<TEST_ID>.json`.
- Defined red, green, and lock proof rules before implementation.
- Defined failure-kind classification so compile, harness, metadata, timeout,
  and infrastructure failures cannot be mistaken for behavioral red proof.
- Defined green pairing requirements against earlier red/protective-red
  evidence.
- Added the adversarial self-test fixture list that `prove.py` must satisfy.
- Updated policy wording to make proof binding and green pairing explicit.

## Stop Boundary

This slice does not add `scripts/prove.py`, does not execute proof-producing
commands, does not create red/green/lock evidence, does not change runtime or VM
product behavior, does not close spec gaps, does not add CI enforcement, and
does not update Codex skills.

## Validation

Reproduce from the repository root:

```sh
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
cargo fmt --all -- --check
git diff --check
```

Expected local result after this slice: metadata validation reports 83 records.
