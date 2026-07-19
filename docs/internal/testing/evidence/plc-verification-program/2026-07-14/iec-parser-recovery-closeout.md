# IEC parser recovery specification and product fix closeout

Date: 2026-07-14

Red source: `c0b61c013fbb350e06b5164083344dba6a662d50`

Product fix: `5b62e8a103343eeaabeea2b6b205b55fc9b0ce1a`

Focused green source: `c8d4e73cc4b412c35a0c5247f2491e0d41d81de7`

## Missing specification and tests

`SPEC_GAP_IEC_PARSER_RECOVERY_001` had no mapped tests and did not define how
malformed control-flow delimiters must fail or where editor-oriented recovery
must stop. IEC 61131-3 Ed.3 section 7.3.3.3, Table 72, and sections
7.3.3.4.2 through 7.3.3.4.4 define the valid selection and iteration syntax,
but do not define an error-recovery tree for malformed source.

The recovery policy is therefore recorded as an IEC implementation decision,
not an IEC deviation. Missing required tokens must diagnose and make the parse
unsuccessful; retained partial syntax exists only for tooling. Recovery must
make progress and preserve an outer synchronization boundary when an inner
terminator is missing.

Seven scanner-backed tests now cover required control tokens, a missing nested
terminator, an unterminated POU, an unknown token, unary and parenthesized
nesting limits, and an unclosed call delimiter.

## Red product result

The new required-delimiter test was run against the clean red source on
`trust-builder`:

```text
cargo test -p trust-syntax --test parser_error_recovery \
  malformed_control_flow_delimiters_are_diagnosed -- --exact
# FAILED; exit 101
```

All nine reviewed malformed forms were accepted as valid partial constructs:

- `CASE` without `OF`;
- a `CASE` branch without `:`;
- `ELSIF` without `THEN`;
- `FOR` without its control variable;
- `FOR` without `:=`;
- `FOR` without `TO`;
- `FOR` without `DO`;
- `WHILE` without `DO`; and
- `REPEAT` without `UNTIL`.

This was a real product parser defect: malformed Structured Text could be
reported as a successful parse instead of a source error.

## Minimal product fix and green result

The grammar now emits the missing-token diagnostics at the existing recovery
points. It does not add a second parser or change the recovery-tree ownership.
The exact red test passed on the green source, as did the full `trust-syntax`
package test suite.

The additional nested-boundary test proves that a missing inner `END_WHILE`
diagnoses at `END_IF` while leaving the outer `END_IF`, `END_PROGRAM`, and the
following assignment available to their owning parse nodes.

## Posture and boundaries

- `IEC_PARSE_RECOVER_001` moves to `gap_open/S0`: the written oracle and mapped
  tests exist, but no producer-authentic red/green proof or causal broad gate is
  claimed.
- The closeout evidence uses `proof_kind = "none"`; focused command output is
  not promoted into product proof.
- No suite, workflow, approved proof producer, validator, board row, or CI
  enforcement changed.
- The broad builder gate and commit-bound report refresh are intentionally
  batched with the next product vertical rather than repeated for this one fix.
