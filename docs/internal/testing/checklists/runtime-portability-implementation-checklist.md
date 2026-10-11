# Runtime portability implementation checklist

Behavior authority: [specification 34](../../../specs/34-runtime-portability.md),
[runtime engine](../../../specs/11-runtime-engine.md), and
[STBC format](../../../specs/12-bytecode.md). This file records execution and evidence;
it does not create additional product requirements. Existing closed extraction boards stay closed.

## Current checkpoint

A1 integration update (10 October): PR #130 merged through the guarded merge at
`258fe24b8c3706d566f5f05c25cfcd75e5b297c3` after all 26 checks passed. Native
Linux/macOS/Windows and VS Code passed first attempts with no retry rescues.
v0.24.71 release verification is complete. The integration agent owns that
release and subsequent A2 publication. #129/v0.24.70 has completed release
verification. None of this is A4 scope evidence.

| Field | Recorded state |
|---|---|
| Updated | 10 October 2026 — B-R4 run 8 software/ELF and physical F401 bring-up pass. Independent source, software and physical reconciliation complete; earlier failures retained. |
| Checkout | `/home/johannes/projects/trust-platform-portability-b`, branch `feat/runtime-portability-b`, base `df427259cc387a7a79fb81132be23e48ea1493d4`. |
| Bootstrap | Canonical AGENTS.md, CLAUDE.md and complete skills manually copied from `/home/johannes/projects/trust-platform`; 21 files byte-match locally and on the isolated builder checkout. |
| Authorization | Owner requested “commit then start next step”; A4 committed locally, Scope B implementation/review and one consolidated builder/hardware batch. Owner is away and explicitly instructed remote tests without requesting board markings. Owner subsequently explicitly requested implementation of all five B-R1 decisions: L2, shared footprint reductions, targeted containers/sorts, preserved checks, reviewed expiring advisory exception and both lock audits. B-R1 has implementation, independent review and one consolidated validation batch. Owner additionally authorized one linked-size measurement before the full batch, with no tests/hardware or automatic retry. Owner subsequently requested implementation of the recommended broader function-preserving set: compact diagnostics, targeted engine/retained-graph keys, lean registry and remaining sort/bookkeeping reductions, with safe ICF evaluation and bounded size-only measurements. Owner explicitly prohibits reduced function sets on any platform; the proposed restricted-profile fallback is withdrawn. Local Scope B commit authorized by the subsequent “commit” instruction; publication was subsequently authorized through the separate release-integration scope. The later explicit order “do as mny tests as required to fix it, dont stop” authorizes the recorded B-R4 corrections and retries without repeated permission requests. |
| Scope | B: NUCLEO-F401RE thin adapter and bare-metal composition, unchanged A4 main/numeric artifacts plus a separately authored GPIO/stack probe, real board traces and memory/timing evidence. |
| Status | Run 8 reviewed candidate passes ELF inspection and corrected validation2 (17 required software gates, one advisory); physical attempt 2 passes all 156 records and the Rust verifier. Upper flash free 18,720 B; peak heap 28,272 B / 73,728 B; peak MSP 13,596 B / 16,384 B, leaving 2,788 B. First metadata precheck and first installation failures remain retained. |
| Source identity | A4 base `df427259cc387a7a79fb81132be23e48ea1493d4`; run-8 frozen manifest `a95bcff98b5d4a51e70acf7e29cf0113b585ac69bcfc39c4cc5fbac6b1e8337c`; ELF `a0e6ed2d7f725fd17746d30c229d02e5b0125c00c494015f3be40322e293ec1e`; UART `9601cceef51d46170961c74e0afe2aba33b44250d3795225f6df60055a315704`. Runtime/firmware/tooling source is unchanged from the tested candidate; closeout documents/evidence and a separate 0.24.73 release-metadata commit follow the freeze. Historical hardware evidence retains its 0.24.71 tuple. |
| Limitations | Electronic board identity available; printed PCB/package markings, physical button actuation and optical LED observation unavailable. No claim of external-plant operation, durable retain, release qualification or worst-case timing proof. |
| Integration | A1/#130/v0.24.71 complete. A2/#131 merged at `3bed89b47b4e0ed8ecc11410b592228fff5f0112`; v0.24.72 release verification is complete (tag, Release workflow, assets/checksums and all five Marketplace targets). A3/A4/B publication is PR #132; merge/release remain pending. |
| Next action | Scope B remote bring-up and independent evidence reconciliation are complete. Retain this result for the next separately authorized scope. Scope B local checkpoint is `f756ddd9a`; the following metadata commit targets 0.24.73. The subsequently authorized publication scope validates the exact integrated SHA, replays its final firmware, and then handles push, guarded merge and release; later implementation scopes remain separate. Printed markings, manual button actuation and optical LED observation remain explicitly unverified; M3 allocation-free/worst-case qualification remains separate. |

Publication is authorized and isolated on `integrate/runtime-portability-stm32`.
Its reviewed source checkpoint `ca205132e` passed the exact-SHA guard and fresh
0.24.73 physical F401 replay: 101 samples/40 activations/zero misses, upper flash
free 18,720 B, heap peak 28,272 B and MSP free 2,788 B.
[Retained publication proof](../../../notes/runtime-portability/publication-evidence/README.md)
records the source/image identities and independent reconciliation. A docs/evidence
closeout `75b42a667` passed its successor exact-head guard and image identity check,
and was pushed as PR #132. Its complete first CI attempt found native Windows
remapping-fixture and macOS system-Bash audit-wrapper failures; Linux and area
checks passed. Automatic review found direct-I/O restart initialization overwritten
by image carry-over. All three corrections were implemented together and independently reviewed.
Corrected source `a8937a07c` passed the exact-SHA guard (8,690 native/519 VS Code),
all 20 required pinned supplement steps, strict documentation and fresh physical
F401 replay with the same verified memory margins and 101-sample oracle.
The new restart regression passed default/no-default/selected i686 execution.
Its retained docs/evidence successor requires its own clean exact-SHA guard and
image-identity check before correcting PR #132. Native Windows/macOS execution,
guarded merge and full release remain pending; prior checkpoints are preserved.
[Release integration command map](../../../notes/runtime-portability/stm32-release-integration.md)
owns the exact-SHA guard, fresh final-image hardware replay, push, guarded merge and release.
Earlier source checkpoints and hardware evidence remain unchanged.

Scope B execution and command map: [record](../../../notes/runtime-portability/b-execution.md).
B-R1 correction scope: [record](../../../notes/runtime-portability/b-r1-execution.md).
B-R2 result and remaining gates: [record](../../../notes/runtime-portability/b-r2-execution.md).

Run 3 correction evidence: [record](../../../notes/runtime-portability/b-r1-evidence/run-3/README.md).
Software assertions and lint/architecture pass. [Run 4](../../../notes/runtime-portability/b-r1-evidence/run-4-diagrams/README.md) closed diagrams at that checkpoint. Its flash/physical gaps are now closed by the
reviewed B-R4 run-8 successor, without changing the historical B-R1 failures.

### B-R1 — Shared footprint reductions, unchanged IEC functionality

- [x] RTP-BR1-01: L2 specification, linker, installer/ELF inspector, partition regressions and diagram agree; two 16 KiB checkpoint sectors remain unimplemented persistence.
- [x] RTP-BR1-02: Byte preparation avoids serialization while decoded-object checks remain; compact shared CRC matches accelerated CRC for fixtures, boundaries and deterministic randomized inputs.
- [x] RTP-BR1-03: Static default-library descriptors preserve every name, parameter shape and hosted custom/override behavior, with allocation/lookup equivalence assertions.
- [x] RTP-BR1-04: Record selected map/sort owners and preserved key order, tie/error order, lookup cost, capacity and work charging; implement targeted reductions and native regressions. Full M3 storage replacement stays deferred.
- [x] RTP-BR1-05: Overflow checks and complete admission stay enabled; reviewed RUSTSEC-2026-0110 exception expires within 90 days; root and firmware locks both undergo dependency audits.
- [x] RTP-BR1-06: Independent review, frozen source and native/core/32-bit/host, MCU, lint/cross-target, architecture/diagram and footprint evidence. Closed by the explicitly authorized B-R4 successor runs and final conditional board capture; original failed batches retained.

- [x] RTP-BR1-07: Expanded reductions preserve all IEC/assertion operations: remaining shared sorts, compact/shared registry metadata, checked engine/retained identities and reused initialization membership. Native order, identity, restart and retained-graph regressions.
- [x] RTP-BR1-08: Compact preparation diagnostics preserve stable fault identities and bounded useful context, with hosted rendering and native parity assertions; uniform function set required across every platform; no capability-reduction fallback.
- [x] RTP-BR1-09: At most two recorded size-only comparisons for the complete reviewed expanded implementation (ordinary and safe-ICF link); no unchecked-overflow variant, no tests/hardware in measurements. Final validation remains one batch with no automatic retry.

B-R1 software items above use runs 2–4. The checked measurement item records the two
authorized attempts, including the failed ordinary compilation and safe-ICF link; it
does not claim a successful link or isolated ICF savings in B-R1. The successor B-R4
run-8 evidence closes RTP-BR1-06 after the shared storage/stack corrections.

- [x] RTP-B-01: Reviewed thin adapter, separately installed application bundle, bounded firmware and native tooling regressions.
- [x] RTP-B-02: Run-8 linked ELF satisfies flash partitions/margin, static RAM, 72 KiB heap and 16 KiB MSP reservation. This does not establish physical stack usage; further candidates require fresh inspection.
- [x] RTP-B-03: Same A4 main/numeric artifacts replay on the physical F401; recorded GPIO pad state, safe-output transitions and independent watchdog reset.
- [x] RTP-B-04: Preparation/scan heap, call-depth 1–4 stack with IRQ activity, timing and numeric measurements satisfy the declared profile.
- [x] RTP-B-05: Independent evidence reconciliation and limitations retained; no substitution of simulated input for a physical button actuation.

### B-R4 — Shared native-stack correction

**Current authorization supersedes the earlier run limit:** “do as mny tests as required
to fix it, dont stop”. Continue reviewed root-cause fixes and recorded software/link/
hardware attempts until the required gates pass. No feature or acceptance reduction.

Current result: **run 8 software/ELF and physical bring-up pass; independently reconciled closeout complete**.
[Execution record](../../../notes/runtime-portability/b-r4-execution.md) and
[retained evidence](../../../notes/runtime-portability/b-r4-evidence/README.md).

Owner explicitly authorized “fix it and run the hardware tests again”. Historical correction work stayed on
feat/runtime-portability-b before its authorized local commit; no function/depth reduction, RAM repartition,
relaxed margins. The later repeat authorization above supersedes the original single-batch
limit; every failed attempt remains separately recorded, with review before correction runs.

- [x] RTP-BR4-01: Spec34 §6.2.2 records continuation, binding, cleanup, operand,
  budget, initializer and harness invariants before implementation.
- [x] RTP-BR4-02: Shared prepare/complete call semantics and explicit stack-dispatch
  continuations; preserve hosted optimized adapters and all lifecycle behavior.
- [x] RTP-BR4-03: Firmware phase boundaries and stop-on-insufficient-stack harness;
  unchanged fixtures, supported depth, images, heap/MSP and acceptance thresholds.
- [x] RTP-BR4-04: Native continuation/lifecycle/operand/depth/error assertions plus
  applicable existing corpus; independent full-diff review and source freeze.
- [x] RTP-BR4-05: Recorded builder batches and explicit source-parity reuse cover new firmware map/ELF,
  complete native/portable/i686/hosted gates, tooling, audits, diagrams and formatting.
- [x] RTP-BR4-06: Physical run-8 attempt 2 passes stack/heap/depth 1–4, numeric,
  process image, STOP/FAULT and watchdog evidence; prior failures retained.
- [x] RTP-BR4-07: Independent evidence reconciliation and checkpoint update.

### B-R3 — Compact live storage, unchanged runtime functions

Historical attempts below remain as run. Their unresolved physical criterion
RTP-BR3-07 is superseded by passing RTP-BR4-06; it is not a remaining Scope B blocker.

**Additional authorization after the failed batch:** owner requested “test and remeasure
then run the hardware tests”. Run 2 covers reviewed deadline/journal corrections, one
preparation/freeze, the deduplicated software gates and one corrected-image link, followed
by the existing physical procedure only if all prerequisites pass. No automatic retries.
The two earlier measurements and failed batch remain separately retained evidence.

Status: **software verified; flash fit/inspection passed in run 3; physical stack qualification failed**, owner “implement it”, 10 October 2026.
Continue on feat/runtime-portability-b at base df427259c. B-R2 failed measurements
and source identity remain historical evidence, not a passing baseline. Authorization:
complete implementation and independent review; at most two size measurements (M1/M2)
with their one-time source preparation; one consolidated validation batch, conditional
hardware only after complete software/ELF/headroom acceptance. No retries, commit/push,
capability cuts, unchecked arithmetic configuration or toolchain changes.

- [x] RTP-BR3-01: Write spec34 §6.2.1 invariants for IDs, frame/staging capacity,
  batch retirement, ordered journals, indexed name fallback, work/allocation and numerics.
- [x] RTP-BR3-02: Implement compact live frame/instance storage and portable indexed
  name lookup, preserving hosted interfaces; lifecycle, stale-ID, high-ID, miss and
  retirement native regressions. Preallocate frame bookkeeping from the admitted depth;
  do not claim all instance/compound allocations eliminated.
- [x] RTP-BR3-03: Replace selected engine journals/bookkeeping trees with compact
  ordered entries, preserve alias rollback and retained graphs, account movement/work.
- [x] RTP-BR3-04: Complete proved numeric/helper changes, one measured validator
  inlining experiment, exact error construction and sector-0 packing/trace decisions;
  preserve full domains and all functions. Include the five B-R2 Clippy expressions.
- [x] RTP-BR3-05: Independently review all source/test/tooling changes. Freeze complete
  source, then M1 once; M2 only for a reviewed measurement-informed revision. Each
  attempt is consumed even on failure; no extra trial/flag matrix.
- [x] RTP-BR3-06: One deduplicated final batch: B-R2 suites plus new lifecycle/numeric
  assertions, 32-bit allocation, MCU graphs, hosted vertical, tooling/Clippy/architecture,
  both locks, diagrams/drift, format/provenance. Collect all independent failures.
- [ ] RTP-BR3-07: Exact measured ELF leaves >=16 KiB upper headroom and respects
  sector0/app/checkpoints/RAM; then original hardware procedure including backup,
  explicit sector erases, UART/stack/heap/timing/safe outputs/IWDG. No shortened fixture.
- [x] RTP-BR3-08: Reconcile evidence; no board claim without execution. A failed final
  batch stops runs; corrections may be prepared but reruns need explicit authorization.

M1/M2 review and measurement are complete for their frozen identities. The measured
M2 image meets the flash floor; RTP-BR3-07 remains open for physical evidence.
Run 2 closes the corrected storage/journal behavior; run 3 closes final placement and
ELF inspection using the same Rust source. RTP-BR3-07 remains open: physical testing
failed stack headroom. Checked software rows do not claim hardware qualification. [B-R3 execution record](../../../notes/runtime-portability/b-r3-execution.md)
and [retained evidence](../../../notes/runtime-portability/b-r3-evidence/README.md)
identify the failed assertions and passing gates. Neither reruns nor hardware are launched.

### B-R2 — Fit the complete runtime on F401

Status: **implemented and reviewed; fit failed and validation remains incomplete**. The owner's “implement it” authorization covered
B-R2 implementation, independent reviews, at most two size-only measurements with
snapshot preparation, and one consolidated software/conditional hardware batch.
No automatic reruns, commits or push. Every PLC function remains required.

Active source: `trust-platform-portability-b`, branch `feat/runtime-portability-b`,
base `df427259c`. The isolated M1 snapshot is
`trust-platform-portability-br2-m1`, detached at the same base with the full B-R1
working source copied before B-R2 runtime edits. Both manually received matching
canonical AGENTS.md, CLAUDE.md and all skills from the primary checkout.
M1 contains layout/build/tooling changes only; M2 includes the complete selected
source wave. M1 completed with a capacity-only link failure: upper span 474,400 B,
15,648 B over L2; sector 0 has 1,248 B free. M2 also failed flash capacity
(17,504 B over). The final native suites passed; two Clippy gates failed. Prepared
corrections remain unvalidated, and hardware is unrun. Both measurement attempts
and the final batch are consumed.

Checked B-R2 items below mean their implementation/review/measurement or reconciliation
was performed, not that fit passed. RTP-BR2-08 remains open for the two failed Clippy
lanes; RTP-BR2-09/10 remain open for missing installable ELF and physical evidence.
The measured M2 source and post-batch lint corrections have separate identities.

#### Evidence and arithmetic

The retained safe-ICF map has SHA-256
`2b9f0af658b63c9902386c2d3a89db36144e56b5ffe657b18e998a5f0cc7d933`.
This B-R1 baseline predates the run-3 mechanical Rust corrections and the B-R2
measurements above. The planning arithmetic below is retained as historical input,
not the current fit result. Read-only inspection established:

| Quantity | Bytes | Meaning |
|---|---:|---|
| Upper firmware span / capacity | 493,536 / 458,752 | 34,784 B (33.97 KiB) over L2 |
| Vector table / sector 0 | 404 / 16,384 | At most 15,980 B (15.61 KiB) recoverable before extra alignment |
| Residual if the entire tail is usable | 18,804 | Still 18.36 KiB to remove just to link |
| Required additional reduction for a 16 KiB upper-region margin | 35,188 | 34.36 KiB beyond maximal tail use, not merely the residual-to-link |
| Merged string section | 14,306 | 11,210 B are path strings including terminators; physical classification below |
| Outlined function definitions | 1,543 totaling 18,742 B | Confirms outlining exists; does not measure net savings or prove optimization is exhausted |

Moving data into an existing firmware-owned sector changes placement, not total
occupied flash. Alignment, merging and path remapping interact, so savings must not
be added as independent guarantees. The review's 39–52 KiB range does not guarantee
the proposed 16 KiB margin: from the old layout that needs 51,168 B total benefit.
Estimates prioritize work; only the final map/ELF establish fit.

Follow-up inspection reads the saved ELF object, SHA-256
`997d7dee59569c20b34326177b4d2f3751e160b4a289b74ff2acc4c3d896b753`,
without a new link. The safe-ICF log records 92 removed text and 16 removed read-only
sections. Its address-significance table contains 47 entries and no function symbols:
the heap, IRQ counter, five dragon tables and parameter metadata. The review reports
no additional foldable twins. No incremental saving is credited to unrestricted ICF;
keep safe ICF and spend no measurement slot on `all`. This inventory is not an executed
safe-versus-all comparison or a general permission to fold address-significant data.

Splitting the physical `.rodata.str1.1` bytes on NUL terminators gives:

| String group | Count | Bytes including terminators |
|---|---:|---:|
| Worktree paths | 61 | 6,748 |
| Cargo registry paths | 19 | 1,942 |
| Local rustup source paths | 13 | 1,598 |
| `/rustc/<hash>/library/...` paths | 11 | 922 |
| Other names/messages | 93 | 3,096 |
| Total | 197 | 14,306 |

This reconciles exactly; the review's four byte subtotals sum to 14,335 and are not
used as a physical partition. Contrary to its wording, `/rustc/` strings are present.
Normalize the three local prefixes in Phase 1 and measure the result; about 5 KiB is
a working estimate, not guaranteed reclaimed upper-region capacity. Moving/shrinking
the same strings must not be counted twice. Existing precompiled string constants
are not automatically rewritten by changing this crate's source-path flags.

#### Decisions and disposition of the nine proposed levers

| Lever | Plan and limits |
|---|---|
| 1. Sector-0 immutable data | Include first. Select whole read-only input sections, preserve alignment/vectors and assert the 16 KiB end. Do not assume a wildcard automatically packs only what fits. Audit reset/vector symbols, data load addresses, load segments and installer erase ranges. |
| 2. Common BTree representations | Select the numeric owners from the retained map now and author alongside Phase 1. Normalize u32 newtype keys and existing ID-vector payloads without new side vectors. Keep typed wrappers at boundaries, the same tree operations and ordering. Treat packed Value-map keys, optional lifetimes and the three name caches separately; no blanket two-type replacement. |
| 3. Cold error constructors | Include measured shared failure constructors, preserving error identity, useful context and exact hosted rendering. Reuse existing conversion helpers, with cold/no-inline boundaries where appropriate. Do not force the ordinary success path through indirect dispatch. |
| 4. Smaller execution/preparation functions | Prioritize cold error/construction work outside the recursive dispatcher, then natural boundaries in other measured owners. This can remove cold-only slots from every recursive frame. Inspect actual frame layout and full nested hot/cold/IRQ peaks: compiler slot reuse, spills and call overhead prevent a source-only guarantee of the final high-water reduction. Keep one dispatcher, shared budgets and deadlines. |
| 5. Duplicate generic helpers | Include the charged sort error adapter, assertion comparator and evidenced duplicate helpers. A common internal sort failure marker must restore each caller's original error and exact charge/tie behavior. Retain typed public contracts; do not invent a general type-erasure framework. |
| 6. Explicit faults | Apply to touched, reachable malformed-data/capacity paths with a specified fault. Preserve fail-closed cleanup/output behavior and the firmware panic record. Inventory remaining panic/arithmetic/index sites; no claim of a panic-free crate from removing unwraps, and no blanket new lint suppression. A full core/alloc panic-elimination project remains outside this fit scope. Overflow checks stay on. |
| 7. Assertions and 128-bit arithmetic | Share assertion comparison control flow while retaining all names/messages and float formatting. Narrow numeric intermediates only with full-range equivalence, including LINT_MIN, ULINT_MAX, ties-to-even, nonfinite input, signed bit patterns, text radix/sign parsing and error precedence. Keep i128 where proof is absent. Time operators also use i128 division/conversion, so changing stdlib conversions alone cannot be credited with removing those helpers. |
| 8. Build flags | Put the already measured safe ICF choice, address-significance emission and existing linker/CPU settings into one default firmware configuration. Normalize source paths with the pinned compiler's supported remapping; preserve useful file/line context. No unrestricted ICF, optimization-level sweep, overflow-off variant or toolchain change in this scope. |
| 9. Firmware trace writer | Select from the retained map if useful; authoring need not await M1. Use shared integer/hex output helpers only for measured duplication. Preserve the B1 protocol byte-for-byte, error records and separation of timing from serial output. Keep full runtime float formatting; trace optimization is not a capability cut. |

Specific qualifications from source inspection:

- `memory.rs` contains three name-based caches and two other lookup maps, not five
  string-keyed caches. Strings cannot be replaced by a lossy numeric hash. Stable
  prepared IDs may be used only where already available or where their exact
  interning, unknown-name and invalidation behavior is designed and bounded first.
  Cache failures remain misses; scans must not acquire linear name-table work.
- Raw-ID normalization does not require side vectors or add lookup indirection.
  Specifically, `activation_pous` and `instance_templates` can share the existing
  `BTreeMap<u32,u32>` shape used by method owners and retained identity translation;
  `owned_instances` can share `BTreeMap<u32,Vec<u32>>` with retained marks/root indices;
  InstanceId sets can share `BTreeSet<u32>`. Preserve typed operations at the API edge.
  The RAM concern applies to widened/packed keys and any later auxiliary storage,
  not to unchanged u32 representations. Count B-tree node capacity/alignment in
  allocator peaks rather than bounding total RAM by four bytes per live entry alone.
- The Value-map merge needs an explicit key proof. `MemoryLocation` has five enum
  variants, including three I/O areas, and the journal key also contains a usize slot.
  Prove admitted bounds, supported/rejected locations, injectivity and original sort
  order before packing. Existing 65,536-record/local caps may support a checked compact
  encoding; they do not authorize truncating host offsets or changing error precedence.
  `Option<FrameId>` must preserve None versus Some(0) and the full admitted ID range.
- The measured BTree family is an optimization budget, not a minimum saving. An
  average-per-instantiation estimate does not establish a 12 KiB floor: instantiation
  sizes, LTO/inlining and existing folding differ. Existing retained-graph, snapshot,
  cycle and execution-frame suites are the primary regression corpus; rerun them on
  the new representation. Add only missing key-boundary/order/rollback assertions.
- Both safe and unrestricted ICF fold functions. The difference is address-significant
  sections, not whether function folding occurs. Use the saved-object inventory above
  as the practical reason not to spend a build on `all`; safe ICF is already in the
  retained map and adds no new credited saving.
- Path remapping does not rewrite already compiled core/alloc objects. It improves
  normalization, but a two-directory byte-identity build is required before claiming
  full reproducibility; that extra build is not in this two-attempt campaign.
- Assembly or alternate compiler-builtins would broaden numerical/startup evidence.
  Neither is required by this plan. Nightly, build-std, RUSTC_BOOTSTRAP and panic-record
  bypasses are excluded. The reported GCC/assembly saving is not independently verified.

Registry review: `parameters.rs` defines nonzero-sized named statics, and descriptors
borrow them. The `ptr::eq` assertions checking that sharing are valid and should stay;
Rust gives those statics allocation identity. Function-address equality is different:
the `fn_addr_eq` checks do not by themselves prove registry behavior. In B-R2 test
authoring, preserve registration coverage through representative native calls and
signature equality, replacing address-only behavior claims where redundant. Do not
remove the static-sharing/allocation regressions or claim an observed LTO test failure.
There is no identified production dispatch rule based on function address here.

The 128-bit helper caller set must include root `datetime.rs::days_from_civil`,
`time_ops::duration_to_ticks`, `time_ops::scale_duration`, and float/integer conversions.
Source contains wide division in all three time helpers; which operations emit a
particular compiler builtin must be traced in the saved object. Checked narrowing
must preserve extreme signed years, epoch arithmetic, MIN/-1, full ULINT factors,
rounding and existing faults. Do not credit the divider as removed until all linked
callers are accounted for and the next map confirms absence.

#### Phase 1 — Placement baseline; source authoring may proceed concurrently

- [x] RTP-BR2-01: Implement spec 34 §D.1's sector-0 extension in `memory.x`/
  `profile.x`, keeping `_stext=0x08010000`, all vector offsets, application sector 1
  and checkpoint sectors 2/3 intact. Add explicit read-only placement/end symbols
  and assertions. Keep the startup crate/link script pinned; do not fork startup.
- [x] RTP-BR2-02: Update `xtask/src/portability/elf.rs` and `map.rs` together.
  The current inspector accepts sector-0 loads, but needs named data bounds and
  read-only checks. The current map reporter rejects any non-vector load below
  `.text`; it must understand both firmware regions. Report occupied load bytes,
  upper span/free bytes, sector-0 use/free bytes and padding separately. Add native
  cases for valid discontiguous loads, vector overlap, sector-boundary overflow,
  a segment spanning protected sectors, writable data in sector 0, and `.data` LMA.
- [x] RTP-BR2-03: Freeze one ordinary firmware build configuration: stable 1.95.0,
  `opt-level="z"`, overflow checks, safe ICF with address-significance metadata,
  original CPU/link scripts and every function enabled. Integrate normalized path
  mappings without replacing required rustflags or inheriting host CC/CXX overrides.
  Update firmware README and diagram sources; retain the B-R1 candidate flags/map.
- [x] RTP-BR2-04: Review the complete phase, including authored tooling tests,
  linker scripts, flags and installer. Complete one mechanical formatting/provenance
  preparation for this snapshot and reconcile it with review. Then, only if explicitly authorized, perform
  size measurement **M1 once** on the builder. Retain errors, map, flags, identities
  and load-region report. No test or board run. A linker capacity miss with a complete
  map informs Phase 2; a compilation/tooling failure is retained and stops execution.

The first map determines actual tail occupancy and remaining reduction required for
16 KiB upper-region free space. It is not a prerequisite for selecting or authoring
already evidenced source improvements. Keep the M1 baseline free of those runtime
edits by using a frozen, bootstrapped measurement snapshot; do not overwrite concurrent
work or mix identities. A separate snapshot is a source copy, not an extra build.
No agent delegation is started by this plan. M1 does not authorize extra trials.
If M1 already satisfies the floor, skip unnecessary source reductions and M2 and use
that unchanged candidate for final acceptance.

#### Phase 2 — One selected source reduction set

- [x] RTP-BR2-05: Record a per-owner selection from the retained B-R1 map now:
  straightforward raw-ID maps/sets, shared assertion/sort/error helpers and cold
  dispatcher boundaries. Author them concurrently with placement/config/tooling
  work, preserving the M1 snapshot. Use M1 to adjust the remaining size budget,
  not to postpone independent authoring. Trace writing and proven-equivalent numeric
  narrowing follow the same selection. Keep rejected/retained rationale per lever;
  do not promise cumulative savings from overlapping symbols or an average floor.
- [x] RTP-BR2-06: Finish all selected implementation and native test authoring.
  Cover numeric limits and stable faults; ordering/cost exhaustion; instance/frame
  deletion, reuse protection, retained graphs and restart; output alias rollback;
  cache invalidation/misses; unchanged trace bytes and function/parameter registry.
  Preserve valid static-identity and zero-allocation tests; registry dispatch is
  demonstrated by native behavior, not merely function-pointer address equality.
  No per-edit tests, links or lint runs. Existing asserts/oracles are not weakened.
- [x] RTP-BR2-07: Obtain independent review of all changed source, tests, profile,
  tooling and reachable callers, including included Rust fragments. Complete the
  single formatting/provenance preparation and freeze before the final candidate.
  Only if explicitly authorized, perform size measurement **M2 once** of that exact
  default configuration. It is the last measurement attempt, not a flag matrix.
  Record the source/locks/toolchain/ELF/map hashes and map deltas against M1/B-R1.

Two measurement attempts are the explicitly authorized exception to the implementation-first
validation cadence, including each snapshot's one formatting/provenance preparation,
not recurring tests/lint or authorization granted by this document. A failed compile/link
counts as an attempt. There is no automatic retry, ICF-all fallback, opt-level variant
or third link hidden in acceptance. Freeze measured Rust/config bytes for validation;
if later corrections change them, the measurement no longer certifies that candidate.

#### Phase 3 — One consolidated software and conditional hardware batch

The unchecked B-R2 candidate-specific gates below remain historical failures.
Their software/ELF/physical requirements are satisfied by successor B-R3/B-R4
evidence; no claim is made that the failed B-R2 M2 image became qualified.

- [ ] RTP-BR2-08: Before execution, record one deduplicated command ledger using
  the B-R1 run-2 suite allocation plus new native assertions. Include core all-feature
  and portable suites, i686 allocation/behavior evidence, F401/C6 checks, hosted unit
  and affected integration/stdlib/restart suites, the required four runtime-vertical
  binaries, xtask/platform/firmware tooling, Clippy including portable/firmware lanes,
  cross-target warnings, both lock audits, architecture then diagrams/drift, and
  format/diff/provenance checks. No duplicate test-all or release guard in this scope.
- [ ] RTP-BR2-09: Inspect the exact M2 ELF (or unchanged M1 if Phase 2 was skipped).
  Require at least 16,384 B free in the upper region, sector-0 end <=0x08004000,
  safe vector/reset addresses, valid load segments and unchanged 72 KiB heap/16 KiB
  MSP reservations. Reuse its hashes and bytes; do not relink to obtain a passing map.
  A linked image below the headroom floor remains an unqualified candidate.
- [ ] RTP-BR2-10: If link/partition/headroom and required software prerequisites
  pass, use Scope B's existing device procedure in this same authorized batch:
  back up flash, program/verify the firmware sectors and separate application,
  preserve checkpoint sectors, replay the unchanged three artifacts, compare traces,
  check output-off states and IWDG reset, and measure live heap/stack/IRQ/timing.
  Confirm app-only installation preserves firmware-owned sector-0 data. No mass
  erase, option-byte change, fixture reduction or manual board-marking dependency.
- [x] RTP-BR2-11: Reconcile independent review, source and generated artifact identities;
  retain all attempted/failed/unrun steps. Close B hardware items only on real device
  evidence. If fit or hardware fails, collect independent software results in the
  authorized batch, leave dependent steps unrun, and stop without another build.

The 27.4 KiB i686 peak is a useful prior estimate only. Re-measure after container
changes; it excludes ARM layout, allocator metadata/fragmentation and MSP/IRQ stack.
Require actual heap fit and spec D.1's observed 2 KiB stack/IRQ margin, preserve fault
recording, and keep timing measurements separate from serial transmission. A smaller
function or a successful flash link does not establish a stack or timing bound.

No function is removed by this plan. If the two attempts cannot meet the full-function
acceptance floor, report the remaining measured obstacle and prepare a new scope;
do not silently reduce capabilities, hardware acceptance criteria or the toolchain pin.

Primary references checked during planning: [ST RM0368 §3.3/Table 5](https://www.st.com/resource/en/reference_manual/rm0368-stm32f401xbc-and-stm32f401xde-advanced-armbased-32bit-mcus-stmicroelectronics.pdf),
[cortex-m-rt 0.7.5 linker source](https://docs.rs/crate/cortex-m-rt/0.7.5/source/link.x.in),
[LLVM address significance](https://llvm.org/docs/Extensions.html#sht-llvm-addrsig-section-address-significance-table),
[LLD 22.1.2 ICF](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-22.1.2/lld/ELF/ICF.cpp),
[Rust function-pointer identity](https://doc.rust-lang.org/std/ptr/fn.fn_addr_eq.html),
[Rust static allocation identity](https://doc.rust-lang.org/reference/items/static-items.html),
[LLVM 22.1.2 stack-slot reuse](https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-22.1.2/llvm/lib/CodeGen/StackColoring.cpp),
[Rust 1.95 remapping flag](https://raw.githubusercontent.com/rust-lang/rust/1.95.0/src/doc/rustc/src/command-line-arguments.md).


A1 historical evidence remains in `docs/notes/runtime-portability/a1-execution.md`.
A2 evidence and full failure history: [execution record](../../../notes/runtime-portability/a2-execution.md).
Current integration decisions and deduplicated command map: [release integration](../../../notes/runtime-portability/a2-release-integration.md).

Historical pre-run-4 correction review: 116 source/config/spec/diagram paths, SHA-256
`30ec65ad4ae1827fb9cab98e561e737cf711260fe2ff24bbfd6765d482f98008`
(sorted path/NUL/content-or-`<deleted>`/NUL; excludes evolving execution notes/checklist).
That historical review does not certify the current post-run-4 corrections.
Prepared batch command/script is recorded in the execution note. Checkboxes stay open until
reviewed implementation and required evidence agree.

## Expanded A2 review correction scope

The owner explicitly requested all findings and recommendations implemented, including the larger
refactors. This supersedes the initial suggestion to defer them. The expanded corrections are now verified through A2-R6/R7/R8;
run-3 evidence remains historical for the original relocation.

- [x] RTP-A2-R01: Commit-safe evidence, complete manifests, accurate status/format/target claims.
- [x] RTP-A2-R02: Real bytecode modules and documented public API; hosted owned conversions,
  public borrowed view and slice-bound lifetime; canonical shared pure test corpus and common helpers.
- [x] RTP-A2-R03: One decoded instruction representation/context, structured rejections, remove
  no-op/dead passes, preserve semantic checks and error precedence where required.
- [x] RTP-A2-R04: Bound validator analysis storage/work; branch-entry stack states and native
  adversarial-budget assertions; checked serializer/jump arithmetic and header/table exclusion.
- [x] RTP-A2-R05: Validated borrowed marker required at host VM materialization, while later
  profile admission/executable preparation remain A4; native positive and rejection tests.
- [x] RTP-A2-R07: Document and regress LOAD_NULL instance ownership; index tables once, bound
  lookup work, compact decoded instructions, share stack effects, remove validator panic paths,
  and admit the maximum-instruction/reference scale fixture within default limits.
- [x] RTP-A2-R06: Independent review of entire correction, then explicitly authorized consolidated
  batch including 32-bit native execution, both MCU checks, host corpus/runtime vertical,
  lint/format (including remaining touched fragments), architecture/diagrams and provenance checks.

No new validation batch or commit/push is authorized by this scope record.

## How to use this checklist

1. Work on one authorized scope at a time. The four-scope selection changes the old Scope A
   boundary; it does not grant extra runs inside a scope. A1–A4 together close M2A-H.
2. Read the relevant specification and native assertions. Settle ambiguous behavior there before
   coding. Identify the exact observable result, fault/output policy and evidence needed.
3. Implement the complete scope and author its tests. Keep the existing APIs, assertions and
   baseline artifacts. Update the checkpoint and requirement rows as meaningful work finishes.
4. Obtain an independent code/spec review before freezing the implementation. Record reviewer,
   reviewed diff identity, findings and dispositions. A second reading by the author is not an
   independent review. Review need not run builds; no delegation is started by this document.
5. Freeze the implementation and command list, then execute one deduplicated final validation
   batch. Required builds, lint, tests, diagrams and applicable UI/hardware evidence belong there.
   Missing baseline runs use a preserved original snapshot in this same planned batch.
6. Record every failure and unrun check. A failed batch leaves the scope unverified. Fixes may be
   prepared within scope, but another run needs explicit user authorization under AGENTS.md.
7. Close a scope only when its implementation, native evidence and review are complete for the
   same source/build identity. Continue already-authorized work; a status question does not stop it.
   Starting another scope follows the authorization already recorded, or a new request if needed.

Use `planned`, `implementing`, `ready for batch`, `failed`, `awaiting evidence`, and `verified`.
A checked task means accepted evidence and resolved review findings, not merely code written.
Keep separate evidence for host tests, cross-target checking, firmware linking and physical boards.
No document, tool, reviewer or test suite guarantees zero bugs. The release target is zero known
correctness/safety defects in the claimed profile and no missing mandatory acceptance evidence;
this work does not establish functional-safety certification.

## Ordered scopes and acceptance work

All tasks start open. The owning specification supplies the complete behavior contract; the
items below organize implementation without replacing that contract.

### A1 — Portable foundations

- [x] RTP-A1-01: Record compiler/features, numeric compatibility expectations and target
  layout/resource limits needed now, with original source/artifact provenance for attribution.
  Freeze the accepted scheduler baseline after the SCHED-05 fix and updated assertions in A1;
  subsequent scopes inherit the corrected contract. Preserve later hardware measurements as
  pending, not guessed values.
- [x] RTP-A1-05: Implement SCHED-05 first as a separately reviewed change, sharing A1's final batch.
  Apply the reviewed readiness/core/case/host test draft from
  `/home/johannes/projects/trust-platform-task-schedule`, branch `fix/task-interval-schedule`,
  base `be8d81a4a7ab16ca7554b8be0f4723161ec1a47b`, excluding stale AGENTS.md and the unsupported
  changelog claim. Update the 35/55 ms sample baseline assertions to 30/50 ms; retain
  backward-time/jump/saturation coverage. Correct
  `periodic_task_keeps_its_interval_when_cycles_start_late` comments to describe injected logical
  time. Add the core 25 ms / 10 ms case: due 25 at sample 30, `last_run=25`, then due 50 at sample
  50 with zero missed intervals. Add the host `tasks.rs` case using `set_current_time` and
  `execute_cycle` at 10 ms samples through 1000 ms: 40 activations, zero overruns. Update the
  CHANGELOG entry when implemented, without the 75-runs claim. No intermediate validation run.
  Non-blocking maintenance alongside these tests adds `verification/spec-gaps.toml` and the new
  mappings in `verification/runtime-anomaly-taxonomy.toml` beside
  `ANOM_MAP_WATCHDOG_REVIEW_C5ABDD0A`, using `scripts/verification` generators for IDs. Close test
  evidence only after the native assertions pass. No metadata/generator work is started by this
  planning amendment; control-course configurations remain outside scope.
- [x] RTP-A1-02: Fix workspace/default feature inheritance and host opt-ins; implement the explicit
  no_std ordered-map hasher and constructors while preserving insertion order and host APIs.
  Add isolated F401/C6 core CI lanes and target `Value` layout assertions.
- [x] RTP-A1-03: Add the locked libm direct edge with the selected features; route core pow/trunc and
  hosted numeric functions through the agreed primitives. Preserve conversions/faults, retain old
  oracles and document the intentional numeric change. Author boundary and threshold assertions.
- [x] RTP-A1-04: Complete review and A1's one batch after all implementation/test authoring.
  Include `cargo +1.95.0 test --locked -p trust-runtime-core` and
  `cargo +1.95.0 test --locked -p trust-runtime --test tasks --test scheduler_resource` plus the
  runtime vertical and remaining A1 gates, deduplicated. Record corrected scheduler, numeric and
  target-graph evidence. A1 proves these foundations; compiler-free integration and boards remain
  later gates. Version/release actions follow the scope's normal rules.

### A2 — Shared loader and validator

These items closed after reviewed corrections and A2-R6/R7/R8. Runs 1–3 certify only the historical relocation; failed runs 4–7 are retained, with subsequent passing evidence explicitly identified.

- [x] RTP-A2-01: Relocate the complete container, decoder, validator and metadata materialization
  boundary into core with portable collections and the required locked CRC edge. Decide byte
  serialization ownership without moving compiler/HIR lowering into core; preserve host re-exports.
- [x] RTP-A2-02: Move the native assertions with their code; preserve 1.x decode/error behavior,
  bounds, CRC and malformed-section cases. Author missing native rejection assertions where the
  owning contract requires them; raw or partly validated data must not become executable.
- [x] RTP-A2-03: Complete review and A2's batch against the relocated corpus and both target graphs.
  Saved 1.x parsing is not proof of source-free state construction or 2.0 execution.

### A3 — STBC 2.0 format and producer

- [x] RTP-A3-01: Settle exact section/layout/validation rules in spec 12 before implementation:
  executable initializer bodies, defaults/root construction, ownership/lifetime and version rules.
  Record the P/Q compatibility-window policy; name release versions before P ships.
- [x] RTP-A3-02: Implement 2.0 producer/lowering, loader/validator/disassembler/format handling and
  explicit version selection together. Retain the legacy fixture and add a separately named 2.0
  fixture. Keep current hosted execution usable; candidate 2.0 emission is opt-in until A4.
- [x] RTP-A3-03: Complete review and A3's batch: legacy regressions, valid 2.0 round trips,
  malformed initialization/construction/version cases, and source-free rejection of 1.x.
  No claim that a serialized initializer has been executed until A4.

### A4 — Shared-engine integration

- [x] RTP-A4-01: Extract the real dispatcher/state/context and timers needed by the common fixture;
  construct roots/defaults from TYPE_TABLE and execute initialization with that same dispatcher.
  Remove HIR/Expr/harness initialization and host thread-local/time assumptions from this path.
- [x] RTP-A4-02: Execute saved STBC in a fresh headless consumer: Boolean/integer logic, TON, bounded
  array, FB through an interface, nonzero defaults, changing-input local initialization and the
  numeric/control fixture. Include SCHED-05's 25 ms periodic counter task with exact 10 ms logical
  samples from a 0 ms registration baseline: activations 30/50/80/100 ms, 40 through 1000 ms,
  zero overruns; save due times, samples, state and missed counts for later COMPAT-06 replay on every host and
  both boards in B/E and hosted qualification. Preserve native call-depth/budget/fault behavior and host/tier APIs; record remaining
  bring-up allocations and profile limits.
- [x] RTP-A4-03: Complete review and A4's integration batch, including compiler-free loading,
  initialization/state/fault oracles, inherited scheduler assertions and affected host/format
  regressions. Require A1's completed SCHED-05 correction and native evidence before A4; verify
  its compiler-free integration and freeze the saved artifact/trace for B/E. Close aggregate A only
  now; M2A still requires the physical F401 result.

### Program continuation — B complete, later scopes open

| Scope | Dependency and complete deliverable | Acceptance evidence |
|---|---|---|
| B — F401 bring-up (M2A-B), complete | A4; same engine/artifact on NUCLEO-F401RE, frozen 1.95 tuple, timer, process images, UART, IWDG and instrumented bring-up heap. | Physical trace agreement; reset/fault outputs; linked sections, preparation/RUN/heap peaks, native stack paint/MSP/IRQ headroom, numeric timing. B plus A closes M2A. |
| E — C6 bring-up (M2E) | A4 and an available C6 board; reuse the common corpus promptly to expose second-architecture assumptions. | Actual board execution, I/O/time/watchdog, software-float cost and memory evidence. No board means this gate stays open. |
| U — Modernization (M0U) | Default after A+B, before H; E need not block it. Refresh all-project dependency candidates and implement justified upgrades/retentions with Rust/edition/resolver/MSRV/Node changes. | MOD-01 ledger and MOD-06 batch, old/new numeric and performance comparisons, affected WASM/extension/protocol/native/MCU evidence. Requalify changed build tuples. |
| H — Full hosted adoption (M2B) | A+B, normally U; finish shared execution/library coverage, internal register tier and runtime-only/full/development compositions. | Saved-artifact and existing configuration/API/debug/HMI/protocol/restart/update workflows; native Linux/macOS/Windows evidence. |
| C — Bounded operation (M3) | Early hardware findings plus relevant H work; compound storage/atomic retirement, import limits, fixed fault records, bounded persistence and real generation lifetimes/admission/deployment. | Allocation instrumentation including first-use/error/destruction; capacity rejection; lifecycle/update/readers and persistence fault injection. Preserve independent PLC value copies. |
| QH — Hosted qualification (M4H) | H+C+U for each declared build. | Spec 34 §14 on each claimed native OS/architecture, bounded operation and preserved workflows, timing/load/performance records. |
| QF — F401 qualification (M4) | B+C+U for the final build. | §14 on the actual F401: completion-level timing, memory/stack, power-cut retain/install recovery, real faults/watchdog/I/O and declared soak/disturbances. |
| QE — ESP32 qualification (M5) | E+C+U for the final build. | Same applicable §14 contract on the named C6 board, independently of F401 results. |

Define each later scope's exact implementation and single batch before starting it; splitting or
combining an authorized scope requires an explicit change. QH/QF may proceed independently when
their prerequisites hold. Unavailable ESP32 hardware does not stop independent hosted/F401 work.
Completion of A1, A, B or U alone is not completion of the full program. Commit/push/merge/release
actions remain separately requested under AGENTS.md; qualification is not publication.

## Validation allocation

Before a scope's freeze, record the exact commands and assertions, features, target triples,
environment and expected nonzero test counts in its batch record. Use `--locked` and the selected
toolchain. All cargo/just/npm work runs on trust-builder with its target lease, disk preflight,
isolated source copy and canonical agent-file bootstrap. Native macOS/Windows and physical-board
evidence run on their actual platforms; a Linux builder cannot substitute for them.

| Batch | Required native/build coverage |
|---|---|
| A1 | Separate `thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf` core checks/feature graphs; core tests including corrected readiness/case assertions and target layout assertions; `tasks` (draft late-sample and new 25 ms / 10 ms case), `scheduler_resource`; numeric/control/fault assertions; `runtime_core_behavior_lock`, `bytecode_vm_core`, `bytecode_vm_differential`; required runtime vertical. SCHED-05 shares this one final batch. |
| A2 | Both isolated core targets; core/moved unit tests; `bytecode_container`, `bytecode_metadata`, `bytecode_decode_resource_bounds`, `bytecode_sections`, `process_image`, `bytecode_validation`, `bytecode_optional_sections`; affected round-trip/VM behavior and runtime vertical. |
| A3 | Both isolated core targets; core/format/version tests; A2 format corpus as affected plus `bytecode_encoder`, `bytecode_roundtrip`, `bytecode_verification_cases`; affected compiler/disassembler assertions and runtime vertical. |
| A4 | Both isolated core targets, core tests, `runtime_core_compiler_free_load` (new, including the 25 ms / 10 ms trace), complete affected format corpus, `runtime_core_behavior_lock`, `bytecode_vm_core`, `bytecode_vm_differential`, `runtime_restart`, inherited `tasks` / `scheduler_resource` regressions, `vars_retain`, numeric/initializer cases and runtime vertical. |
| Every changed runtime boundary | Runtime vertical = `api_smoke`, `debug_control`, `complete_program`, `runtime_reliability`; add `simulation_workflow` if simulation changes. Include architecture-doctor `--full-map`, affected diagrams/render/drift, formatting and lint. Deduplicate suites included by a required full gate. |
| B/E | Firmware link and map, controlled boot and physical fixture traces, real output/watchdog behavior, preparation/heap/native stack and numeric measurements; retain raw data and firmware/artifact identities. |
| U/H/C/QH/QF/QE | The owning milestone plus all applicable spec §14 acceptance. Add risk-relevant property/fuzz/mutation/concurrency checks in the owning batch; assertions, budgets/seeds and raw results are required, not merely a tool name. |

CI/release rules still apply when requested: `just fmt`, `just clippy`, `just test-all`, applicable
area/hardware evidence and exact-SHA guards before a push. Integrate these into the authorized
batch when that delivery is in scope; do not silently launch another run afterwards.
If a prerequisite fails, record dependent commands as unrun; collect independent failures in the
planned batch where safe. Do not restart failed commands automatically or count zero selected tests
as acceptance. Code changed after validation requires another explicitly authorized batch before
claiming that new code verified.

## Requirement-to-evidence ledger

There is one row for each requirement ID defined in spec 34. The route names implementation and
qualification scopes, not a claim that each requirement needs a separate new test. Read the native
assertions before adding work. At the relevant scope, replace `pending` with exact code/test or
review locations, the batch result and artifact identity. Record partial scope evidence without
closing a cross-platform requirement prematurely. Unavailable evidence remains open; an exclusion
needs the owning spec's profile rule, not a convenient skip. Mapping completeness is not correctness
proof and this bookkeeping cannot invent product work or block it by itself.

| Requirement | Implementation / qualification route | Evidence and state |
|---|---|---|
| ARCH-01 | A1–A4, B/E/H/C; QH/QF/QE | A4 native evidence plus B-R4 run 8: unchanged saved artifacts execute on F401 with exact state/scheduling and labeled numeric tolerances. Other hosts/boards and production qualification remain E/H/Q. |
| ARCH-02 | A1–A4, B/E/H/C; QH/QF/QE | A4 compiler-free core is linked and physically executed by the isolated F401 composition in B-R4 run 8. ESP32 composition/hardware remains E. |
| ARCH-03 | A1–A4, B/E/H/C; QH/QF/QE | Core remains forbid(unsafe_code); F401 BSP unsafe registrations/review and architecture pass through B-R4 run 8. Other BSP qualification remains E/Q. |
| ARCH-04 | A1–A4, B/E/H/C; QH/QF/QE | A4 shared execution is used by the physical F401 clock/I/O/watchdog adapter. Further hosted composition and platform qualification remain H/E/Q. |
| ARCH-05 | C; QH/QF/QE | pending — open |
| ARCH-06 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): private prepared/state constructors enforce native import/state/clock capabilities and logical resource limits; unsupported raw Retain/Io and multiple-resource profiles reject explicitly. Timing and deployment admission remain C/qualification. |
| ARCH-07 | B/E; QF/QE | B partial: reviewed F401 thin adapter and its real GPIO/clock/watchdog evidence pass B-R4 run 8. Other platform and production obligations remain open. |
| ARCH-08 | A1/A4/H; QH | A4 partial (A4-H): selected existing hosted API, debug, restart, standard-library and register-tier regressions remain supported. Full hosted compositions, services, performance and native-OS qualification remain H/QH. |
| ARCH-09 | H; QH | pending — open |
| ARCH-10 | B/E; QF/QE | B partial: F401 uses the declared stable toolchain/full-function composition. C6 library checks pass; C6 hardware and final per-board qualification remain E/Q. |
| MODEL-01 | A2/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/A): serialized bytes, immutable PreparedModule and exclusively owned RuntimeState have separate ownership; saved-artifact construction is compiler-free. Installed-generation replacement remains H/C. |
| MODEL-02 | A2/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R): artifact initializer/default bodies and native bindings are prepared before execution; first and repeated calls use the shared dispatcher. Allocation-free RUN, complete hosted plans/caches and bounded first-use qualification remain H/C. |
| MODEL-03 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| MODEL-04 | A2/A3/A4/H/C; QH/QF/QE | pending — open |
| BUILD-01 | A1/A2/A4/H/U; QH/QF/QE | A4 no-HIR consumer and B-R4 run-8 full F401 link/boot/replay pass. C6 firmware and other product graphs remain E/H/U. |
| BUILD-02 | A1/A2/A4/H/U; QH/QF/QE | B-R4 uses isolated locked firmware workspace and recorded full-feature no_std graph; F401 image executes. Broader feature matrix and modernization remain E/H/U. |
| BUILD-03 | A1/A2; U on modernization | A1 feature/map split preserved; locked portable CRC edge qualified in A2-R6. Toolchain/edition modernization remains U. |
| BUILD-04 | A1/A2/A4/H/U; QH/QF/QE | F401 canonical firmware link, section inspection, boot and physical execution pass B-R4 run 8. C6 remains library-check evidence, not physical proof. |
| BUILD-05 | A1/A2/A4/H/U; QH/QF/QE | Same shared Rust engine runs on F401; no platform semantic fork or reduced function set. Stable safe-ICF build remains measured; other target qualification open. |
| BUILD-06 | A1/A2/A4/H/U; QH/QF/QE | A4 partial (A4-H): compiler, services, host clocks/debugger/profiling and register-tier selection remain hosted adapters around shared execution. Full product composition work remains H. |
| BUILD-07 | A4/H; QH | A4 partial (A4-E/B): the standalone core consumer loads and executes without compiler/HIR. The hosted runtime-only product composition and its isolated feature graph remain H; this requirement is not closed. |
| BUILD-08 | H/U; QH | pending — open |
| BUILD-09 | A1/A2/A3/A4; C for optional no-CAS | A4 partial (A4-B): independent F401/C6 no-default engine checks and graphs pass. Arc/SmolStr bring-up ownership remains; atomic-retirement, no-CAS/C3 and bounded-compound qualification remain C. |
| HOST-01 | A4/H/C; QH | A4 partial (A4-H/E): the actual hosted stack dispatcher, call/reference operations and native semantics are shared with the source-free engine. Full hosted cycle/service adoption and all native OS qualification remain H/QH. |
| HOST-02 | A4/H/C; QH | A4 application-selectable logical limits retained; B-R4 run 8 measures the selected F401 fixture fit. This does not establish hosted capacity or arbitrary F401 program admission. |
| HOST-03 | A4/H/C; QH | A4 partial (A4-H): required runtime vertical, debug, restart, I/O, bytecode and selected compatibility APIs retain native regressions. Complete HMI/protocol/online-change and platform qualification remain H/C/QH. |
| HOST-04 | A4/H/C; QH | A4 partial (A4-H): existing register-tier APIs and stack/tier differential assertions are retained through the host adapter. Compiler-free optimized-plan preparation and bounded cache behavior remain H/C. |
| HOST-05 | A4/H/C; QH | pending — open |
| HOST-06 | A4/H/C; QH | pending — open |
| HOST-07 | H; QH | pending — open |
| HOST-08 | H; QH | pending — open |
| SCHED-01 | A1/A4/H/C; QH/QF/QE | A4 cooperative readiness/order regressions retained; F401 run-8 oracle verifies nominal deadlines and 40 activations with zero misses. Complete host/C6 scheduling-profile qualification remains H/E/Q. |
| SCHED-02 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-03 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-04 | A4/H/C; QH/QF/QE | pending — open |
| SCHED-05 | A1 fix/native evidence; A4 fixture; B/E/H/QH/QF/QE parity | A1 nominal-deadline correction integrates through A4 saved fixture; F401 run 8 confirms 25 ms on 10 ms samples, 40 activations through 1000 ms and zero misses. Other host/C6 replay remains E/H/Q. |
| LOAD-01 | A2/A3/A4/C; QH/QF/QE | Fresh native and F401 run-8 consumers construct/execute saved STBC 2.0 without HIR or source. Other profiles and production admission remain E/H/C/Q. |
| LOAD-02 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-E/A): TYPE_TABLE, construction roots, task/I/O metadata and executable initializers supply fresh state; missing or inconsistent executable metadata fails admission. Complete later-profile coverage remains H/C. |
| LOAD-03 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A): full validation is followed by native import/signature/hidden-state and clock-service checks plus resource/domain restrictions before RUN. Timing/deployment profile admission remains C. |
| LOAD-04 | A2/A3/A4/C; QH/QF/QE | A4 bounded decoder/preparation tests retained; F401 run-8 peak preparation heap is 28,272 B with zero failures. Arbitrary-profile allocator/stack bounds remain C/E/Q. |
| LOAD-05 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-R/H): dynamic indexes, reference identity/lifetime, typed assignments, constants, nested call/depth/fuel/deadline and arithmetic faults remain enforced. Broader profile/platform qualification remains open. |
| LOAD-06 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): executable imports resolve to shared implementations; native FB state/signatures and CURRENT_DT capability are checked. Physical/import blocking, cancellation and full bounded-callback contracts remain C. |
| LOAD-07 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): raw/validated containers are distinct from private PreparedModule admission and RuntimeState construction; public state exposes typed writes, not mutable adapter-trait storage access. Later deployment admission remains C. |
| LOAD-08 | A2/A3/A4/C; QH/QF/QE | pending — open |
| LOAD-09 | A2/A3/A4/C; QH/QF/QE | A4 partial (A4-A/R): bounded construction/preparation and failed restart preserve prior state; native output groups preserve caller destinations on conversion/write/budget failure. Online generation replacement and bounded overlap remain H/C. |
| LOAD-10 | A3/A4; B/E/QH/QF/QE | A3 artifacts construct fresh F401 roots/defaults/task/I/O state in run 8 through the shared dispatcher. Broader hosted/platform construction qualification remains E/H/C. |
| LOAD-11 | A3/A4; B/E/QH/QF/QE | A4 partial (A4-E/R): default/alias/member/local/static initializer bodies execute through the ordinary dispatcher with shared budgets and activation lifetimes. Legacy 1.x retains its hosted Expr adapter; full hosted retirement remains H. |
| MEM-01 | A1/A4/B/E/C; QH/QF/QE | F401 run-8 region inspection and physical peaks pass: heap 28,272/73,728 B, MSP 13,596/16,384 B, Value 32 B/alignment 8. Broader profiles, service/retain staging and production bounds remain C/E/Q. |
| MEM-02 | A1/A4/B/E/C; QH/QF/QE | F401 capacities remain fixed and checked; measured preparation/instantiation/RUN fit in run 8. Full worst-case capacity qualification for all admitted programs remains C/Q. |
| MEM-03 | A1/A4/B/E/C; QH/QF/QE | B-R4 compiled user calls use explicit VM continuations; depth 1–4 with SysTick passes on F401, boot peak leaves 2,788 B. Construction recursion still exists; exhaustive native-stack and allocation-free RUN proofs remain C/Q. |
| MEM-04 | C; QH/QF/QE | pending — open |
| MEM-05 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R/H): stable runtime fault identities are preserved through shared execution. Rich allocated diagnostic errors remain; fixed-size bounded fault records are still C work. |
| MEM-06 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): aggregate/string/reference and shared-struct COW copies are charged before mutation; independent value semantics and transactional output groups are tested. Allocation-free bounded compound representation remains C. |
| MEM-10 | A4/C; QH/QF/QE | A4 partial (A4-A/R/Q): exclusive private EngineState borrows immutable PreparedModule; the public wrapper prevents untyped mutable-trait access. Bounded storage/lookup allocation qualification remains C. |
| MEM-11 | B/E/C; QH/QF/QE | B-R3/B-R4 targeted compact live storage and measured F401 bring-up feasibility are implemented. Complete bounded-compound storage and allocation-free RUN remain C; no broad closure claimed. |
| MEM-07 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-R): mutable state borrows immutable prepared metadata and checks activation/reference lifetimes. Installed-generation replacement, readers and retirement remain H/C. |
| MEM-08 | A1/A4/B/E/C; QH/QF/QE | A4 partial (A4-E/R): artifact member/method/interface metadata supports FBs, inherited classes, dynamic receiver checks and shared method bodies. Broader admitted profiles and board qualification remain open. |
| MEM-09 | A1/A4/B/E/C; QH/QF/QE | Deferred beyond A1 foundations: no bounded-storage/admission qualification claimed. R3-M records slot size only; full requirement remains open for the listed later scopes. |
| PORT-01 | A4/B/E/H/C; QH/QF/QE | F401 run 8 exercises hardware timer/logical-time separation, deadline fault and independent watchdog. Clock wrap/epoch and all-platform service qualification remain E/H/C/Q. |
| PORT-02 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R): explicit services, native capability checks and work/deadline failures are enforced. Returning test callbacks do not prove cancellation, blocking or allocation bounds; platform/import qualification remains C. |
| PORT-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| PORT-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| CYCLE-01 | A4/H/C; QH/QF/QE | A4 shared image sequencing retained; F401 run 8 samples PC13 and exercises injected input-image transitions driving real PA5 readback. Full platform transfer/completion contracts remain E/H/Q. |
| CYCLE-02 | A4/H/C; QH/QF/QE | F401 output publication and STOP/FAULT transitions pass real PA5 pad readback in run 8; A4 transactional failure regressions retained. General adapter completion qualification remains E/H/C/Q. |
| CYCLE-03 | A1/A4/H/C; QH/QF/QE | A4 order/miss regressions retained; F401 run 8 matches saved nominal-deadline state trace. Other host/C6 replay remains E/H/Q. |
| CYCLE-04 | A4/H/C; QH/QF/QE | A4 shared TON/TOF/TP tests retained; F401 saved TON fixture executes against the hardware timer in run 8. Complete timer/target qualification remains E/Q. |
| CYCLE-05 | A4/H/C; QH/QF/QE | A4 partial (A4-S/H): shared %I/%Q/%M codecs, partial/configuration/access bindings and existing I/O regressions are retained. Full forces/debugger and platform integration remain H/B/E. |
| CYCLE-06 | A4/H/C; QH/QF/QE | A4 partial (A4-R/Q): exclusive mutable state and typed public access enforce between-cycle engineering ownership; input images are staged through exclusive access. Bounded external service queues remain H/C. |
| TIME-01 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R): finite nested work/depth limits and deadline-fault tests cover the selected bring-up engine. No schedulability, physical timing admission or successful-deadline guarantee is claimed. |
| TIME-02 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-R): dispatcher, initializer, helper traversal, allocation/copy and COW paths consume shared limits; budget/deadline errors propagate without mutation bypass. Complete callback bounds and physical cost qualification remain C/B/E/H. |
| TIME-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-06 | A4/B/E/H/C; QH/QF/QE | pending — open |
| TIME-07 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-E/S): saved state/numeric traces use explicit injected logical instants, including non-multiple nominal periods and restart time. Physical wakeup/wrap/service-race and timing classification remain B/E/H. |
| STATE-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-03 | A4/B/E/H/C; QH/QF/QE | F401 deadline fault latches and suppresses normal publication; physical safe-off and IWDG reboot pass run 8. Production configured-failure/supervision qualification remains E/H/C/Q. |
| STATE-04 | A4/B/E/H/C; QH/QF/QE | F401 run 8 turns previously high PA5 output off on STOP and deadline FAULT, with pad readback. General multi-driver confirmation and configured fault outputs remain H/E/C/Q. |
| STATE-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| STATE-06 | A4/B/E/H/C; QH/QF/QE | A4 partial (A4-E/R): explicit warm/cold restart reconstructs state and preserves required retained graphs/aliases; function-static first use and failed restart transactions are tested. Physical reset/recovery policies remain B/E. |
| RETAIN-01 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-02 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-03 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-04 | A4/B/E/H/C; QH/QF/QE | pending — open |
| RETAIN-05 | A4/B/E/H/C; QH/QF/QE | pending — open |
| SERVICE-01 | H/C; QH | pending — open |
| SERVICE-02 | H/C; QH | pending — open |
| DEPLOY-01 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-02 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-03 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-04 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-05 | B/E/C; QH/QF/QE | pending — open |
| DEPLOY-06 | B/E/C; QH/QF/QE | pending — open |
| UPDATE-01 | H/C; QH | pending — open |
| UPDATE-02 | H/C; QH | pending — open |
| UPDATE-03 | H/C; QH | pending — open |
| UPDATE-04 | H/C; QH | pending — open |
| UPDATE-05 | H/C; QH | pending — open |
| UPDATE-06 | H/C; QH | pending — open |
| UPDATE-07 | H/C; QH | pending — open |
| UPDATE-08 | H/C; QH | pending — open |
| COMPAT-01 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-H/E): existing 1.x regressions and the new saved 2.0 consumer exercise shared semantics. Explicit 2.0-only static-FB initialization support is documented; full hosted/platform qualification remains open. |
| COMPAT-02 | C; QH/QF/QE | pending — open |
| COMPAT-03 | A1/A3/A4/H/C; QH/QF/QE | A4 native feature corpus retained; F401 run 8 replays the unchanged Boolean/integer/TON/array/interface/default/retain fixture. Broader feature/platform qualification remains E/H/C/Q. |
| COMPAT-04 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-E/R/H): exact integer/Boolean, scheduler, typed fault, copy-back and saved state assertions preserve selected observable contracts. Broader cross-platform exact-trace qualification remains open. |
| COMPAT-05 | A1/A3/A4/H/C; QH/QF/QE | A4 partial (A4-N): shared libm/conversion behavior and saved numeric expectations distinguish exact results/control outputs from operation-specific tolerances. No arbitrary cross-CPU bit identity is claimed. |
| COMPAT-06 | A1/A3/A4/H/C; QH/QF/QE | Saved A4 main/numeric bytes are unchanged; F401 run 8 matches all 101 main rows and numeric contract. Native Windows/macOS profile replay and physical C6 replay remain open. |
| NUM-01 | A1/A4/B/E/U; QH/QF/QE | F401 run 8 records 84 MHz and FPSCR 0 with pinned Rust/libm/build identity. Full numeric premises/environment qualification on other profiles remains E/U/Q. |
| NUM-02 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): strict observations are separated from tolerance-qualified function results in saved-artifact assertions. Matching numeric/environmental contracts on every claimed target remain unqualified. |
| NUM-03 | A1/A4/B/E/U; QH/QF/QE | A4 partial (A4-N): saved numeric replay checks operation-specific reference tolerances and resulting threshold/command decisions; existing exceptional/width/fault suites remain. Full application/platform corpus remains open. |
| NUM-04 | A1/A4/B/E/U; QH/QF/QE | F401 run 8 passes three exact numeric bit checks, TIME/Boolean state and ten tolerance-qualified function results. Broader application/platform observations remain E/H/Q. |
| NUM-05 | A1/A4/B/E/U; QH/QF/QE | Full shared numeric/standard-library functions remain linked; F401 numeric fixture passes with 9,464 us maximum observed scan. Full target runtime/timing qualification remains E/H/U/Q. |
| PLAN-01 | A1–A4/B | A1–A4/M2A-H and physical F401 M2A-B bring-up are complete with B-R4 run-8 fixture evidence. This closes M2A only; later modernization/hosted/bounded-storage/production scopes remain. |
| PLAN-02 | A1/E; QE | A4 supporting evidence (A4-B): the C6 graph includes the common loader/executor. Physical ESP32 bring-up and qualification remain E/QE; F401 evidence cannot close them. |
| PERF-01 | A1/A4/B/E/U/H/C; QH/QF/QE | F401 run-8 observed main/numeric scan maxima are 5,798/9,464 us on the frozen 84 MHz profile. This is fixture timing, not general WCET or all-platform performance qualification. |
| PERF-02 | A1/A4/B/E/U/H/C; QH/QF/QE | B-R4 run-8 map/ELF, region reservations, physical preparation/RUN/heap/MSP and Value-layout records are retained. Other targets and production capacity qualification remain open. |
| PERF-03 | A1/A4/B/E/U/H/C; QH/QF/QE | No physical latency/timing distribution measured in A1; logical-time tests are R3-S, not timing qualification. Requirement remains open. |
| PERF-04 | A1/A4/B/E/U/H/C; QH/QF/QE | A1 reports bounded evidence and no speed/WCET claim (R3); performance acceptance remains open. |
| TEST-01 | A2/A3/A4/B/E/H/C; QH/QF/QE | A4 partial (A4-A/R/S): first-use/static-restart, hidden references, readonly/type/limit, initializer and output-failure cases extend the native corpus. Full disturbance, hang, service and deployment qualification remains open. |
| TEST-02 | B/E/H/U/C; QH/QF/QE | B-R4 run-8 source/ELF/UART identities, full successful physical capture, failed attempts and independent reviews are retained. Manual observations and production/all-platform qualification remain open. |
| TEST-03 | QH/QF/QE | pending — open |
| TEST-04 | QH/QF/QE | pending — open |
| TEST-05 | A2/A3/C; QH/QF/QE | A4 partial (A4-A/R): admitted-wire mutations, capability/capacity rejection and failed construction/restart preservation are exercised. This is not a broad fuzz campaign or deployment authorization/rollback qualification. |
| MOD-01 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-02 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-03 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-04 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-05 | U; QH/QF/QE for changed tuples | pending — open |
| MOD-06 | U; QH/QF/QE for changed tuples | pending — open |
| DESIGN-01 | A1–A4/B/E/U/H/C; QH/QF/QE | A4 partial (A4-Q): the existing dispatcher is shared through small context boundaries; preparation, construction, assignment/reference policy and lifecycle remain cohesive modules. Later profile designs remain open. |
| DESIGN-02 | A1–A4/B/E/U/H/C; QH/QF/QE | A4 partial (A4-A/R/Q): private admission, exclusive state, typed fallible writes and runtime lifetime/copy/budget checks reinforce the native contract. Fixed diagnostic/generation/bounded-storage design remains C. |
| DESIGN-03 | A1–A4/B/E/U/H/C; QH/QF/QE | A4 partial (A4-Q): independent correction reviews, architecture checks and ownership/flow diagrams cover the shared execution boundary. Remaining host/platform and bounded-profile extraction stays H/B/E/C. |

### A3 evidence keys

- **A3-R1**: failed initial compilation/architecture; dependency audit and isolated no-dev feature trees pass. Unchanged dependency files retain that evidence.
- **A3-R2**: [ledger](../../../notes/runtime-portability/a3-evidence/run-2-ledger.tsv), 221 core/70 portable/139 i686 tests and F401/C6 core compilation pass. Core bytes remain unchanged through A3-R4.
- **A3-R3**: [ledger](../../../notes/runtime-portability/a3-evidence/run-3-ledger.tsv), canonical diagrams render successfully; source-authoring failures remain historical, corrected in A3-R4.
- **A3-R4**: [ledger](../../../notes/runtime-portability/a3-evidence/run-4-ledger.tsv), all 13 required/3 advisory steps pass; [execution and evidence reconciliation](../../../notes/runtime-portability/a3-execution.md) records counts, identities and remaining A4/board boundaries.
- **A3-R5**: [ledger](../../../notes/runtime-portability/a3-evidence/run-5-ledger.tsv); 224 core, 70 portable, 142 i686, both MCU checks, 219 integrations and 3,863 units pass. Lint/warnings/architecture/rendering/metadata pass; three authoring-fixture failures are retained.
- **A3-R6**: [ledger](../../../notes/runtime-portability/a3-evidence/run-6-ledger.tsv); all 31 authoring and four boundary tests pass after fixture-only corrections; all eight required and two advisory steps pass. Production source and generated artifact remain covered by run 5.

### A2 evidence keys

A2-R1/R2/R3 (historical relocation only): [execution and failure record](../../../notes/runtime-portability/a2-execution.md), frozen manifest and full batch-1 logs. Runs 2 and 3 closed the original relocation gates only; subsequent corrections reopened A2 and are closed by A2-R6/R7/R8.

A2-R4: [run-4 ledger](../../../notes/runtime-portability/a2-evidence/run-4-ledger.tsv) and [execution record](../../../notes/runtime-portability/a2-execution.md). Failed expanded correction batch; post-run-4 changes were unverified at that checkpoint, then qualified in A2-R6/R7/R8.

### A4 evidence keys — verified run-11 source

All keys now identify the passing [run-11 evidence](../../../notes/runtime-portability/a4-evidence/run-11/README.md), its exact commands and frozen source/fixture hashes, and [final independent acceptance](../../../notes/runtime-portability/a4-review/run-11-final-independent-acceptance.md).
The historical run-5 evidence remains intact for its older source. These keys
establish software scope proof; later platform and hardware qualification stays open.

- **A4-E**: Run-11 core-all-features/core-portable and selected i686 logs: core/tests/runtime_core_compiler_free_load.rs saved artifact/expected-trace oracle; host/tests/runtime_core_compiler_free_load.rs source-to-saved-to-fresh-core and legacy scan parity. i686 does not run every hosted or numeric suite. Commands, hashes and saved trace are bound by the linked evidence index.
- **A4-A**: Run-11 preparation_profile and source_free_profile_admission native cases plus core preparation/decoder tests: exact accounting boundaries, expansion/import/native state/capability/domain/multiple-resource limits; private API compile-fail contract.
- **A4-B**: Run-11 isolated thumbv7em-none-eabihf and riscv32imac-unknown-none-elf checks and no-dev feature graphs, Rust 1.95.0 tuple. Library compilation only, not linked firmware or execution.
- **A4-H**: Run-11 runtime-unit and affected runtime-integration logs: actual retained API/format/debug/stdlib/tier/differential/I/O/restart assertions, required api_smoke/debug_control/complete_program/runtime_reliability vertical; runtime cross-warnings supplementary only.
- **A4-S**: Run-11 source_free_cycle, source_free_cycle_contract, tasks/tasks_fb/scheduler_resource and saved trace assertions: nominal periods, sampled edges, ordering, process images and retain-before-output failure behavior.
- **A4-N**: Run-11 generated numeric-v2.stbc and core/tests/source_free_numeric.rs: ten LREAL reference results bounded by 8*f64::EPSILON*max(abs(reference),1), exact root/power/sum/scaled TIME/command expectations; existing portable_numeric_contract and shared stdlib conversion/helper/FB suites. These are finite native observations, not general numerical proof.
- **A4-R**: Run-11 source_free_assignment/source_free_readonly/source_free_restart_graph, compiler-free hosted cases and core engine units: typed/COW/constant/lifetime gates, alias/default frequency, class/static/NULL binding, initializer work/deadline, output transactions and restart graphs.
- **A4-Q**: Run-11 complete ledger, frozen source manifest/archive, generated STBC hashes, commands/environment, independent review manifests, affected lint/cross-warning/supply-chain/architecture/render/drift/format/diff outcomes. Metadata/provenance advisories remain individually classified. No release guard, commit/push or physical qualification implied.

- **A4-COST**: Run-11 source_free_execution_cost, borrowed_reference_allocations and source_free_cycle_contract: 3,000 declarations / 2,000 stores, 2,000 bound inputs, sparse 1 MiB marker publication and actual zero reference-path allocations. No resource limit was raised to conceal cost.
- **A4-CORRECTIONS**: [Finding-by-finding disposition](../../../notes/runtime-portability/a4-execution.md#external-review-disposition-evidence), [complete Rust identity](../../../notes/runtime-portability/a4-review/run-11-final-rust-identity.json), and independent acceptance cover RH1/RM2/RM3/RL1–7/RQ1–5, including generic counters, all requested standard blocks, falling edges, hierarchical I/O, exact diagnostics, deadline boundaries and forged staging writes.

### A1 evidence keys

All R3 references identify the unchanged **tested source snapshot**, not a new validation of the subsequent bookkeeping correction. See the [execution record](../../../notes/runtime-portability/a1-execution.md) and its relative evidence links.

- **R3**: all 24 steps pass in [run-3 ledger](../../../notes/runtime-portability/a1-evidence/run-3-ledger.tsv); exact commands/environment/frozen identity are beside it.
- **R3-S**: core `src/task/readiness.rs`, core readiness/case tests, host `tests/tasks.rs` and `scheduler_resource`; [native runtime output](../../../notes/runtime-portability/a1-evidence/run-3-runtime-tests.txt) and [core output](../../../notes/runtime-portability/a1-evidence/run-3-core-tests.txt).
- **R3-B**: workspace/core manifests, core `collections.rs`, `tests/portable_foundations.rs`, isolated F401/C6 checks/feature trees and `.github/workflows/ci.yml`.
- **R3-N**: core `numeric/math.rs`, arithmetic/TIME operators, hosted numeric helpers, `tests/portable_numeric_contract.rs`, numeric/VM/vertical suites; finite reference output is in the native runtime log.
- **R3-M**: core `value/layout.rs`; `record_value_slot_layout` in the core output plus both target build bounds.
- **R3-R**: [review messages](../../../notes/runtime-portability/a1-evidence/review-record.md), architecture/diagram/CI check statuses in the run-3 ledger.

## Batch, review and failure record

A2 batch 1 is retained as failed: 15 required PASS, 2 FAIL (Clippy, unnecessary architecture exemption), 2 UNRUN (diagrams/drift); advisory metadata failed. All 381 native assertions passed. Independent source review completed before the batch. Separately authorized run 2 closed lint/architecture/diagrams but failed three native artifact pins. Separately authorized run 3 passed 25 affected native tests and format/diff checks; the original relocation snapshot was verified, before the expanded corrections reopened A2. Full failures and reviewed corrections remain in A2-R1/R2/R3.


Historical A1 scope table, before release integration: three explicitly authorized
batches ran. These outcomes do not describe the later merge/release status in the
current checkpoint. The field list below remains the template for later scopes.

| Batch | Source / review | Result and disposition |
|---|---|---|
| A1 run 1 | Initial A1 source review; initial frozen source retained | Failed: fixture syntax, MySQL lint and dependency advisories; later host suites unrun. [Ledger](../../../notes/runtime-portability/a1-evidence/run-1-ledger.tsv). |
| A1 run 2 | Security/metadata remediation reviewed; TLS test gap corrected before batch | Failed: duration operand-context lowering and LSP lint; advisory metadata wording failed. [Ledger](../../../notes/runtime-portability/a1-evidence/run-2-ledger.tsv). |
| A1 run 3 | Five-file review hash `64550b9f0909f4292b53cdfcc583c79a469e63418d77a1dbf0c31a26e7abd992`; 73-file frozen manifest retained | Passed: 23 required plus one advisory step; no automatic retry. [Ledger](../../../notes/runtime-portability/a1-evidence/run-3-ledger.tsv), [commands and identities](../../../notes/runtime-portability/a1-execution.md). |
| Post-closeout bookkeeping | Detailed ledger, registry links, gate inventory and portable evidence corrected after review | Documentation/metadata only. No run 4 and no fresh validator result. A1 remains verified, not push-ready. |

A4 batch history is retained independently of the historical A1 table above:

| Batch | Required result | Evidence |
| --- | --- | --- |
| A4 run 1 | 10 PASS, 12 FAIL, 4 UNRUN | [Ledger](../../../notes/runtime-portability/a4-evidence/run-1/ledger.tsv) |
| A4 run 2 | 16 PASS, 8 FAIL, 2 UNRUN | [Ledger](../../../notes/runtime-portability/a4-evidence/run-2/ledger.tsv) |
| A4 run 3 | 23 PASS, 5 FAIL | [Ledger](../../../notes/runtime-portability/a4-evidence/run-3/ledger.tsv) |
| A4 run 4 | 27 PASS, 1 FAIL | [Ledger](../../../notes/runtime-portability/a4-evidence/run-4/ledger.tsv) |
| A4 run 5 | 28 PASS; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-5/README.md) |
| A4 run 6 | 17 PASS, 10 FAIL, 2 UNRUN; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-6/README.md) |
| A4 run 7 | 17 PASS, 10 FAIL, 2 UNRUN; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-7/README.md) |
| A4 run 8 | 19 PASS, 8 FAIL, 2 UNRUN; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-8/README.md) |
| A4 run 9 | 26 PASS, 3 FAIL; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-9/README.md) |
| A4 run 10 | 28 PASS, 1 FAIL; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-10/README.md) |
| A4 run 11 | 29 PASS; both advisories PASS | [Ledger and frozen evidence](../../../notes/runtime-portability/a4-evidence/run-11/README.md) |

| Field | Required record |
|---|---|
| Scope and authorization | Scope ID, boundaries and instruction; explicit rerun authorization if applicable |
| Frozen identity | Branch/HEAD plus dirty patch and untracked-file hashes or complete source snapshot; spec, lockfile, artifact/config hashes |
| Environment | Canonical/destination agent-file verification, builder/platform path, compiler/flags/features, board/OS identity |
| Review | Independent reviewer, reviewed diff identity, findings, fixes/dispositions and unresolved items |
| Planned commands | Exact commands, target/lease, expected assertions, baselines and evidence locations; no hidden retry policy |
| Result | Every command's exit/status, selected/passed/failed/skipped counts, logs and relevant raw measurements |
| Failure ledger | Each failure, unrun dependency, violated requirement, root cause if known, prepared fix and next authorized action |
| Gate decision | Verified only for evidence actually obtained; remaining scope/platform claims stay open |

Historical document-setup evidence (superseded for A1 execution by the records above): The earlier dated report in the
[research archive](../../../notes/runtime-portability/README.md) records v0.8 document consistency
only. The v0.9 scheduling amendment was reviewed against the current sources, draft diff and
IEC §6.8.2; v0.10 corrects its placement to A1 following the sequencing review. No validation
batch, tests, generators or commits were run for either amendment. A2 and later implementation gates were open at that document checkpoint; A1 subsequently passed run 3 and A2 closed through A2-R6/R7/R8.

## Resume and goal use

Read AGENTS.md, the current checkpoint and owning spec first. Verify branch/HEAD/diff and preserve
unrelated work. For another checkout, manually copy and verify canonical AGENTS.md and the full
`.codex/skills/` tree before work; preserve the uncommitted plan too. Read the last batch/failure
record before launching anything. Resume the next incomplete authorized task without replaying
passed commands, reopening historical boards or losing a STOP/failure boundary. Update this
checkpoint before handoff, context reset or stopping, including the next concrete action.

Use one goal per authorized scope. Goals preserve an objective across turns; this repository file
holds detailed progress and evidence across sessions/reviewers. A goal cannot waive scope, STOP,
validation or publication rules. See [OpenAI's goal guidance](https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex).

A2 expanded corrections are verified through A2-R6/R7/R8. Resume from the current checkpoint; no A3 or A2 commit/push is authorized. A1 publication and #129 integration are separate from A2 implementation.

A1 accepted evidence: [execution record](../../../notes/runtime-portability/a1-execution.md) in the A1 implementation checkout; full logs/requirement audit are linked there. Three batches were explicitly authorized separately; none was an automatic retry. That execution record covers A1 only; A2 checkmarks use the final A2-R6/R7/R8 allocation; earlier A2 records are historical.


### Historical post-run-4 correction review checkpoint

Latest external-review corrections are implemented and incrementally reviewed without running
checks. The explicit [130-path manifest](../../../notes/runtime-portability/a2-evidence/pre-run-5-review-manifest.json)
has SHA-256 `98670d41d8846c0200d4050933c24439d632779667cbb75691d5035f7b6992b9`;
the independent reviewer reproduced its records and complete exclusion list. This is source
review only, not a fresh all-file audit or test result. At that checkpoint A2 items remained open and run 5 awaited authorization. The subsequent
authorizations and executed corrections are recorded below and in the final execution note.

A2-R5: [complete stopped-run ledger](../../../notes/runtime-portability/a2-evidence/run-5-complete-ledger.tsv); source-freeze preparation failed before all native checks. It did not close A2; later A2-R6/R7/R8 did.


### A2 final evidence keys

- A2-R6: [ledger](../../../notes/runtime-portability/a2-evidence/run-6-ledger.tsv): core 204,
  portable 53, i686 167 assertions pass; both MCU checks, no-dev graphs, supply chain,
  architecture and rendered diagrams pass. Other failures preserved.
- A2-R7: [ledger](../../../notes/runtime-portability/a2-evidence/run-7-ledger.tsv): runtime
  unit 3,858 and tooling 102 assertions pass; Clippy/cross-target/metadata pass. Two failures retained.
- A2-R8: [ledger](../../../notes/runtime-portability/a2-evidence/run-8-ledger.tsv): all nine
  required steps pass; VM 44 assertions close the corrected NULL fixture; formatting complete.

[Final execution record](../../../notes/runtime-portability/a2-execution.md) explains the
passing union, exact identities and unrun later scopes. No single earlier failed batch is
relabeled passed. Only evidence and closeout documents changed after the final source copy.

The user subsequently authorized the local A2 commit and starting A3 on a separate branch.
Shared release-guard parity files already owned by #129 are excluded from this A2 commit;
they remain preserved locally and will be inherited through upstream integration.


## Historical A3 implementation checkpoint

A2 commit: `9f62fd09181c222ae6f8802128600708e3a304ea`. Canonical bootstrap copied and
verified from `/home/johannes/projects/trust-platform` to this A3 worktree. The three
shared guard parity files remain inherited local differences pending #129 integration.

Design constraints confirmed by source review: ordinary aggregate expression emission
currently rejects initializer aggregates; POU type records omit private instance layout;
initializer lookup sees only earlier frame locals then outer storage; method/function static
ownership differs; concrete reference owners require explicit construction mappings; the
existing execution entrypoint must reject 2.0 until A4 consumes its mandatory semantics.
Global and type defaults must come from authoring declarations/catalogs, not current values.
At that implementation checkpoint, no A3 build, test, validator, formatter, firmware action or commit had run. Run 1 and its failures are recorded below.


### Historical A3 first-batch preparation

The connected 2.0 producer, portable admission/disassembly and native test corpus
are prepared on `feat/runtime-portability-a3` at A2 base `9f62fd091`. Independent
read-only reviews corrected static visibility, frame metadata, dense storage,
lexical identity and source API issues. All three A3 items remain unchecked until
executable evidence is reconciled. At that checkpoint the first consolidated batch was prepared but
had not started; see [A3 execution record](../../../notes/runtime-portability/a3-execution.md).
The new builder checkout uses the same base and receives the canonical rule files
manually. No A3 commit, push, A4 implementation or board execution is authorized.


### Historical A3 run 1 failed; corrections prepared

The first consolidated batch finished on 9 October 2026 with 12 required failures
(11 compilation-dependent steps plus architecture) and 3 dependent steps unrun.
No native test assertions passed; no MCU compilation/execution claim is available.
Supply chain, formatting and advisory metadata checks passed. All A3 boxes remain
open. Evidence: [run-1 ledger](../../../notes/runtime-portability/a3-evidence/run-1-ledger.tsv)
and [execution/correction record](../../../notes/runtime-portability/a3-execution.md).

Compiler-root corrections and a focused stack-validation function split are
prepared locally and independently reviewed without actionable findings. They are untested.
No second batch, commit, push or A4 work is authorized. Next action: obtain explicit authorization for one consolidated validation batch;
its command plan is prepared, with unchanged dependency evidence reused from run 1.


### Historical A3 continuation authorization

After run 1 corrections and independent review, the user replied “yes, you dont
need to ask” to the additional-batch request. This authorizes run 2 and necessary
A3 follow-up validation after complete fixes/review, without repeating permission
questions. Preserve each frozen batch and full failure ledger; do not retry
unchanged failing commands or weaken gates. A3 commit/push and A4 remain outside
this authorization. Coordinate builder use with the separate #129 release guard.

### Historical A3 run 2 checkpoint

Portable/core tests and both MCU library checks passed; hosted compilation and
diagram rendering failed. Core source remains unchanged by the prepared facade
and diagram fixes. A3 items remain open. See the current checkpoint and A3
execution record for counts, evidence and standing continuation authorization.

The changing-input fixture uses the IEC-permitted reference initialization in
spec 12 §11.5.10: an FB array element selected by the current method input. It
preserves source constant-expression diagnostics and must be evaluated per call
by the A4 oracle.


### Historical A3 run-4 closeout

Historical run-4 closeout: all three A3 items were checked using the source-compatible evidence chain above. They were reopened for the subsequent external-review corrections; those changes are now verified by the separate R5/R6 closeout below.
P/Q compatibility policy is recorded in spec 12; actual product versions remain a
pre-P release gate. Existing behavior tests are retained for A4's shared engine.
The generated 2.0 artifact has not been executed by that engine. No A3 commit,
push, A4 implementation or board work was performed. Only closeout docs/evidence
changed after the successful frozen candidate; Rust and fixture bytes are preserved.

### Historical A3 external-review correction batch

A3-R5 completed: all gates passed except three authoring-fixture assertions. A3-R6 subsequently passed every planned step. All review findings and quality recommendations are
implemented. Independent read-only review identified and rechecked the expression-only
type discovery fix, edge-test field correction and canonical-action rejection tests.
The retained 113-path review manifest/archive has identity
`5b44303ab8373e80235ff678ec6a66cfb1337446ec5e94da6be64ad439555f5c`.
At that pre-batch checkpoint no native or MCU result was claimed for the correction tree. The subsequently completed consolidated batch includes
all changed core/host layers; no commit, push, A4 or hardware execution is authorized.

### Historical A3 correction closeout

All external findings and quality recommendations are implemented and verified:

| Review area | Implemented result | Evidence |
|---|---|---|
| Staging writes | CFG must-proven staging destinations, native target checks, no synthetic owner | R5 core/i686 and R6 forged-global authoring regression |
| Artifact demand | Reachable stdlib templates, expression-only type discovery, canonical recipe sharing | R6 pruning, SIZEOF and canonical-link positive/negative tests; R5 generated 6,164-byte fixture |
| Missing features | Edge, retain, partial alias, configuration, class and lowered partial-write coverage | R6 31 authoring + four boundary tests |
| Diagnostics and tooling | Specific rejection reasons, explicit stable disassembly, strict prefixes | R5 core + R6 authoring; generated text retained |
| Code quality | Named opcodes, shared reference helpers, one partial table and invalid helper, grouped state/typed IDs, prebuilt action indexes | Independent review; R5/R6 lint/compile/native evidence |
| Provenance | README indexed; exact current review input manifests and archives retained; old digests labeled historical | Artifact index and review archives |

Closeout-only documentation changes follow the run-6 freeze; no Rust or fixture bytes
were changed afterwards. Scope verification is not an exact-SHA release artifact,
source-free execution or hardware qualification. No A4 implementation occurred.

### Historical local A3 commit and initial A4 authorization

The owner authorized a local A3 commit and a separate A4 implementation branch.
`19fbdad18` records canonical workflow files (byte-identical to the independently
reviewed A1 integration candidate); `7e6938a75` records the tested A3 implementation,
format specification and fixtures. This evidence/checkpoint commit completes the
local A3 series. No extra test run or A3 publication was performed. A4 has one
authorized consolidated batch after implementation and independent review; it
does not inherit A3's follow-up-run authorization.

### Historical initial A4 execution checkpoint

Source/command map and implementation decisions are in
[the A4 execution record](../../../notes/runtime-portability/a4-execution.md).
A4 has one authorized validation batch after completed implementation and review.
No automatic retry, commit/push, board work or A1/A2 release change is authorized by
that initial development authorization. At that checkpoint all RTP-A4 boxes were open.

### Historical A4 run-5 scope closeout

The current checkpoint and A4 evidence keys supersede that initial authorization
checkpoint: standing reviewed correction cycles were subsequently authorized. Runs
1–4 retained failures; run 5 passed every planned step. See the full [batch history](../../../notes/runtime-portability/a4-evidence/run-5/README.md).
At that historical checkpoint RTP-A4-01..03 were closed. The external review below
reopened them; current corrections remain unverified. M2A remains open until
actual F401 scope B execution.
No A4 commit, push, hardware work or later scope was performed.


### A4 external-review correction scope

The owner requested every finding and recommendation fixed. Run 5 remains historical
proof of the prior source. Run 11 and final independent reconciliation verify every
correction below; A4-CORRECTIONS provides the finding-by-finding evidence.
No limits may be raised to conceal program-size costs, and no failure assertion may
be weakened. Requirement meanings are extended by spec 34's execution-cost contract
and spec 12's dedicated fault identities. All work stays in the A4 worktree.

- [x] A4-RH1: Prepare indexes once; destination-only output transactions and input staging; binding-based image work. Native realistic-size regression.
- [x] A4-RM2: Borrowed reference policy paths; hosted zero-allocation regression.
- [x] A4-RM3: Dedicated fault variants/codes, owning checks and native fault assertions.
- [x] A4-RL1: Preserve raw evidence externally; keep reviewable summaries and hash/location manifests in git; ignore scratch artifacts.
- [x] A4-RL2: Cache borrow failure is a miss, including instance type-cache callers.
- [x] A4-RL3: Bounded deadline polling stride with explicit boundary checks.
- [x] A4-RL4: One execution budget across dispatcher, helpers, initialization and hosted tiers.
- [x] A4-RL5: Remove unreachable construction branches; correct executed trace header.
- [x] A4-RL6: Software checklist closure explicitly excludes later board replay.
- [x] A4-RL7: Source-free TOF/TP/counter/trigger/bistable/falling-edge/hierarchical-I/O/deadline/staging-forgery execution coverage.
- [x] A4-RQ1: Group EngineState lifecycle, work, image and construction state by responsibility.
- [x] A4-RQ2: Named record predicates and one partial-access wire mapping.
- [x] A4-RQ3: One prepared immutable standard-library registry, reused during restart.
- [x] A4-RQ4: Documented portable facade; hide mutable VM internals and remove blanket docs suppressions.
- [x] A4-RQ5: Full dirty-Rust review manifest, verbatim-move equivalence inventory and independent review of each authored slice, plus independent final evidence reconciliation.

Resume status: all external-review changes are implemented and independently
verified against run 11. The final 239-path Rust identity and independent evidence
reconciliation are linked above. All A4 software items are closed; physical board
replay, release qualification and later bounded profiles remain their own scopes.
No A4 commit or push occurred. The A2 integration agent continues separate release
work. No further A4 test run is planned after the passing consolidated batch.

B-R1 run-2 closeout and prepared corrections: [execution record](../../../notes/runtime-portability/b-r1-execution.md#consolidated-software-validation-run-2). No checkboxes are closed from unrun corrected-source gates.
