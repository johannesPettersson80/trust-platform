# Bytecode Duplicate Standardized-Section Rejection

Date: 2026-07-14

## Scope

This product vertical covers the STBC rule that each standardized section ID
from `0x0001` through `0x000C` may occur at most once. It does not define the
complete bytecode validator contract or a stable typed error-code surface.

## Red

Tests-and-spec commit: `cf8987658931e5097ae890cf26f90bb9557afc3d`

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform-bytecode-count-502fff48" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" cargo test -p trust-runtime --test bytecode_container duplicate_standard_section_ids_are_rejected -- --exact --nocapture'
```

Result: 0 passed and 1 failed. `BytecodeModule::decode` accepted a container
with two `STRING_TABLE` sections. The runtime's section lookup returns the
first matching section, so accepted meaning depended on table order.

## Green

Product checkpoint: `5fb897f7e36538c22cb1de97e231f613af9c6870`

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform-bytecode-count-502fff48" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" cargo test -p trust-runtime --test bytecode_container --test bytecode_decode_resource_bounds --test bytecode_encoder --test bytecode_metadata --test bytecode_roundtrip --test bytecode_sections --test bytecode_validation'
```

Result: 62 passed and 0 failed. The table-driven regression duplicates each of
the twelve standardized IDs. Decode now rejects the duplicate table before
payload selection while leaving unknown extension IDs outside this singleton
rule.

## Honesty Boundary

- `SPEC_GAP_BYTECODE_VALIDATOR_001` remains open.
- `SPEC_GAP_VM_ERROR_MODEL_001` remains open.
- `VM_SEAM_VALID_001` remains `spec_gap` at `S0`.
- The evidence has `proof_kind = "none"`; it records a tests-first product
  defect and fix, not complete validator coverage or release proof.
