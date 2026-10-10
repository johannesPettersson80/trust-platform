# B-R4 native-stack correction

**Current result:** run-8 software/ELF and physical F401 bring-up pass. Independent final review
and retained evidence are below; the original plan and every failure remain historical.
The owner subsequently authorized all necessary correction tests and hardware retries.
No commit, push or production qualification is claimed.

Owner: “fix it and run the hardware tests again”. Authorization covers complete
implementation, independent review and one consolidated software/link/inspection
batch with physical replay conditional on software/ELF acceptance. No automatic
retry, commit, push, functionality/depth cut, RAM repartition or weaker margin.

Checkout trust-platform-portability-b, feat/runtime-portability-b, HEAD
df427259cc387a7a79fb81132be23e48ea1493d4. Canonical AGENTS/skills from primary
trust-platform were verified (21 files) before work. Baseline is B-R3 run-3 ELF
87d622c71fa6f6dab01d13caef9db2671a174f9ccb575b0398d76389b8f61c5b,
with passing software/flash but failed physical stack evidence. Board remains halted.

Spec34 §6.2.2 owns the correction. Dispatcher/call agent owns shared continuations;
root owns specification, firmware phase boundaries/harness, tooling and evidence;
independent reviewer traces initializer paths and reviews shared execution changes.
This was the pre-run design; the completed run-1 outcome is recorded below.

Planned batch: reuse B-R3 run-2 complete command allocation, adding the new continuation
and host-testable stack-guard assertions through their existing suites. Prepare/format
once, reconcile review, then core all-feature/no-default/i686, F401/C6 graphs, hosted
unit and 68 integration suites, adapter/tooling and firmware library, canonical firmware
link + unchanged ELF inspector, all Clippy/warning/audit/architecture/diagram/format
and metadata gates. Preserve all independent failures. Physical run uses a new evidence
directory and the same full flash backup/explicit erases/verification/25-second capture
procedure; prior board capture remains untouched. No repeated full pre-push release gate.


Design selected: call preparation produces an immediate builtin result or an owned
user-call plan; explicit continuations retain return/output/edge state alongside the
existing FrameStack. A per-callee operand floor isolates callers. Shared completion
also serves the hosted synchronous optimized adapter. New metadata growth is fallible,
charged and capped by admitted depth. Error unwind restores edges, parked locals and
activation identities without replenishing fuel. Legacy direct CALL keeps its operand
contract. Hosted buffer pooling must retain the new continuation capacity.

Initializer finalization is extracted verbatim behind one non-inlined boundary after
lexical locals are restored, reducing temporaries live across initializer dispatch.
Native tests cover deepest admitted initializer depth, rejection, trap cleanup and reuse.
This does not eliminate all construction/type/recipe native nesting or claim a whole-
profile stack bound.

Firmware main/numeric/GPIO fixture entries receive explicit phase boundaries. The
host-testable stack guard enforces the existing 16 KiB capacity and 2 KiB free margin
for MEM samples, after each depth record and before repainting inactive stack. Failed
measurements propagate through existing safe-off/failure/stopped-idle handling. No
fixture shortening, memory repartition, runtime feature cut or claim that sampling can
prevent the first overflow inside an operation. Source reviews are retained externally.


Independent reviews accepted the 13-file shared-call slice, four-file firmware slice,
and initializer extraction/tests. Review found an initializer pre-entry ownership bug:
lexical locals moved before fallible entry checks could be lost. The staged Option now
retains ownership until frame capacity is reserved; errors restore lexical locals from
the entered frame or retained staging frame. Native rejection/reuse assertions cover it.

Hosted ExecutionBuffers are now pooled whole, preserving continuation capacity instead
of allocating it on each scan. Existing public fields remain available; external struct
literals for this internal HIR interoperability type would need Default construction
because continuation state is private. Product Runtime entry APIs and PLC semantics
are preserved. New regression cases reside in existing core/native test binaries.
No source run occurred before the single formatting/provenance preparation.


## Run 1 — retained failure, hardware not launched

The single authorized batch ran on 10 October 2026, from preparation at 20:05:50 UTC
through completion at 20:13:07 UTC. Frozen source manifest has 1,011 records and SHA-256
`ec2cc5372aa76a691df42dc88035de28f7aa9ee38e676bf5f44d38b678d5e0b9`.
The ledger records **36 required PASS, 4 required FAIL and 2 advisory PASS** rows.
No retries occurred. [Evidence](b-r4-evidence/README.md) retains commands, raw logs,
source identities, reviews and the footprint comparison; large raw objects stay outside git.

| Native suite | Passed | Failed |
| --- | ---: | ---: |
| Core all features | 399 | 0 |
| Core portable | 360 | 0 |
| Core i686 | 280 | 0 |
| Hosted unit | 3,841 | 0 |
| Hosted integration, 68 binaries | 541 | 1 |
| Platform and xtask | 91 | 0 |
| Firmware library | 2 | 0 |
| Provenance helper | 4 | 0 |

Failures:

1. The new `deferred_calls_preserve_null_presence_and_suspended_local_copyback`
   fixture used `pointer`, a reserved case-insensitive keyword, as a parameter name.
   Source compilation failed before its behavioral assertions; this is not a failed
   execution assertion. The sibling NULL/default case and other new call tests pass.
2. The image links but leaves **16,064 B** in the upper flash region, **320 B below**
   the unchanged 16,384 B floor. Span is 442,688 B, 480 B above B-R3 run 3;
   sector 0 has 224 B free. The inspector correctly rejects it and produces no passing
   ELF report. ELF SHA-256:
   `1be0a4bf7879d8f82886c5466b63dcd3bec4b2c8ca32273915d0db675e238404`.
3. Both affected and portable Clippy reject one `manual_clamp` expression in
   FrameStack capacity growth. These are two failed gate rows for the same source issue.

Every other required software gate passed, including both MCU core graphs, cross-target
warnings, both lock audits, architecture, rendered diagrams/drift, formatting and fixture
parity. Generated diagrams were copied back. Hardware replay is **unrun** because both
software and ELF prerequisites failed; the physical board remains halted from B-R3.

Linked prologues show the dispatcher own frame fell from 2,864 to 2,344 B, and initializer
evaluator from 1,312 to 968 B; former recursive native-call helpers are absent. Firmware
phases now have separate frames. These are static local-frame measurements, excluding
callees/interrupts; they do not establish physical headroom or a whole-profile bound.

## Prepared corrections after run 1 — unverified

The fixture uses `ref_arg` at all four sites, retaining its expected 133 assertion.
Fixed-bound growth uses `.clamp(1, VM_MAX_CALL_DEPTH)` after checked multiplication.
Root and deferred calls now share one non-inlined input-copy helper, preserving length
checks, clone charges/order, supplied-value presence, initializer ownership and faults.
Nine repeated firmware failure-record paths share one concrete cold writer; protocol
bytes, ignored UART errors, safe-off order and returned failure tags remain unchanged.
These remove duplicated work without changing functions, limits, layout or margins.
Their size effect is **not measured**. No post-correction test, formatter, link or hardware
run has occurred; the run-1 source freeze continues to identify the tested version.

Next execution requires explicit authorization under AGENTS.md's no-automatic-reruns
rule. Proposed allocation: one preparation/freeze with independent exact-source review,
the affected core/portable/i686 and hosted suites, firmware/tools, lint and required
cross-target/audit/architecture/format/provenance gates, one canonical firmware link and
unchanged ELF inspection. Only all required PASS results permit the already-planned
one physical replay with stack/heap/depth and safety/watchdog verification. No weakened
threshold, feature cut, automatic retry, commit or push is authorized.


Independent review of all seven corrected Rust files is accepted at identity
`17368e7a40975f4adce55cd69e5755a217fd2d65d3c389a55966ab1db6dcf03f`.
It caught a StableErrorCode-to-string boundary mismatch in the new failure helper;
all eight call sites now use the existing allocation-free `as_str()` representation.
This was corrected before further execution. Review does not prove compilation or fit.


## Continuing authorization

Owner now directs: “do as mny tests as required to fix it, dont stop”. This explicitly
supersedes the earlier no-automatic-reruns boundary for this correction/hardware task.
Continue root-cause fixes, independent review and evidence-retained test/link/board
attempts as required, without additional permission questions. Full functions, stable
toolchain, overflow checks, all flash/RAM/stack margins and no commit/push remain.
Run 2 starts from the reviewed seven-file corrections; each run keeps its own source
freeze, ledger and artifacts. Hardware remains conditional on software/ELF acceptance.


## Run 2 — complete software and ELF pass

The continuing authorization covers this run. Frozen manifest has 1,171 records,
SHA `a44d0a232571559bd0815df7ef6c4acce29d0649003edf5af2da49e9004052d5`.
All 40 required and two advisory steps passed. Native counts match run 1 except
hosted integration now passes all 542 tests, including the repaired fixture.
The new ELF passes the unchanged inspector: 442,176 B upper span, 16,576 B free,
sector 0 free 224 B; heap/MSP remain 72/16 KiB. ELF SHA
`2c226db7d2ac5aa7c38b4efd9f0f22ac3e727b9be45180f23d962c5442e66c8c`.
The shared-copy/failure-record corrections recover 512 B against run 1 in aggregate.
No isolated per-edit saving is asserted.

Before hardware replay, review found two adapter DBP writes lacking the explicit
PWR_CR dummy read required by ST RM0368 section 5.4.1. The correction adds that
volatile read immediately after each write, in normal reset control and terminal
fault handling. The reinstall script also records and clears only the existing
backup phase-cookie words, checking PWREN, DBP and clear readbacks. No backup-domain
reset, firmware reboot-loop bypass or unrecorded previous evidence removal occurs.
The firmware's unexpected-reset guard remains active.

Run 3 is a focused final adapter/firmware batch: native tools/firmware tests, ARM
adapter check, adapter/firmware Clippy, one canonical link and ELF inspection, metadata,
architecture/diagrams and formatting. Exact source/lock/fixture parity reuses run-2
core, hosted, cross-target and audit proof; no such suite is relabeled as rerun.
One physical attempt follows only all required passes. Continuing authorization
allows further root-cause correction if necessary, with each attempt retained.


## Run 3 — software pass; physical instantiation still fails stack criterion

Focused run 3 passes all 30 required software and two advisory steps at manifest
`38d0a13dbd8186789ff7c64f955273acf863e4d60991e5fc0840b241c00cbda0`.
Final ELF SHA `3ad794db9695f5106c22d1bdc0fc4901c51dfca370999bc8520adef28fae8b7c`
retains 16,576 B upper free space. One physical installation/capture ran. Flash and
application verification and firmware/checkpoint preservation pass. The explicit
reinstall recorded prior cookie `5452ffff/abad0000`, enabled and checked PWR/DBP,
and cleared only that marker. Firmware's runtime guard is unchanged.

The board emits main-prepare heap peak 28,272 B and stack 10,256 B used / 6,128 B
remaining, then main-instantiate heap peak 22,408 B and saturated painted-stack
16,384 B used / 0 remaining. It reports `B1,FAIL,runner,stack-headroom` and stops.
The unchanged trace verifier fails the headroom criterion. No cycle, depth probe,
safe-output transition or watchdog-reset qualification is claimed for this attempt.
The board was halted without a reset/replay; returned idle MSP was `0x20017f50`.

A diagnostic read of the already-painted unused SRAM, from ELF `__sheap=0x2001203c`
to stack top, finds first non-paint word at `0x20013eac`: 16,724 B observed written
stack, 340 B beyond the reserved region, with 7,792 B untouched below. cortex-m-rt's
startup painter and exact ELF symbols establish this range. This read does not enlarge
the reservation or change the 2 KiB required margin; it is diagnostic high-water,
not a maximum-SP or whole-profile proof.

Owning source path: nested instance/default/array construction still synchronously
enters initializer dispatchers. Common recursive `construct`/`node` frames hold
branch-specific aggregate/override temporaries across those callbacks. The next
correction separates these existing branch bodies while preserving charges, order,
errors and all domains; no boxing, RAM partition, depth or function-set change.
Implementation/review and new recorded tests/link/physical replay continue under the
owner's explicit repeat authorization.


Run-4 implementation isolates existing Apply-Struct recipe overlay, array/struct,
enum and subrange bodies behind their own non-inlined helpers. Only values.rs and
its test module change. Two new native tests pin exact work/value/allocation boundaries
and cleanup/check ordering; existing assertions remain. Full run-2 software allocation
will execute, with the canonical link/inspection first for prompt native-frame evidence.
No flash or stack saving is credited before measurement; board replay still requires
all software and ELF gates. Run-3 source and physical failure are separately retained.


## Run 4 — software/flash pass; physical replay withheld on stack evidence

Full run-4 software allocation passes at frozen manifest
`63138bc047e54224a9ea2edd970cdb323db24d74c4bda12f7c4dddf9fb8f7298`
(1,431 records). Native core lanes pass 401/362/282 tests; hosted unit 3,841,
hosted integration 542, tools 91 and firmware 2. ELF
`5c0da1f439ae538b0f44f9bebdeac7d7c128aa5b368aa10899ba6ceac8dc49c4`
passes inspection with 16,640 B upper free. No hardware replay of this candidate.

Linked frames show construct 632→536 B, node 672→328 B, with a new 304 B array
helper active only on that branch. The identified nested path loses 1,016 B but
still has 15,328 B of fixed frames before leaves/IRQs, above the allowed 14,336 B
usage. Physical replay was withheld rather than knowingly exceeding the criterion.

The next source change preserves the existing start_call boundary with inline(never).
Call-specific binding/continuation temporaries otherwise remain inlined into the
2,336 B common dispatcher even when executing defaults with no user call. No function
body, limit, allocation or exception behavior changes. Run 5 first measures the
reviewed frozen candidate; native core/portable/i686, affected call/initializer suites,
runtime vertical and applicable gates follow if the linked evidence supports replay.
The run-4 full hosted corpus remains separately identified evidence, not rerun proof.


## Run 5 — measurement only; remaining construction-stack gap

Source freeze `2fb19a24c1d344bf3e991b984a422ba7558314f4eb95ea290a0a77ca263da928`.
The canonical link and inspection pass. ELF
`820b2571053da7dd28caba3d5fafbebc40c82f59211f348926ed71ef895c3871`
leaves 18,624 B upper free, recovering 1,984 B against run 4. Dispatcher own frame
falls 2,336→1,872 B; start_call now has its own 888 B frame. The identified nested
instantiation chain still totals 14,400 B fixed frames, 64 B over the permitted
14,336 B usage before leaf/interrupt overhead. This is accepted measurement evidence,
not stack-readiness approval. Run-5 native validation and hardware replay were not
launched. No measurement-reviewed acceptance marker was issued for those phases.

Next measured correction separates existing shallow state allocation/reservation
from deep resource construction, preserving restart transactions, charges, limits
and ordering. No boxing, new persistent allocations or profile changes are authorized
by that implementation choice. Repeat authorization remains active.


## Run 6 — unfavorable measurement, not retained as an optimization

Frozen source `6a8823e1b138ec193157ed08c0935da87548b377eddebeb2a7399c93d5f713b2`.
ELF `7c5aa689752d2bc199f56f9094b5cd8066a42d6b1a5403a62bf2aca7a41550fb`
passes flash inspection with 18,592 B free, but the caller's state/result temporaries
increase instantiate_with_services from 1,824 to 1,976 B. The identified path grows
to 14,552 B before leaves/IRQs. Native validation and physical replay were not launched.
The allocation-phase extraction is reverted to its exact prior source; the unfavorable
measurement remains. No heap ownership redesign is substituted silently.

The next candidate preserves the existing metadata-entry cloning helper boundary so
its branch-specific clone/allocation temporaries do not inflate the common recursive
construction frame. The clone result, charges, callbacks and ordering remain unchanged.
The next image is measured before deciding whether to run further native/physical gates.


## Run 7 — software pass; physical margin still insufficient

Frozen source `e1213d088ec94eb0767cc4ad88c4f402218f06df91caaed29d0f670909aa3aff`.
All 39 required software and two advisory gates pass. Core 401 / portable 362 / i686 282,
20 affected hosted binaries 170, native tools 91 and firmware 2 pass. Full hosted
3,841-unit/542-integration proof remains independently labeled run4 evidence.
ELF `f11b73ae4b6138d253df2aeea386c77c4c5e88d6a493b94d581325699e5f7caf`
passes inspection with 18,592 B upper free. One physical attempt installs/verifies
correctly and preserves firmware/checkpoint regions around the application write.
Main preparation reports 28,272 B heap peak and 10,328 B painted stack; instantiation
reports 22,408 B heap peak and 14,668 B stack used / 1,716 B remaining. This is below
capacity but 332 B short of the required 2,048 B headroom, so the guard and verifier
correctly fail. The board is halted; later physical phases remain unrun.

Next correction is firmware composition only: prepare returns Box<PreparedModule>
through an explicit phase boundary, moving the immutable control owner from the
persistent fixture stack into the existing 72 KiB measured heap before deep
instantiation. Box allocation precedes PREP/MEM reporting; state drops before the
owner. Standard allocation failure uses the existing terminal OOM/safe-output path.
No custom unsafe allocator, core API, runtime function, admission check or memory
reservation changes. Spec34 §6.2.2 and firmware README state the ownership policy.


## Run 8 — full-function physical bring-up passes

Frozen source `a95bcff98b5d4a51e70acf7e29cf0113b585ac69bcfc39c4cc5fbac6b1e8337c`.
ELF `a0e6ed2d7f725fd17746d30c229d02e5b0125c00c494015f3be40322e293ec1e`
leaves 18,720 B free in the upper firmware region and 224 B in sector 0. The same
stable Rust 1.95.0, safe ICF, overflow checks, complete library/import functionality,
device-side validation, heap/MSP reservations and acceptance thresholds remain.

The firmware-only ownership correction allocates the 804-byte prepared owner in the
existing measured heap before PREP/MEM reporting. The main fixture native frame
falls from 2,488 to 1,416 B; numeric and GPIO frames each fall by 1,064 B. The
preparation helper grows by 792 B but returns before instantiation. State still drops
before its borrowed prepared owner. Hardware counts the allocation and all drops.

The first software attempt stops at unchanged-source parity before tests: preparation
correctly moved the registered HardFault source line from 133 to 135, while the old
reuse list still pinned line 133. The corrected validation2 excludes that changed
metadata from reused proof, pins its current bytes, and reruns architecture. Its
17 required software steps and one advisory step pass; raw native counts are tools
91, firmware library 2 and provenance helper 4. It does not relink or reformat.
Core 401 / portable 362 / i686 282 and the affected hosted 170-test corpus remain
run-7 proof through 169 unchanged-source/lock/artifact pins. The broader 3,841-unit
and 542-test/68-binary hosted corpus remains explicitly historical run-4 evidence.
The three intervening core changes were reviewed function-boundary attributes;
run 7 reexecuted their applicable native/hosted corpus. No historical result is
relabeled as a new run-8 execution.

### Installation failure and guarded second attempt

The first physical attempt fails during OpenOCD flash programming, before the UART
case starts. An unexpected reset enters the partially written new image; the saved
exception frame points from main/critical-section acquisition into the unprogrammed
`__primask_r` location, followed by the unprogrammed HardFault handler. RCC reset
flags do not show an IWDG reset. The trigger of the programming-time reset remains
undetermined; no runtime failure or watchdog explanation is inferred. The complete
failed log, exception/reset registers and partial-flash backup remain retained.

A reviewed installer correction enables reset/HardFault vector catch during writes,
asserts its DEMCR readback, and retains verbose debugger evidence. It clears all
catch bits and verifies readback before the one execution reset. This protects
against executing a partial image on another unexpected reset; it does not claim
to identify that reset's trigger. The second attempt writes and verifies the exact
456,168-byte firmware image and 14,760-byte application bundle, preserves firmware
around the separate application write, and preserves both checkpoint sectors.
No firmware source, compiler configuration or linked artifact changed between attempts.
OpenOCD documents the protection in [Cortex-M vector catch](https://openocd.org/doc/html/Architecture-and-Core-Commands.html).

### Physical acceptance

The second capture contains all 156 ordered records and ends with the independently
identified watchdog reboot and DONE. UART SHA-256:
`9601cceef51d46170961c74e0afe2aba33b44250d3795225f6df60055a315704`.
The unchanged Rust trace verifier passes and is appended to validation2's ledger
as its eighteenth required passing step.

| Measurement | Physical result | Acceptance / interpretation |
| --- | ---: | --- |
| Main oracle | 101 matching samples through 1,000 ms | 40 periodic activations, zero missed intervals |
| Numeric fixture | 101 samples; 40 activations; zero misses | All exact/tolerance numeric assertions pass |
| Peak measured heap | 28,272 B | Below unchanged 73,728 B arena; zero failed allocations |
| Heap after each fixture drop | 0 B | Includes prepared-owner deallocation |
| Boot-wide stack peak | 13,596 B | 2,788 B free, exceeding 2,048 B minimum by 740 B |
| Call depth 1 / 2 / 3 / 4 stack | 7,336 / 10,348 / 10,348 / 10,348 B | Real activated calls; SysTick count increases |
| Maximum observed main scan | 5,798 us | Finite fixture observation, not WCET proof |
| Maximum observed numeric scan | 9,464 us | Below its 10,000 us fixture deadline; not general timing admission |
| Physical Value slot | 32 B, alignment 8 | Target evidence |
| GPIO and safe outputs | PASS | PC13 sample, injected image transitions with PA5 pad readback, BOOT/STOP/FAULT off |
| Independent watchdog | PASS | Deliberate withheld feed; reset flag and safe boot; catches disabled |

The firmware ends in supervised STOP with outputs off after the watchdog reboot.
Manual button actuation, optical LED inspection and printed board/package markings
remain unverified as allowed by the remote bring-up contract. This is Scope B
fixture feasibility, not an exhaustive stack/WCET proof, zero-allocation RUN,
durable RETAIN, production deployment or release approval. M3/M4 and later
checklist gates remain. All source and evidence are uncommitted.


## Closeout

Independent source, linked-image, software, physical and requirement-ledger reviews
accept the specified Scope B fixture gate. RTP-B-01 through RTP-B-05 and
RTP-BR4-01 through RTP-BR4-07 are complete. The failed B-R2/B-R3 candidate
rows remain historical failures with their successor disposition stated explicitly.
The current requirement ledger closes only the F401 bring-up portions, preserving
all remaining platform, bounded-storage, timing, persistence and release obligations.

The final retained [evidence index](b-r4-evidence/artifact-sha256.json) covers every
curated file, including its README and external-artifact index. Large binaries,
maps, complete disassemblies and archives stay outside the repository, with
verified SHA-256 pins. The frozen compiler/runtime/firmware/tooling source is
unchanged; only closeout evidence/documents and validated generated diagrams follow
the source freeze. Agent usage exhaustion during curation was handled by the root
agent; completed independent acceptance remains retained. No additional validation
was needed for the evidence-only closeout, and no commit or publication occurred.


## Local commit preparation

The owner subsequently requested “commit”. Scope B implementation and its reviewed
fixture evidence are committed as a local checkpoint before release-version metadata.
Current runtime/firmware/tooling sources match the run-8 frozen inputs; historical
evidence retains its original 0.24.71 build tuple and linked ELF. A separate local
metadata commit prepares version 0.24.73, following fetched main's 0.24.72, without
changing dependencies or source semantics. Its exact committed candidate still needs
the release guard before any push; no guard, push, merge or release is included in
this commit instruction. The primary checkout and board image remain untouched.
