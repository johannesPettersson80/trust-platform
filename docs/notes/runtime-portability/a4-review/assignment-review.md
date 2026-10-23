# A4 assignment and context source review

Verdict: no remaining actionable finding in the reviewed other-author changes.
This is source inspection only; builds, tests, formatters and validators were not
run. The authored regressions remain unverified.

Source identity: `77b91381fcf1b1850611f88b397bbfe8d45e3523` with the dirty files recorded in
[assignment-review.json](assignment-review.json). The manifest contains 21
per-file SHA-256 hashes and byte counts. Its aggregate is
`f7da514bf67cdd4bc261c54ad18e1c990631b0369c78cf8b83354ac68364932e` using sorted relative path, NUL, raw bytes, NUL framing.
The expanded manifest includes the original twelve assignment/context review
paths plus the final COW, read-only, wrapper and integration paths. It identifies
current reviewable bytes rather than treating private agent messages as evidence.

The earlier findings are closed by source inspection:

- AR-01: string-element output copyback now falls back to the checked, charged
  materializer when a borrowed Value cannot represent CHAR/WCHAR. The regression
  calls an actual source-authored function with an output into `output_text[1]`.
- AR-02: path writes charge shared Struct backing and owned array children before
  `Arc::make_mut`, propagate sharing from copied ancestors, and charge string
  replacement buffers. Failed output groups restore storage and frame destinations;
  consumed work/allocation charges are retained.
- AR-03: instance traversal uses the selected storage throughout path charging,
  including the staged input-image snapshot.
- AR-04: whole program-root replacement is rejected. Paths through untyped program
  roots resolve their template members and enforce constant flags before proceeding.
  A native gate regression also covers mutable versus constant member paths through
  `Plant`, while always rejecting whole-root replacement.
  Constants are checked for direct, dynamic and native-output stores while initializer
  staging remains writable. The wire-mutation regressions remain unexecuted.

Ordinary assignment normalization does not call initializer recipes. The inspected
store and copyback paths normalize before commit, and the storage adapter does not
repeat that normalization. The public `RuntimeState` wraps private `EngineState`;
its API exposes typed writes and immutable storage without implementing the mutable
internal reference-context trait. Raw-call entry/return uses the common frame
initialization, retirement and resume hooks, with the hosted adapter retaining its
existing typed storage semantics.

This reviewer previously authored scalar stdlib/call extraction, reference helper
extraction, copy-read charging, typed construction, process-image/SIZEOF work and
part of the source corpus. Those earlier portions are **excluded** from this
independent verdict, even where the manifest necessarily hashes mixed-author files.
The review also excludes this reviewer's admission/decoder/budget implementation;
that requires a different reviewer. Specification inspection here covers the
assignment, COW, read-only and public-state contracts, not independent approval of
this reviewer's preparation/construction additions.

No release readiness, successful execution, measured memory peak, deadline bound
or hardware qualification is claimed.

## Subsequent source-only delta

The [External-binding shadowing addendum](assignment-review-external.md) records
the later seven-file correction and its current hashes. The original manifest
above is retained as the earlier review checkpoint, not rewritten as evidence
that the later bytes were already inspected then.
