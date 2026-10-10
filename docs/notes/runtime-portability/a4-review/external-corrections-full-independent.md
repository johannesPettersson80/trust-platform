# Independent A4 correction review (source only, active)

Reviewer did not author implementation. Bootstrap: manually copied canonical AGENTS.md,
CLAUDE.md and complete skills from /home/johannes/projects/trust-platform to
/home/johannes/projects/trust-platform-portability-a4; copies matched. Branch
feat/runtime-portability-a4, base 77b91381fcf1b1850611f88b397bbfe8d45e3523.
Architecture, native-test and IEC skills applied. No build, test, formatter or validator run.

## Findings requiring correction before freeze

1. Input staging/native output journaling still performs linear duplicate lookup per
   destination (`engine/output_transaction.rs`). Two thousand distinct input bindings
   require about two million work units just for duplicate lookup, exceeding default
   cycle fuel. Use an indexed physical-slot journal, retaining just-in-time resolution
   after preceding native OUT writes. Add a default-profile many-input regression.
2. Warm/cold restart checks completion inside candidate construction, but not after
   copying retained values/process images and before replacing the active state.
   A deadline can expire during the final sub-stride work. Add a final candidate
   deadline check before `*self = next` and assert failed restart preserves the old
   state. Independently found by the separate budget reviewer; see its own report.
3. Residual layout scans bypass prepared indexes: program dispatch scans the full
   layout per program; retain snapshot scans the full layout per program root;
   SINGLE task sampling scans all globals per task. Reuse existing prepared lookup
   groups and charge actual lookup/visited values.
4. Invocation-state errors in `memory/execution_frames.rs` still return TypeMismatch
   or NullReference for double suspend, invalid resume and expired IDs. Classify
   invariant failures with InvalidExecutionState and expired identity consistently;
   native assertions should pin those codes. Unknown access aliases should use
   InvalidAlias, consistent with the dedicated alias policy.
5. `io_image/mod.rs` is a new public module with blanket missing-doc suppression;
   its public prepared codecs/bindings and fields remain undocumented. Remove the
   new suppression, document the intended public API and narrow preparation internals
   where appropriate. Older untouched suppression debt is separate.

The budget reviewer also identified source-preparation coverage for the lifecycle
include fragment and allocator unsafe-line inventory, recorded separately.

## Coverage and positive findings

The full inventory includes every dirty Rust file and deletion rather than only
selected implementation slices. Initial snapshot: 237 records, 82 modified,
88 newly authored, 29 adapted moves, 3 byte-identical moves, 35 deleted source files.
Adapted moves include documentation additions, no_std imports and TypeId-to-
ConversionType migration; they are not represented as byte-identical evidence.
The initial inventory is `run-6-full-source-inventory.json`; final corrections and
formatting require an updated final inventory before validation.

Reviewed groups: bytecode decode allocation/work faults and preparation boundaries;
prepared declaration/member/type/root/action/edge indexes and fixed registry precharge;
engine state composition, lifecycle construction, staging and lifetime checks;
borrowed reference policy/traversal and destination rollback; process-image codecs,
input sampling/output staging; readonly/type/alias protection; public prepared-state
facade and HIR-only legacy adapters; host stack/register interface changes; standard
function/block relocation and conversion adaptation; new source-free and allocation
regressions; related specifications, evidence location map and batch inputs. The
separate independent budget reviewer deeply covers the shared fuel counter, root/
nested entry, optimized fallback, deadline stride and batch orchestration.

The VM facade no longer exposes source-free raw mutable execution metadata.
Legacy fixture mutation is explicitly limited to STBC 1.x and cannot forge a
PreparedModule. Normal portable consumers retain registry/block APIs without HIR.
Default initializers use the same dispatcher, and the forged dynamic-store test
corrupts private admitted metadata to exercise runtime protection independently
without adding a public bypass. The borrowed-reference allocation test counts actual
System allocator calls after warming host caches, rather than claiming allocation
proof from a pointer or estimated budget. The large declaration/store regression
keeps default limits. New 2.0 tests author TOF, TP, CTU/CTD/CTUD, triggers, bistables,
F_EDGE, hierarchical I/O and mid-cycle deadline/output withholding.

All 255 external historical artifact map entries were independently read and their
sizes and SHA-256 digests match. Original run inventories remain immutable. This is
preservation evidence, not execution proof of the current correction candidate.

No source-level claim certifies compilation or hardware. Run 5 remains historical;
run 6 is unrun and no A4 commit/push/hardware claim is made.

## Final pre-format disposition

All findings above were corrected and independently reread before freeze:

- Destination journals now use a physical-location/slot BTreeMap. Both membership
  and insertion work and storage are charged before mutation. Native copy-back
  still resolves destinations immediately before each write; redirected aliases
  and duplicate original values retain transactional rollback. Two thousand
  distinct DINT inputs now have an authored two-scan default-profile regression.
- Restart samples the candidate deadline immediately before replacement. The new
  cold-restart test explicitly expects fresh-construction polls plus one and proves
  failed publication preserves the old value and does not fault the old state.
- Program/SINGLE lookup uses prepared names/slots; retain snapshots use charged
  retained-declaration groups preserving lexical owner and original wire order.
  A unit assertion distinguishes wire order from construction slot order. Retained
  struct remapping resolves aliases and uses the prepared type/member index.
- Invocation-state and unknown-alias faults now have the intended dedicated codes
  and exact-code assertions preserving prior local vectors/state.
- New process-image public API documentation replaces the blanket suppression;
  PreparedModule remains the admission boundary. PreparationUsage fields and
  hierarchical clone demand are documented.
- The formatter includes lifecycle.rs and both newly changed register-tier include
  fragments. The seven allocator unsafe sites match policy lines 26, 27, 30, 32,
  34, 36 and 39. Run 6 retains the reviewed preamble, compares it after formatting,
  and fails before freeze with a complete prerequisite ledger if positions change.
  No automatic policy rebinding or suppression is introduced.

No remaining implementation blocker was found by this source review. Compilation,
all required native assertions, lint, architecture and diagrams remain unverified
until the one consolidated batch. The final pre-format source/orchestration pins
are in `run-6-full-source-inventory.json`; the earlier finding snapshot is preserved
in `run-6-initial-source-inventory.json`. All 237 Rust records are included. Formatting
and authorized provenance generation are expected preparation changes; the batch's
frozen manifest must independently record the bytes actually executed. This review
is not release, hardware or zero-defect certification.
