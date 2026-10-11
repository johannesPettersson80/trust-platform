# A4 admission source review

Verdict: no remaining actionable blocker found in the paths pinned by
`admission-review.json`. This is independent source inspection, not executed
validation. Compilation, exact-limit assertions and behavior remain unverified.

The earlier byte-entry allocation blocker is corrected: the existing decoder
now charges actual typed vector reservations, string payloads, copied buffers
and section-validation scratch before allocation. Byte-entry preparation carries
those charges into the cumulative preparation budget. The hostile empty-string
fixture targets the original reservation gap; exact-limit tests cover decoded
and byte entry points. Constant expansion reserves arrays and structs only after
charging their actual shapes. Native import signatures and shared hidden-state
layouts are checked separately from wire validity. One-resource admission avoids
classifying another resource's programs as local background work. The legacy
entry points retain their fixed wire caps and do not acquire a selected profile.

The public RuntimeState wrapper has a private EngineState and explicit typed
operations. It implements neither Deref nor the dispatcher context traits, and
returns only immutable VariableStorage. The compile-fail example pins the prior
storage_mut escape once executed.

The provenance helper scans unit and integration sources in both crates, handles
shared old-to-new digest replacements, and leaves historical measured evidence
alone. Its current planned mutation files do not contain rewritten case pins.
Discovery completes before source writes; filesystem failures can still interrupt
its sequence of writes, so this is not an atomic multi-file transaction.

PreparationUsage describes logical cumulative demand, not allocator overhead,
RSS, stack usage or hardware qualification. The retained graph implementation
was authored by this reviewer and is excluded from this independent verdict.
