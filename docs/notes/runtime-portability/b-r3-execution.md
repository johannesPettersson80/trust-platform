# B-R3 execution record

Owner authorized “implement it” after agreement on full-function storage reductions,
combined frame/staging capacity, two bounded size measurements and one consolidated
software/conditional hardware batch. M1/M2 and two later explicitly authorized links are retained. Run-2 software and
run-3 ELF inspection pass; the single physical attempt fails stack headroom. The board
is halted. No automatic retry, commit or push occurred.

Base df427259cc387a7a79fb81132be23e48ea1493d4, branch feat/runtime-portability-b.
Canonical rules/skills from /home/johannes/projects/trust-platform were manually copied
and byte-verified in the active worktree before implementation. B-R2 final measured
upper span 476,256 B, overflow 17,504 B; required reduction 33,888 B for a 16 KiB margin.

Root owns spec/checklist, engine journal/bookkeeping integration, layout/trace decisions,
measurement/validation scripts and evidence. Storage agent owns VariableStorage and
portable lookup/caches. Numeric agent owns full-domain arithmetic and selected error/
inline changes. Independent review occurs after complete authoring; no per-edit builds.
Spec34 §6.2.1 defines the behavior and resource invariants before source edits.

Selected layout change: replace merged strings in sector0 with the pinned dec2flt
POWER_OF_FIVE_128 table, retaining the existing small table selections and assertions.
No saving credited before M1. Reverted the B-R2 custom trace formatter to the retained
M1 fmt-based runner/main/trace/lib; its protocol fields and hardware fixture remain.
The corrected single-record Console transaction and its native budget tests stay.
Removed custom-writer tests with the removed implementation; actual protocol parsing
and byte/field assertions remain in xtask and the conditional board phase.

## M1 and selected M2 work

M1 frozen manifest, 587 paths: 6b1f6cffdcea7609337f862c97078827c54b0df8928629a83d637690a5fd19e0.
The one preparation missed include!-based time_ops in cargo fmt; its first explicit
format completed before linking and the manifest was refreshed. Earlier manifest
bbea428d... remains as pre-fragment evidence. No repeated behavior check ran.

The canonical full-function firmware links: upper span 454,976 B, upper free 3,776 B,
sector 0 free 736 B, actual flash load 470,584 B. Text shrank 21,136 B and upper span 21,280 B
relative to B-R2 M2. The 16 KiB headroom gate still fails by 12,608 B. No physical action
has run. M1 uses safe ICF, stable 1.95, overflow checks and all runtime functions.

Measured selection for M2: compact the remaining deletion-heavy engine identity maps/
sets and lifecycle marks, preserving typed keys, ordering, allocation/work accounting
and terminal cleanup. Leave retainedGraph-only maps alone where the firmware map shows
no linked owner. Replace scalar integer-to-float promotion via i128 with the existing
full-width numeric helper; use checked-width integer arithmetic with a frozen old-wide
oracle (especially MIN MOD -1=0). Revert the ineffective validator inline experiment.
These are selected from the remaining 19,244 B BTree family and named numeric owners;
no additive savings or successful margin is assumed. M2 is the last authorized link.


M2 uses sorted live pairs for the five deletion-heavy engine metadata shapes, with
fallible geometric growth and explicit binary-search, relocation and shift charges.
Terminal cleanup retains each metadata collection once and completes even when the
work budget is exhausted. Instance reservation failures remove partially inserted
metadata; promotion reserves the destination owner list before changing lifetime.
Ordinary POU metadata reserves the admitted depth; outer instance-owner metadata
and live-activation IDs reserve the checked twice-depth bound, alongside execution
frame storage. Instance payloads retain charged bring-up growth. This does not claim allocation-free RUN.

M2 numeric review retains the old wide integer implementation as a test-only oracle,
covering signed/unsigned widths, mixed kinds, extrema, exponents and fault precedence.
Scalar integer-to-float construction uses the existing full-domain numeric conversion;
float-bit tests cover extrema and values around 2^53. The ineffective validator
inline attribute is removed. These native assertions remain unrun before the batch.

Builder preflight observed 103 GiB free on the Cargo target volume and a separate
Control Studio release compilation on the HMI target. B-R3 uses six Cargo jobs and
its leased target; timing-sensitive hardware work will recheck concurrent workloads.
The exact M1 source was archived before M2 synchronization.


## M2 and consolidated batch result

Final measured manifest: 590 records, SHA-256
`a27301e8e9b8d89f9db9b34329508a0274c93b44b04c56768a122b0f8703cf28`.
Canonical M2 link passed. Upper span 441,920 B leaves 16,832 B, which exceeds the
16 KiB floor by 448 B. Sector 0 has 736 B free. ELF inspection passed all partition,
alignment and RAM reservations. No function was removed and overflow checks stayed on.
M1-to-M2 upper reduction is 13,056 B; tree code shrank 12,738 B while replacement
engine code grew 2,836 B. The independent map review separates symbols from causal claims.

The one consolidated batch completed on the builder, 19:18:59–19:25:36 UTC.
Including preparation, its ledger has 38 required PASS rows, one required FAIL row,
and two advisory PASS rows. The failed integration command ran all 68 binaries:
536 assertions passed, two failed. Core all-feature 387, portable 348, i686 268,
hosted unit 3,841, native tooling 91, firmware library one and provenance helper four
all passed. All three Clippy gates, cross-target warnings, both lock audits, metadata,
architecture, diagram render/drift, format and artifact parity passed.

Failures retained without retries:

1. `physical_deadline_faults_mid_cycle_and_withholds_outputs`: expected exactly 32
   clock polls, observed 33. Terminal cleanup's charged work can sample the physical
   clock again after expiry. Preserve cleanup and latch expiry until the operation reset.
2. `two_thousand_distinct_input_bindings_fit_default_work_limit`: ExecutionTimeout.
   Journal insertion charged the whole length even for append, accumulating 1,999,000
   units for 2,000 destinations before other work. Charge real movement and bulk-sort
   input snapshots so arbitrary binding order cannot cause quadratic construction.

The original assertions remain. Corrections are being authored after the frozen batch;
none has been built, formatted, measured or tested. Both size measurements and the
single batch are consumed. Fresh validation, including a fresh firmware link, requires
owner authorization. The passing M2 ELF is retained and is not attributed to corrected
source. No hardware phase ran because the software prerequisite failed.

The 32-bit allocation profile peaked at 28,069 B across preparation/instantiation/scans
of the three artifacts; retirement returned live allocation to zero. This is hosted
32-bit evidence, not on-board heap/stack proof. The 72 KiB heap and 16 KiB MSP remain
reserved in the ELF. Generated diagram output was copied back after the batch.

Retained command logs, reviews, identities and counts: [evidence](b-r3-evidence/README.md).
Raw maps/ELFs/source archives remain outside git. No source reduction is credited with
independent savings without controlled evidence; no physical execution is claimed.


## Prepared correction boundary

The deadline correction adds an operation-scoped expiry latch to the existing shared
budget, cleared by the existing reset. Portable entry, work, native-call and unwind
checks reach the same latch; hosted Runtime sampling is unchanged. Root independently
reviewed the four-file correction and its new native reset/cleanup assertions. The
original hosted 32-poll assertion remains unchanged.

Bulk snapshot correction is authored and independently reviewed. It preserves the
original write and invalid-reference order while sorting/deduplicating physical keys
before snapshots. Native tests add reverse/mixed 2,000-binding artifacts, alias
restoration and first-invalid-reference coverage. Original failed assertions remain. No correction is
covered by the passing M2 ELF or old native results. Diagram source now describes
these corrections; its generated SVG/manifest are retained batch outputs and will need
regeneration in the next authorized preparation. No generator or check was rerun.

An independent reviewer reconciled all 113 retained M2 files with their raw counterparts,
all suite counts, the failed ledger row, the map/ELF arithmetic and allocation profile.
Evidence is intact; this reconciliation does not change the failed batch status.


## Additional authorization: run 2

After the failed batch, the owner explicitly requested “test and remeasure then run
the hardware tests”. Finish and independently review the deadline and journal fixes,
then one preparation/freeze and one additional consolidated batch. The existing suite
allocation is retained; a single canonical firmware link is inserted after native tests
and before ELF inspection, compared against the retained M2 map. Physical testing follows
only if all software/ELF/16 KiB headroom prerequisites pass. No automatic retries.

Run-2 scripts and evidence are separate from consumed M1/M2/first-batch paths. The
source freeze/review identity, ledgers and board guard are retained. No tests have yet
run on the corrections at the time of this authorization record.


## Run 2 result and prepared layout correction

Run 2 completed on the builder at 19:38:22 UTC. All native behavior suites passed:
392 all-feature core, 353 portable, 273 i686, 3,841 hosted unit, 539 hosted integration
across 68 binaries, 91 platform/tooling, one firmware-library and four provenance-helper
assertions. Original deadline and input-binding regressions plus new reverse/mixed
ordering cases passed. All lint, cross-target, audit, metadata, architecture, diagram,
format and artifact-parity checks passed. The ledger has 39 required PASS rows, one
required FAIL row and two advisory PASS rows, including preparation.

The corrected full-function firmware links. Upper span is 442,720 B, leaving 16,032 B;
this is 352 B below the unchanged 16 KiB margin, so ELF inspection rejects it. Sector 0
still has 736 B free. Hardware was not launched. Run-2 frozen identity (812 records):
`8b68c8dc3978de9049405e1ecf001d581558319cf739c0e8bfbde9483cb4d25d`.
The newly authorized batch and link are consumed; no automatic repeat.

A two-file linker correction is prepared after the batch: `memory.x` selects complete
immutable `libm::math::generic::sqrt::RSQRT_TAB` (256 B, alignment 2) and
`core::unicode::unicode_data::white_space::WHITESPACE_MAP` (256 B, alignment 1) into
sector 0 after existing 32-byte alignment. `profile.x` asserts the selected span is
exactly 512 B. Existing overlap, partition and RAM assertions remain. No Rust code,
function, test, overflow policy or toolchain changes. Predicted upper free space 16,544 B
and sector-0 free 224 B are **not measured**. A new link and inspector must confirm them.
The current local linker scripts therefore differ from the retained measured image.

Next action requires owner authorization: one focused link/inspection of this reviewed
layout correction, then the already specified board procedure only if it passes. Native
run-2 behavior evidence remains applicable to unchanged Rust sources. Do not repeat the
full software corpus without a new reason. Preserve the failed inspector and all maps.
No commits, push, flash or hardware run occurred.

The two-file placement correction has independent source review at
[b-r3-evidence/review-run2-packing/acceptance.md](b-r3-evidence/review-run2-packing/acceptance.md).
All frozen Rust/test files remain byte-identical to run 2. The retained evidence index
includes its README, reviews, both failed batch ledgers and the run-2 correction boundary.


## Additional authorization: focused run 3

Owner answered “yes” to one additional link/inspection followed by hardware tests if
it passes. Run 3 covers only the independently reviewed two-file placement correction;
all Rust/test sources remain identical to passing run 2. Reuse that software evidence,
freeze the final source identity, link once with the canonical configuration and run
the unchanged ELF inspector. Then execute the reviewed board procedure and trace
verification if the margin/partition checks pass. No automatic repeats or publication.


## Focused run 3 and hardware result

Focused run 3 completed at 19:43:25 UTC: canonical link and unchanged ELF inspector
pass. Source manifest 944 records, SHA-256
`49bf385446ebcea7bf50a440c113555b9d2c92197fd0d0c10c392b1a59f58c8e`.
Upper span 442,208 B leaves 16,544 B (160 B above the 16 KiB floor); sector 0 leaves
224 B. The measured relocation is exactly 512 B. ELF SHA-256
`87d622c71fa6f6dab01d13caef9db2671a174f9ccb575b0398d76389b8f61c5b`.
All Rust/tests remain identical to passing run 2. No functionality was removed.

The physical NUCLEO-F401RE procedure ran once: complete original flash backup,
explicit firmware-sector erase, write/readback verification, separate application
write, byte preservation of both firmware regions and the checkpoint sectors, then
one reset and 25-second UART capture. Hardware verifier failed with `insufficient
measured stack headroom`. The script's successful acquisition exit is not acceptance.
The board was halted for failure preservation; no reset/retest followed.

Main instantiation used 16,272 B of the 16,384 B reservation (112 B remaining versus
2,048 B required by the verifier); main-run saturated the painted region. Nesting
probes report depth 1: 9,680 B, depth 2: 16,344 B, depth 3: saturation at 16,384 B,
then a panic record. Saturated paint is only a lower bound. Halt MSP `0x20012130`
is 7,888 B below reserved floor `0x20014000`. The panic record's 64-character cap is
intentional; its implausible metric values cannot establish allocator use or an exact
panic cause. The stack reservation violation itself is established.

Independent evidence review confirms all 101 main logical rows match the oracle,
13 numeric records and GPIO injected transitions were emitted, and installation
preserved partitions. Depth 4, final stack peak, later high-output STOP/FAULT challenges,
gpio-dropped and intended watchdog reset/second identity/DONE were not reached.
Physical button actuation, optical LED observation and worst-case timing remain
unverified. The board attempt is failed, not partial acceptance.

Static ELF/source review identifies large native frames retained by nested VM entry
and call wrappers. Next work should address the shared engine's native call-stack
cost with a reviewed bounded design, preserving the admitted call depth and functions.
Do not lower the fixture depth, relax headroom, or infer a new safe RAM partition from
hosted heap estimates. This task has no authorization for another build or board run.
Raw dumps stay outside git; logs, hashes and independent reviews are retained in
[b-r3-evidence](b-r3-evidence/README.md). No commit, push or publication occurred.


Independent static diagnosis: the final dispatcher reserves 2,864 B per invocation;
the additional user-call path retains at least 4,440 B per nesting level before
transient initializer/helpers/IRQ frames. The firmware runner retains a 3,736 B frame
with fixture phases inlined. These are static prologue facts, not a complete stack
bound. The fixture is the finite Gpio→Outer→Middle→Leaf chain, not unbounded PLC
recursion. The existing logical VM FrameStack therefore does not by itself bound
native recursion. Explicit continuations in the shared dispatch loop, with preserved
parameter binding, output copyback, initializer lifecycle and error cleanup, are the
recommended next design; do not create a platform-specific execution fork. Full
ARM disassembly stays outside git, with selected prologues and identities retained.
