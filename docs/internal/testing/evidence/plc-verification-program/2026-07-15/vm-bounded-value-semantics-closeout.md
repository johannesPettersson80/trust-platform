# VM Bounded-Value Semantics Closeout

- Date: 2026-07-15
- Product source revision: `193be5876917c7ec032c2d1b67dc49186e8b7365`
- Evidence posture: specification closeout and focused regression evidence;
  this is not producer-authentic red/green proof.

## Reproduced Defects

Tests-first probes exposed concrete product defects before the implementation
was changed:

- semantic analysis accepted typed `DINT -> REAL` and `LINT -> LREAL` even
  though some source values require rounding;
- ordinary subrange variable initializers were not checked against the declared
  lower and upper bounds;
- assignment normalization rounded a nonrepresentable `DINT` into `REAL`;
- the VM primitive policy accepted incompatible runtime tags after failed
  normalization; and
- contextual literals for named subranges reached the VM with the base literal
  tag instead of the declared subrange base tag.

## Closed Boundary

The product specification now defines a closed accuracy-preserving implicit
conversion matrix, bounded string runtime writes, subrange initializer and
runtime-write behavior, and fail-closed primitive-tag handling. The compiler,
lowerer, runtime-core normalization, and VM type policy implement that contract.
`SPEC_GAP_VM_VALUE_SEMANTICS_001` is closed against
`SPEC_VM_VALUE_SEMANTICS_001`.

`SPEC_GAP_VM_ERROR_MODEL_001` remains open. The focused tests assert the current
typed internal errors or diagnostic categories without claiming a stable public
error identifier.

## Focused Validation

Run on `trust-builder` against the source-equivalent validation checkout using
`CARGO_TARGET_DIR=$HOME/.cache/codex-targets/trust-platform-gate`:

- bounded-value HIR module: 6 passed;
- bounded-value runtime integration: 3 passed;
- VM primitive policy module: 4 passed;
- runtime-core non-widening normalization regression: 1 passed;
- coercion proof integration: 7 passed;
- computed subrange runtime paths: 3 passed;
- bounded string runtime integration: 15 passed.

The broader batch gates are recorded separately after the metadata checkpoint;
they do not upgrade this evidence row beyond `proof_kind = "none"`.
