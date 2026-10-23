# Scope B-R1: shared runtime footprint corrections

Status: function-preserving reductions and reviewed Rust corrections pass native tests,
Clippy, target checks and architecture through runs 2–3. The separately authorized
run 4 passed diagram rendering, drift and diff checks. Last measured firmware is
34,784 B over L2. Physical board execution remains unrun. Failed batches are retained.

## Scope and source

Worktree: `feat/runtime-portability-b`, based on A4
`df427259cc387a7a79fb81132be23e48ea1493d4`. The canonical agent files are copied from
`/home/johannes/projects/trust-platform`. This scope implements the owner's five
explicit decisions: L2, shared core reductions, targeted collection/sort changes,
retained overflow/admission checks, and an expiring dependency exception with both
lockfiles audited. No commits, push, profile feature cuts or M3 redesign are included.

The B run-1 map had 487,876 bytes of `.text` and 77,580 bytes of `.rodata`.
Its SHA-256 is `d12d328a476fa000979f789e3154e10d28ccf22506dbd9946051c6a815f60992`.
L2 provides 448 KiB for code (sectors 4–7), with vectors in sector 0, application
in sector 1 and two reserved 16 KiB checkpoint sectors. Those reservations do not
implement persistence. The previous map implies approximately 104 KiB still needs
removing to link under L2; this is not a claim of adequate release headroom.

## Initial implementation allocation (expanded later below)

- `PreparedModule::from_bytes` bounds the actual input and decodes once, then uses
  the full validator and preparation path. Struct-built modules retain bounded
  serialization checks. Neither route bypasses admission or resource accounting.
- One core IEEE CRC-32 API serves bytecode and firmware. Hosted std builds retain
  crc32fast; no_std uses a compact table-free implementation. Wire bits stay unchanged.
- All 89 default standard functions use immutable descriptors, with an empty owned
  override map. Case-insensitive lookup, custom registration and override precedence
  remain. `StdParams` fixed-name payloads now use `Cow<'static, [SmolStr]>`; in-tree
  Rust constructors are adapted. This is a Rust source-API adaptation, not an IEC
  function/signature change.
- Eight build-once grouped preparation indexes use sorted keys and contiguous
  value ranges, preserving key order and per-key wire order. Explicit-action
  membership uses sorted values. RUN-path trees and the static-ordinal map remain.
  This is not a blanket BTree-to-insertion-map replacement.
- A shared non-inlined heapsort driver preserves the validator's comparison, swap,
  tie and charging sequence. Prepared sorts charge actual work. Group construction
  adds temporary buffers; the allocation measurement must expose their cost.
- Linker and ELF inspector agree on L2. Firmware overflow checks remain enabled.
  Identical-code-folding flags and broad diagnostic rewrites remain outside this scope.
- RUSTSEC-2026-0110 has an owner, rationale, removal condition, review date and expiry
  of 2027-01-08. Root and standalone firmware locks both run cargo-deny and audit;
  independent graph failures are collected. Hosted root checks still see the adapter.

The earlier native-tool include path and range-check Clippy corrections are included.

## Validation allocation

Implementation and native test authoring precede independent review. One final
preparation phase resolves changed lock metadata without broad dependency upgrades,
formats both workspaces and touched include fragments, and refreshes source bindings.
The source and commands are frozen before the consolidated batch. The retained batch
script records individual exit codes; there are no automatic reruns.

Required evidence: core all-features and no-default suites; 32-bit musl execution
including allocation measurements; hosted unit and affected integration suites,
including the runtime vertical and reference allocation assertions; native firmware
bundle, adapter and xtask tests; F401 and C6 core graphs; release firmware link;
affected Clippy and cross-target warnings; both lockfile audits; architecture and then
canonical diagram rendering/drift; formatting and diff checks; unchanged A4 artifact
identity, bundle packing and conditional ELF inspection.

The footprint tool retains the new map even if linking fails and compares disjoint,
approximate symbol families against run 1. Aggregate changes cannot establish causal
savings for each individual edit. Native 32-bit counting-allocator results measure
requested allocation payloads and lifetimes, not LLFF overhead/fragmentation, MSP,
interrupt demand or physical timing. They do not certify the 72 KiB firmware arena.

Only a successful link and partition inspection permit the autonomous board phase.
That phase uses the unchanged main/numeric artifacts plus the GPIO probe and retains
heap/stack/timing, output and watchdog evidence. No button press or printed board
marking is assumed while the owner is away. A failed prerequisite leaves dependent
hardware steps unrun and the corresponding checklist items open.

## Review and evidence

Independent source review found no additional definite blocker and pinned 75 source
files. The external record is `.artifacts/runtime-portability-b/b-r1/review/` under
the projects directory. Review identified lockfile refresh and final formatting as
freeze prerequisites. The authored external `b-r1/builder-preparation.sh` and
`builder-batch.sh` are undergoing command review; neither has been executed.
Current code is unverified by execution.

A later external review proposes feature cuts, ICF, compact diagnostics and an
unchecked-overflow measurement variant. These are suggestions, not implemented or
implicitly authorized changes. Its estimated savings overlap and do not describe
exactly the selected collection changes above. The owner explicitly authorized one linked-size measurement before the final validation
batch, then directed: finish the approved reductions, measure the image, and use the
map to select remaining reductions. This exception includes no tests or hardware and
no automatic retry. Final source formatting and lock authoring are complete: no
registry version upgrades, root lock unchanged, firmware lock removes crc32fast and
cfg-if only. Independent review reconciled 6,650 source records, manifest SHA-256
`c30b734de5ceff1ab2232e7fb77f0ba53f1824ab6eae28b6c9b28e4245ae6761`.
The single firmware link failed because the image exceeds L2; the supporting native
map-report command passed. No retry, tests or hardware commands were launched. Raw measurement evidence is outside the repository under
`projects/.artifacts/runtime-portability-b/size-1/`.


## Preliminary size result

[Measurement ledger](b-r1-evidence/size-1-measurement-ledger.tsv),
[map report](b-r1-evidence/size-1-footprint.json), and
[linker log](b-r1-evidence/size-1-firmware-link.txt) retain the actual result.
The unchanged frozen source manifest is retained alongside them; only subsequent
status/evidence documentation differs. Raw maps remain outside Git.

| Quantity | B run 1 | B-R1 size 1 | Change |
|---|---:|---:|---:|
| Code span including alignment | 565,472 B | 526,976 B | -38,496 B (37.6 KiB) |
| `.text` | 487,876 B | 457,644 B | -30,232 B |
| `.rodata` | 77,580 B | 69,300 B | -8,280 B |
| `.bss`, including fixed heap | 73,788 B | 73,788 B | unchanged |
| Excess over L2's 458,752 B code region | 106,720 B | 68,224 B | -38,496 B |

The linker reports 68,196 bytes beyond FLASH at `.rodata`, then 68,224 after
`.gnu.sgstubs` alignment. Report the aligned 66.6 KiB shortfall. This is a size
measurement, not functional validation. There is no installable ELF and no hardware
result. All functions and overflow checks remained enabled.

Approximate disjoint text-family changes: encoder -7,002 B (now absent), sorting
-9,702 B, BTree -2,516 B, preparation -2,432 B and validator -2,450 B.
These family changes are not isolated per-edit measurements. The source-derived
savings estimates were optimistic for the actual selected changes. Remaining BTree
text is 29,962 B and sorting text 15,560 B; neither can be assumed removable in full.

Next: inspect the retained map to select remaining behavior-preserving reductions.
Do not spend the full validation batch on a source already known not to link, and
do not silently add restricted IEC profiles, disabled overflow checks or repeated
measurement links. No new measurement is authorized by this result.


## Selected remaining reductions (not implemented or measured)

Read-only map/source review selected these next behavior-preserving candidates:

1. Remove remaining per-type standard sorting from section ordering, recipe ordering
   and ready-task ordering using the existing shared driver. Decoder ties must retain
   original wire order; ready tasks retain priority/due/index order. Replace standardized
   section-ID membership with a bitset for the closed known-ID set.
2. Avoid hidden bulk-constructor sorting: insert method owners into their current map,
   and move the initializer staging traversal's existing visited-instance set instead
   of rebuilding identical membership from backups. Preserve charging and ownership.
3. Share identical immutable standard-library parameter lists across function families.
   The 155 emitted SmolStr slots represent 20 unique shapes with 51 slots; approximately
   2,496 bytes could be avoided without changing the public registry representation.

The map contains 14,620 bytes of remaining standard sorting and 29,962 bytes of BTree
code. These are current costs, not promised savings. Replacement/adaptor code remains,
and removing both entire families would still not cover the 68,224-byte gap. Some
related code is attributed elsewhere, so this is a conservative planning warning,
not a mathematical proof that a full-function F401 runtime is impossible.

The immutable registry introduces 8,704 bytes of read-only metadata (4,984 function
records plus 3,720 parameter names), explaining why removal of the 16,384-byte CRC
table produces only an 8,280-byte net rodata reduction. This eliminates repeated heap
construction but is a real flash tradeoff. Do not describe it as a measured flash saving.

Closing the remaining gap needs a wider compact-core decision as well as these small
reductions. Compact preparation diagnostics are a candidate, but were explicitly
excluded from the approved five-point correction scope. Full M3 storage redesign,
feature restrictions, speculative linker changes and disabled overflow checks remain
outside scope. The final validation batch is held rather than spending it on the
currently known link failure. The early measurement authorization is consumed.


## Expanded implementation authorization

The owner requested implementation of the recommended function-preserving set after
reviewing size-1. This supersedes the earlier deferral of compact diagnostics, targeted
engine/retained-graph representation and safe ICF evaluation; it does not authorize
conversion removal or the full M3 storage redesign. The fallback capability is a written
contract only. Keep every current IEC and ASSERT operation and overflow checks enabled.

Finish the expanded code and test authoring, independently review, then allow at most
two retained size-only comparisons of the complete implementation: normal linking and
safe identical-code folding. No tests or hardware in these measurements. They do not
replace the single consolidated native/cross-target/dependency/architecture/hardware
batch. No automatic validation retry, publication or commits. If the measured image
still cannot fit, preserve both maps and report the unresolved resource constraint.


## Uniform-function clarification

The owner explicitly rejected reduced MCU function sets after authorizing expansion.
The earlier suggested restricted F401 fallback is withdrawn, including its documentation
as a planned option. STM32 and ESP32 keep the same IEC operations, standard functions,
conversions and ASSERT operations. No feature flag will remove them. Hardware limits
and physical I/O configuration remain target-specific. Failure to fit must be reported
as a resource blocker, never solved by silently rejecting additional PLC functions.


## Expanded implementation and bounded measurement closeout

All current PLC functions remain. Implemented remaining shared sorts and comparator
adapters, decoder known-ID bitmap, checked lifecycle/retained keys, reuse of staging
membership, allocation-free owner inference, canonical borrowed library signatures,
and structured portable diagnostics. Hosted text/error behavior stays intact; portable
faults retain typed context and render only on request. Structural fault equality does
not allocate. Removed only the obsolete owner-inference set allocation charge; actual
body-visit work remains charged. Scheduler sorting now charges actual work explicitly.

`StandardLibrary::get` returns `StdFunctionRef` borrowing a canonical signature. Core
`StdParams` and registration APIs remain; all 89 default functions and algorithmic
conversions remain. no_std `InvalidSection` details are typed; explicit text inspection
preserves messages, while fault equality compares structure. A portable expectation
now names the existing typed rejection and also pins its prior exact rendered message.
These Rust representation adaptations do not reduce PLC capabilities.

Independent review pinned all 107 dirty Rust paths before measurement. Formatting and
review produced a 6,661-record snapshot. The ordinary attempt stopped at compilation:
`heap_sort` error-type inference was ambiguous at the decoder's `?` boundary. The sole
correction specifies `heap_sort::<BytecodeError>`; it was independently reviewed before
using the second and last authorized attempt with safe ICF. No ordinary retry occurred.
There is therefore no same-source ordinary-link baseline for isolating ICF savings.

The second attempt compiled the runtime and reached the linker, retaining a new map
and actual folding output. The complete source snapshot has 6,663 entries, SHA-256
`94e91e74b5df04acd07bd7ff22e3016115da4b2c7fc2e1eba0cc85590c7ba4f4`.
The source and map were independently reconciled. The map, binary objects and complete
raw logs remain external; curated [ledger](b-r1-evidence/expanded-measurement-ledger.tsv)
and [footprint report](b-r1-evidence/expanded-footprint.json) are retained here.

| Quantity | Expanded safe-ICF candidate |
|---|---:|
| Code span, including alignment | 493,536 B (482.0 KiB) |
| L2 capacity | 458,752 B (448 KiB) |
| Remaining overflow | 34,784 B (34.0 KiB) |
| Total span reduction from B run 1 | 71,936 B (70.25 KiB) |
| Further reduction from size 1 | 33,440 B (32.66 KiB) |
| `.text` / `.rodata` | 435,476 B / 58,028 B |
| `.bss`, including the reserved heap | 73,788 B |

The object-inspection step retained exit 1: the glob selected an LLVM-bitcode `.rcgu.o`
as well as the real ELF object, and readelf rejected the bitcode. The retained ELF
section output does contain `.llvm_addrsig`; the linker log records actual safe folding.
Keep the failed inspection status. Do not pretend it was rerun or convert it to PASS.
Safe ICF was measured through explicit command flags; the default firmware config has
not been claimed to link. All overflow checks remained enabled. No hardware ran.

The two-attempt measurement allowance is consumed. The software batch described below
checked native behavior, both MCU graphs, 32-bit heap estimates, lint, dependencies and
architecture without repeating the firmware link or flashing the board. The failed size
prerequisite leaves physical checks unrun. Software verification cannot close F401 fit
or the Scope B hardware gate.


## Consolidated software validation: run 2

[Ledger](b-r1-evidence/run-2/ledger.tsv),
[artifact index](b-r1-evidence/run-2/artifact-sha256.json), and
[frozen changed-path manifest](b-r1-evidence/run-2/formatted-source-manifest.json).
Started 2026-10-10 16:45:34 UTC; finished 16:57:27 UTC. No retry.

| Native evidence | Passed | Failed / ignored |
|---|---:|---:|
| Core all features, including doctests | 352 | 0 / 0 |
| Core portable no-default, including doctests | 312 | 0 / 0 |
| i686 musl portable library and seven integration suites | 232 | 0 / 0 |
| Hosted runtime unit suite | 3,841 | 0 / 0 |
| Hosted runtime integration, 68 binaries including runtime vertical | 538 | 0 / 0 |
| Native platform and xtask | 84 | 0 / 0 |
| Native firmware bundle | 1 | 0 / 0 |
| Provenance helper unit tests | 4 | 0 / 0 |

These are executions across configurations, not distinct-test counts. Both F401/C6
library checks and no-dev feature trees passed. Firmware Clippy, cross-target warnings,
both lockfile audits, mutation tooling/contracts, advisory metadata, formatting, diff,
application bundle packing and original artifact parity passed. The audit includes the
reviewed bare-metal exception; it does not assert that the advisory disappeared.

Three required commands failed: affected Clippy and portable Clippy; architecture.
The two lint causes are the decoder's modulo alignment expression and the common Clone
suffix remaining in RegisterValueOpKind when HIR-only variants are absent. Architecture
requires a module owner/split note for the 5,044-line aggregate standard-library tree.
Diagrams and drift remained unrun. The ledger preserves the prior failed firmware-size
prerequisite and unrun ELF/hardware steps. B-R1 is not fully verified or push-ready.

32-bit allocation payload peaks (not MCU allocator/stack proof):

| Artifact | Preparation peak | Instantiation peak | Scan peak |
|---|---:|---:|---:|
| Main | 28,069 B | 23,033 B | 25,311 B |
| Numeric | 23,550 B | 19,973 B | 22,685 B |
| GPIO | 13,014 B | 11,327 B | 12,731 B |

The maximum is about 27.4 KiB. All captured state retired to zero live bytes. These
estimates exclude allocator metadata/fragmentation, ARM ABI differences and native/IRQ
stack use. Scans still allocate in this bring-up engine: this is not M3 bounded-RUN
qualification. No physical trace, stack-paint or timing evidence exists yet.

### Prepared post-batch corrections — not rerun

1. Use `!entry.offset.is_multiple_of(4)` for the identical alignment check.
2. Rename the four shared value-profiling events to LoadConstant, ReadReference,
   BindExpression and CopyOutput, updating all seven enum/caller files. Counter fields
   and increments remain unchanged; hosted-only register clone/move events remain.
   Rust enum/debug spellings change; serialized profiling counters do not.
3. Record the standard-library owner and existing responsibility split in the architecture
   policy. No line threshold is increased and no diagnostic is suppressed.

Independent read-only [correction acceptance](b-r1-review/correction-acceptance.md)
found no blocker and pins nine corrected files. They have not been formatted, compiled,
linted or tested after this batch. Any further validation requires explicit authorization.
No further size link, hardware command, commit or push has been launched.

## Correction batch run 3 — authorized

The owner answered “do it” to one correction-validation batch. The nine-file
reviewed correction is complete. Run 3 covers core all-feature and portable tests,
F401/C6 library checks, hosted unit tests and the four runtime-vertical suites,
affected/portable Clippy, cross-target warnings, provenance/metadata checks, full-map
architecture, canonical diagram rendering/drift and formatting/diff. Formatting
and current provenance refresh precede the frozen source. Existing run-2 evidence
continues to cover unchanged dependency graphs, i686 allocations and wider integration.
No firmware link, hardware, commit or push is included. Failures are retained without retry.
Scripts and raw output: external runtime-portability-b/run-3; builder run-3 evidence cache.

### Run 3 result and remaining diagram correction

[Ledger](b-r1-evidence/run-3/ledger.tsv) and
[retained evidence](b-r1-evidence/run-3/README.md). This explicitly authorized batch
finished 10 October 2026 at 17:11:59 UTC, without retries. The 303-record frozen
manifest is `09d7a5da97080b84c13aee943f9eeea870f65cdab4aca896f17fdabaadc80a3e`.

| Native suite | Passed | Failed / ignored |
|---|---:|---:|
| Core all features, including doctests | 352 | 0 / 0 |
| Core portable, including doctests | 312 | 0 / 0 |
| Hosted runtime unit | 3,841 | 0 / 0 |
| Runtime vertical, four binaries | 28 | 0 / 0 |

All three prior root causes are validated: both Clippy lanes and full-map architecture
pass. F401/C6 library checks, host/Windows warning checks, provenance tooling (16 + 20
Python tests), advisory metadata (1,029 records), formatting/diff and saved-artifact
parity also pass. Architecture's optional historical mutation shards remain PARTIAL;
this is not new mutation execution evidence. There are 24 required PASS rows, two
advisory PASS rows, one required FAIL and one required UNRUN.

The sole failure is diagram rendering: a quoted note used literal newlines at the end
of runtime-bytecode-vm-execution.puml. PlantUML requires a block note or escaped line
breaks there. The source now uses `note as FootprintDetails` / `end note`, preserving
all text. This correction is not rendered or validated. Diagram drift remained unrun.
The renderer's error SVG and other outputs are archived externally, not adopted.
No Rust file was edited after the tested freeze; only documentation/evidence and this
diagram correction changed. All functions remain enabled on every platform.

Next executable action requires authorization only for canonical diagram rendering,
drift and diff checks. No native rerun is needed for this diagram-only correction.
No firmware link, hardware, commit or push was performed. Last linked-size evidence
still shows a 34,784-byte overflow; further function-preserving size work is required
before the F401 hardware gate can proceed.

The diagram-only [independent review](b-r1-review/run-3-acceptance.md) accepts the
syntax correction and inspected the remainder of the diagram. Corrected source hash:
`391fce298f7bded002205d9030adf9d9c2a7877389486b2da22c85549662cb2a`.
It is source review, not rendered proof. Pending authorized commands are
`bash scripts/render_diagrams.sh`, followed on success by
`python3 scripts/check_diagram_drift.py`, and `git diff --check` on the builder.

## Run 4: diagram-only correction validated

The owner explicitly authorized this follow-up with “yes”. The three commands listed
above ran once on the builder and all passed, 10 October 2026 17:15:03–17:15:06 UTC.
[Ledger](b-r1-evidence/run-4-diagrams/ledger.tsv) and
[evidence index](b-r1-evidence/run-4-diagrams/artifact-sha256.json) retain exact source,
script, command, output and generated hashes. Canonical agent files matched the primary.
The corrected source equals the independent review identity. The canonical container
render succeeded; generated SVGs and manifest were copied back byte-for-byte. Only
the VM SVG changed content. No runtime source or assertions changed, and no Cargo,
native test, firmware link, device action or commit ran in this diagram-only batch.

B-R1 software checks are now satisfied by the reconciled runs 2–4. This is not a
successful firmware link, Scope B hardware qualification or release-guard result.
Full functionality, overflow checks and admission remain intact. The last measured
candidate still exceeds the F401 L2 code region by 34,784 bytes. Further size reduction
and an explicitly authorized measurement are needed before physical execution.
