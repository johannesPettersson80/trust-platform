# Independent source review of A1 review-agent corrections after A4 run 1

Inspected these six paths beneath `crates/trust-runtime-core/src/vm/`:
`call/mod.rs`, `dispatch_refs.rs`, `debug_map.rs`,
`engine/initialization/construction.rs`, `prepared/budget.rs`, and
`construction/values.rs`.

Their reviewed identity is
`d44784dd311e92c3b056fef15a450d6135c274709cc6cac5256a38b30614e41a`,
SHA256 over sorted path bytes, NUL, file bytes, NUL for each entry. This matches
the author's supplied identity.

The source resolves the four concrete compiler diagnostics retained in run 1:
the frame constructor supplies an empty parameter-presence bitmap; the unused
InstanceId and test Vec imports are gone; the unused declaration binding is `_`.
An empty presence bitmap preserves ordinary frame initialization until native
argument binding supplies explicit presence information.

The preparation split gives POU metadata and I/O metadata separate helpers.
The TYPE_TABLE construction split gives arrays and structures separate helpers.
By inspection, the extracted branches preserve charge/evaluation order, error
propagation, owned input movement, intrinsic/default recipe behavior and recursive
depth increments. Struct field defaults still precede explicit overrides. Passing
name_idx by value preserves access after moving the type-data payload. Imports and
helper signatures are consistent with their callers. No waiver or threshold
change was introduced in this slice. No additional source blocker was identified.

This is independent source review of another agent's changes. It excludes this
reviewer's own assignment-normalizer and provenance-helper corrections. No tests,
builds, formatters, validators or mutation discovery were executed. Compilation
and behavior remain unverified until the next authorized consolidated batch.
