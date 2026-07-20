# IEC STRING Binding Bounds Closeout

- Date: 2026-07-15
- Product source revision: `193be5876917c7ec032c2d1b67dc49186e8b7365`
- Evidence posture: specification closeout and focused test association only;
  this is not producer-authentic red/green proof.

## Closed Boundary

`docs/specs/02-data-types.md` and `docs/IEC_DECISIONS.md` now define the
receiving-capacity rules for `STRING[n]` and `WSTRING[n]` across assignment,
initialization, function and function-block copy-in/copy-back, function results,
and `VAR_IN_OUT`. Capacity is counted in Unicode scalar values; ordinary writes
truncate only the excess suffix, while `VAR_IN_OUT` requires the same resolved
family and capacity. `STRING` and `WSTRING` remain separate implicit-assignment
families.

`SPEC_GAP_IEC_STRING_BINDING_BOUNDS_001` is therefore closed against
`SPEC_IEC_DATA_TYPES_CANDIDATE_001`. The 19 committed catalog records remain
the reviewed test denominator for this closeout.

## Focused Validation

Run on `trust-builder` against the source-equivalent validation checkout using
`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate`:

- `cargo test -q -p trust-runtime --test string_binding_bounds`: 15 passed.
- `cargo test -q -p trust-hir --test semantic_type_checking control_flow_and_calls::string_in_out_`: 2 passed.
- `cargo test -q -p trust-hir --lib cross_file_constant_string_capacity`: 2 passed.

The broader batch gates are recorded separately after the metadata checkpoint;
they do not upgrade this evidence row beyond `proof_kind = "none"`.
