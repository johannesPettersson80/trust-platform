# A4 batch orchestration source review

Read-only review of `.artifacts/runtime-portability-a4/setup/`:
`run-on-builder.sh`, `format-once.sh`, `freeze-source.py` and
`prepare-metadata-index.py`. No scripts, syntax checks, tests, builds, formatters
or validators were executed. The provenance helper is this reviewer's own code
and is excluded from independent assessment.

The 63 explicit hosted integration suite paths and five i686 integration suite
paths exist, without duplicate suite names within those lists. Core all-features
and no-default-feature commands cover their separate feature configurations.
The numeric fixture generator precedes source freeze and numeric execution;
`source_free_numeric` reads its artifact at runtime, so the exact test skip after
generator failure prevents a dependent behavioral run without preventing other
core tests from compiling. Original saved-fixture parity is a separate required
step. Failure does not erase its result.

The script pins Rust 1.95.0, unsets inherited CC/CXX and bootstrap flags, and uses
the self-contained i686 musl linker. No retry loop was found. The existing
progress wrapper terminates timed-out processes instead of retrying them.
The outer target lease, canonical bootstrap, installed targets/tools, sufficient
space on each filesystem, builder workload coordination and separate archival
of ignored setup scripts remain launch prerequisites owned by the root agent.

Three corrections were requested before launch:

1. Remove register_ir/interpreter.rs, register_ir/profile.rs and
   register_ir/tier1/execute.rs from the explicit fragment formatter list: they
   are real module children already formatted by `cargo fmt`.
2. Include staged-only changes in the source manifest, or explicitly reject a
   nonempty index. The current local index was empty, but `git ls-files
   --modified --others` alone is not a complete candidate identity.
3. Record disk/format/freeze prerequisite failures and UNRUN rows for dependent
   planned gates on early exit, rather than leaving them absent from the ledger.

Generated diagrams and their manifest are produced after the source freeze;
retain their separate output hashes and record those generated-only deltas at
closeout. The isolated metadata index leaves the real index untouched. Advisory
metadata failure does not override native/runtime gate results.

This record reports source inspection and requested corrections, not batch
execution or validation success. Corrections require a follow-up source read.

Follow-up source read: the three duplicate formatter entries are removed; a
required staged-index check now rejects staged-only candidates; early prerequisite
exits fill remaining ledger rows and write result/FINISHED. Successful numeric
fixture generation now has its own explicit ledger row. One final bookkeeping
correction was requested: early-exit UNRUN rows for provenance-refresh,
metadata-index and metadata must keep their advisory classification instead of
incrementing required failures. No script execution occurred.

Final follow-up: early-exit advisory classifications are now preserved. No
remaining orchestration blocker was identified by source inspection. The following
SHA256 pins identify the reviewed setup bytes; they are not execution evidence:

- `run-on-builder.sh`: `0124bd58323d297bfa13be0bbd3ba93c282cde55245a9599b82f3c3626b7cc98`
- `format-once.sh`: `7cbcf1b0aa72c382a0e499d0a7d2d4dfe5a032d182d40a6ab62add122cd68f30`
- `freeze-source.py`: `bc0065203277ac91e5a98283a0e400315be9221f79133272003a0f1e33c5790d`
- `prepare-metadata-index.py`: `2a642875d9aac204e94634e4b104c432f2fc008888f8cb506369c50623e7ec42`

The final safety regression adds only `--test source_free_readonly` to the hosted
integration command (64 explicit suites). Removing that one argument reproduces
the preceding reviewed script hash exactly. Current `run-on-builder.sh` SHA256:
`f61581ec6cbe534e598f2a46ed87947662ce510c3345060c41b4339c7d9cba6e`.
No execution was performed for this follow-up.
