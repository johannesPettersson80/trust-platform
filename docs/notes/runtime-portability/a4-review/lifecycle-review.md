# A4 lifecycle source inspection

This is source inspection only, before the first A4 validation batch. Canonical
AGENTS.md and the complete skills tree matched the primary checkout at
`/home/johannes/projects/trust-platform`; the reviewed worktree was
`/home/johannes/projects/trust-platform-portability-a4`, branch
`feat/runtime-portability-a4`, base `77b91381f`.

The adjacent JSON enumerates the exact current source paths and SHA256 values.
Its aggregate identity covers those records, not a claim to have reviewed every
change inherited by the worktree. Exclusions in that file are part of the verdict.

No additional lifecycle correctness blocker was found in the inspected paths:

- Public RuntimeState wraps private EngineState and does not implement the
  dispatcher traits exposing mutable untyped storage.
- Raw CALL suspends its caller before initialization and resumes on return;
  native calls resume even on error. Outer dispatch cleanup retires remaining
  frames. Initializer frames restore lexical locals and retire staged storage.
- Restart builds a candidate at the current logical time. Owned retained object
  graphs transfer identity, while references/interfaces to artifact roots rebind
  without preserving non-retained object state. Function static storage remains
  outside retained resource-global import.
- Output copyback restores storage and current caller locals on commit failure;
  work and allocation charges remain consumed. Input and output image updates
  stage their value/image changes before publication.
- Hosted stack wrappers delegate to the core dispatcher; debugger hooks,
  optimized register execution and legacy HIR initialization remain hosted.

The earlier runtime-execution diagram note was contradictory. Root corrected it
to say legacy Runtime import stays 1.x while PreparedModule admits 2.0 and
RuntimeState executes it; the corrected lines were reread. Diagram rendering and
drift checks remain unrun.

Allocation accounting remains a conservative logical charge, not a complete
allocator ceiling: several frame/container reservations allocate outside that
counter. This does not establish the M3 bounded-storage or Scope B physical-peak
claim. The separately discovered copy-on-write path charge was implemented by
this reviewer afterward and requires another agent's independent review.

No tests, builds, formatters, validators, diagram rendering or hardware checks
were executed for this review. Assignment/cycle/provenance code authored by this
agent and the new path-write accounting are explicitly not independently
certified here. Decoder/prepared-admission changes are excluded while their
owner and reviewer finish them.

## Retained graph indexing follow-up

Independently read the monitor-authored `retained_graph.rs` correction, SHA256
`5db0ff6c3706d0f7c3fb82d8a242c2b842bf69057675b0314ee214527f0bb543`. Source initialized/once marks are indexed once by
owner; destination artifact root indices are collected alongside initial identity
mapping. Transfers then select only the copied owner's marks and mapped roots.
This preserves the previous retention rules and ordering while removing repeated
global mark/root scans. B-tree operations receive a logarithmic comparison bound;
the remaining declaration lookup performs and charges its actual layout scan.
Index reservation/insertion charges precede those operations. No additional
semantic blocker was found in this correction. No tests or validators were run.
My constant protection and COW writes are excluded from this independent review.
