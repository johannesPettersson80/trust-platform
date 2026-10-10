# A4 run-1 provenance correction

Run 1's advisory provenance refresh failed before writing accumulated source/case
pin updates. The saved discovery output for MUTANT_RETAIN_ON_WARM_FALSE targets
runtime/retain_snapshot.rs, which now re-exports the function from core/retain.rs.
Its candidate list therefore has no matching function. Active manifest ownership,
the reviewed contract and its source-path assertion now point to the core owner.
The hosted runtime_restart test remains the execution oracle. Historical measured
reports are unchanged. Digest/selector refresh remains a future consolidated-batch
step; the new selector was not guessed from source line numbers.

Source-only audit of existing run-1 discovery JSON:

- validate_pou_index and validate_stack_shape each have one matching candidate.
- convert_value has one matching candidate; recorded location is dispatch.rs:86:16.
- check_subrange_assignment has one matching candidate.
- Parser::recover_top_level_until has three matching operators; the existing exact
  selector picks parser.rs:238:21, preserving expression identity.
- retain_on_warm has zero matches in its former owner, the diagnosed failure.
- ADS status discovery was not reached. Its named function and comparison remain
  in connectors/mapping.rs; this is source inspection, not fresh discovery proof.

The helper now reports mutation id, source path, matching-candidate count and
requested function/genre/replacement/selector on failure. Selection is a small
separate function with synthetic native tests for exact selection among three
operators, ambiguous rejection, zero-match diagnostics and unique FnValue
relocation. These tests are authored, not executed. No discovery, validator,
formatter, build or test was rerun during this correction.
