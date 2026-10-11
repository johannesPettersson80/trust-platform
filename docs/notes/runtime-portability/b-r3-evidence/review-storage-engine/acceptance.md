# B-R3 storage/engine independent source review

Reviewer: `/root/br1_static_registry`. Reviewed 19 files authored by the storage and root agents, excluding the reviewer's own numeric/calendar/error-constructor/inline slice.

Canonical AGENTS.md and complete .codex/skills verified against `/home/johannes/projects/trust-platform`. Active branch `feat/runtime-portability-b`, base `df427259cc387a7a79fb81132be23e48ea1493d4`. Authority: spec 34 section 6.2.1 and current B-R3 checklist.

Verdict: no remaining blocker in this reviewed source set. This is source review, not compilation or execution evidence.

Checks: complete u32 identities with checked exhaustion; sorted live-entry membership and arbitrary removals; reserve/clone preservation and separate ordinary/staging capacity; no_std/host cfg and public aliases; portable name lookup through existing hashed map preserving case, inheritance and shadowing; charged instance-array growth; all shared ExecutionContext implementors and retire callers; cleanup after errors/fuel exhaustion preserving original fault; promotion survivors and side-metadata retirement; exact location/slot journal keys, alias deduplication and sorted rollback; native test imports, fixture paths and applicable cfg boundaries.

Findings resolved during review:

- SavedDestinations exact one-slot growth undercharged cumulative reallocation and relocation work. Corrected to geometric fallible growth with precomputed full allocation and relocation/shift demand. Both snapshot and retained-global callers charge before mutation. Native growth/replacement and exhausted-snapshot tests authored.
- The new retained growth-demand method lacked its RuntimeError import. Import added and source inspected.

The active-journal snapshot caller passes exactly one reference; bulk I/O snapshots run without an active journal. Therefore pending active growth demand is valid for the current call graph.

Adjacent pre-existing observation (outside reviewed edits): io/staging.rs sample_input_image uses exact one-slot growth with per-slot allocation charges. Reported to root separately; this review is not an allocation audit of all engine paths.

No builds, tests, formatters, validators, hardware actions or repository edits were performed for this review. Source may be formatted later; this manifest pins the bytes read here, not future files. Historical moved code was inspected for integration, not re-certified as an exhaustive semantics audit.

Manifest SHA-256: `b02818e8e463882d6e667eade37cf7e06828b57dec9ac307815f8476f1a00a5a`.

## Supplemental I/O staging review

The adjacent observation is now resolved in the owning input and output paths. Each path charges its complete binding-count scan, counts the same predicate used by its write loop, checks count-times-slot bytes, charges that allocation, and reserves once before decoding or publishing. Subsequent pushes are bounded by that exact count; ordering is unchanged. Allocation failures leave bound input values and output images unchanged. The new native fixture regression pins both failure boundaries. No blocker found; no build/test/formatter execution.

`io-staging-identity.json` pins these two source files, superseding the earlier storage_tests.rs hash and adding io/staging.rs to the accepted set. SHA-256: `0f8abbea03497386ca6508d897fbe02416b63cf190e31ea45bc5600b9da8d420`.

## Formatted source reconciliation

All 20 accepted file hashes match the saved pre-format source set. Read the complete formatted diff: only whitespace, trailing commas and module declaration ordering changed. Current bytes match each corresponding entry in M1 formatted-source-manifest.json (SHA-256 `bbea428d5782d333477ea1beb5db234ef2725dc7fb7f6b2cb1fedb19dc674a47`). Source acceptance therefore extends to these formatted files for the planned measurement. No formatter/build/test/validator was executed by this reviewer.

`formatted-identity.json` SHA-256: `0fe91cc606ae9b14fdfdddfa9034fab548057125d5a1e4d43484cf17d7e6a09a`.
