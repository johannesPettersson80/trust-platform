# Bytecode Decoder Collection-Count Bounds

Date: 2026-07-14

## Scope

This product vertical covers the structural allocation boundary for top-level
and nested `u32` collection counts in the STBC decoder. It does not define a
general VM resource budget or close
`SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001`.

## Red

Tests-only commit: `7fe5015f8ae559a1f7862b7e88e3ea4cdfcac415`

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform-bytecode-count-502fff48" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" cargo test -p trust-runtime --test bytecode_container'
```

Result: 4 passed and 7 failed. Every new count-bound test observed
`UnexpectedEof` instead of the required early `InvalidSection` rejection. The
source audit confirmed that the decoder called `Vec::with_capacity` from each
untrusted count before proving that the minimum encoded entries fit.

## Green

Product checkpoint: `36837353b993e906c2b27d1cb0ad969bb7d1da86`

```text
ssh trust-builder 'cd "$HOME/projects/trust-platform-bytecode-count-502fff48" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" cargo test -p trust-runtime --test bytecode_decode_resource_bounds'
ssh trust-builder 'cd "$HOME/projects/trust-platform-bytecode-count-502fff48" && CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-gate" cargo test -p trust-runtime --test bytecode_container --test bytecode_roundtrip --test bytecode_sections --test bytecode_metadata --test bytecode_validation --test bytecode_encoder'
```

Results:

- decoder resource bounds: 7 passed, 0 failed;
- existing container plus adjacent encoder, decoder, metadata, and validator
  suites: 54 passed, 0 failed.

The shared decoder check uses checked multiplication and the minimum bytes
required by every entry. Counts that cannot fit the unread containing payload
fail before count-sized capacity is reserved. No arbitrary maximum was added.

## Honesty Boundary

- The broad determinism/resource-limit gap remains open.
- `VM_SEAM_DETERMINISM_LIMITS_001` remains `spec_gap` at `S0`.
- Maximum container, instruction, stack, local, reference, call-depth, and
  execution-time limits remain unspecified.
- This evidence has `proof_kind = "none"`; it records tests-first product work,
  not invariant promotion or release proof.
