# HIR Index Expression Risk Review

## infer-index-expression-outcome

The `infer_index_expr_outcome` mapping is specified by
`SPEC_IEC_DATA_TYPES_CANDIDATE_001#3-4-array-data-types-section-6-4-4-5` and
`IEC_HIR_INDEX_VALIDATION_001`.

The reviewed body accepts a nonconstant `ANY_INT` index without rejecting it
from the index type's declared subrange alone. Constant index expressions still
receive static bounds validation, and runtime bounds validation remains
responsible for the actual value of a nonconstant index.

The source identity, owner (`language`), area (`compiler_iec`), visibility, and
`state_mutation` risk signal are unchanged. The acknowledgement is rebound to
the reviewed body only; it does not infer correctness or create proof beyond
the cited specification, invariant, and mapped tests.
