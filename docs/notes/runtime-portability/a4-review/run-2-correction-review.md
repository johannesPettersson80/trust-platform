# A4 run-2 correction source review

No actionable source finding in the three paths pinned by the [manifest](run-2-correction-review.json).

- The raw Retain/I/O regression now appends a typed External declaration covering
  the new reference. Its zero demand, cleared flags/default/related/source-name,
  Global ownership and absent POU owner agree with the existing wire rules.
  External entries do not acquire dense owned slots, roots or initializer actions.
  The positive validated_source_free assertion remains before profile rejection;
  no production validator or admission policy was weakened.
- The mutation selector assertion keeps exact mutation id/function/operator and
  selector syntax, resolves the recorded line/column through existing source_offset,
  and verifies the selected bytes are the actual `==` in `if src == dst {` within
  convert_value. Formatting may relocate the operator; stale or different selection
  still fails. New source pins require the planned batch refresh, not this review.
- core::mem::size_of_val(dims) receives a borrowed [(i64, i64)] slice and equals the
  previous slice-length times element-size charge. Surrounding checked arithmetic,
  error propagation and allocation order are unchanged.

This is source inspection only. No tests, builders, formatter, validator or
provenance refresh ran. The reviewer's thirteen-file cleanup is explicitly
excluded and awaits separate independent review.

Aggregate SHA256: `688313b070213780efda64b0559c88092a998583cceb9ab1c93ef738a9a930e9`.
