# LSP position encoding and line-boundary product fix

Date: 2026-07-14

## Missing contract and tests

`SPEC_GAP_EDITOR_LSP_POSITION_ENCODING_001` asked which encoding governs every
incoming and outgoing LSP position. `docs/specs/14-lsp.md` now records UTF-16
as the supported session encoding, supplementary scalars as two code units,
and LF, CRLF, and bare CR as the three line-ending sequences.

Existing tests covered initialization, incremental edits, hover ranges, and
semantic-token starts and lengths. Two new tests add the missing line-boundary
partitions: direct UTF-16 round trips over every LSP line ending and a real
hover request on a later bare-CR line.

## Red results

At clean commit `8861ba1a2ffc862cebdf0552d73ed5ece2147977` on
`trust-builder`:

```text
cargo test -p trust-lsp \
  utf16_positions_round_trip_across_all_lsp_line_endings -- --nocapture
# exit 101
# CRLF byte 6: left Position { line: 0, character: 4 }, expected character 3
```

At clean commit `2d0572cd39ef24257bd23f130a9209fc2c8b9043`:

```text
cargo test -p trust-lsp \
  lsp_hover_uses_utf16_positions_after_bare_cr_line_endings -- --nocapture
# exit 101: hover after bare CR line endings
```

The shared line index recognized only LF. CRLF's CR byte was counted as a
character when converting the LF offset, and a bare CR did not advance the
line at all. The latter made valid later-line requests unresolvable.

## Green result

At clean commit `993530fac7ab71f6e033770df7338df89deba396` on
`trust-builder`:

```text
cargo fmt --all -- --check
cargo test -p trust-lsp utf16 -- --nocapture
# 6 passed
```

The line index now recognizes CRLF as one sequence and bare CR as a line break.
Offsets inside any terminator clamp to the preceding line end. The same helper
serves incoming positions and outgoing ranges, edits, and token coordinates.

## Honest posture

The written contract and six exact tests close the specification gap.
`EDIT_LSP_POS_001` remains `S0/gap_open`: the tests do not emit same-run case
artifacts, so producer-authentic proof and a causal broad gate remain explicit
debt.
