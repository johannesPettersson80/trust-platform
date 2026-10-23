# A4 shared-engine execution record

Status: A4 external-review corrections are scope-verified by run 11 and final independent acceptance; uncommitted. Checkout `trust-platform-portability-a4`, branch
`feat/runtime-portability-a4`, base `77b91381fcf1b1850611f88b397bbfe8d45e3523`.
Canonical rules were manually copied from the primary checkout and all 21 files
matched. The owner initially authorized one implementation/review/batch scope, then explicitly
authorized reviewed correction cycles with “dont ask you fix it” and “continue a4
until its done.” Each completed batch remained frozen and its full failures were
retained. No per-command automatic retries, A4 commit/push or hardware work.

## Scope and architecture

Specification 34 §13.1 and checklist RTP-A4-01..03 own this scope. Extract the real
hosted stack dispatcher, not a second fixture-specific interpreter. Core already
owns values, arithmetic, frames/stacks, traps and the bytecode boundary; this scope
adds shared mutable storage, prepared execution metadata and execution.

- Shared storage preserves hosted public APIs; synchronization of lookup caches is
  a platform/composition concern. Source-free type policy comes from TYPE_TABLE.
- Move the existing opcode loop, call binding/copyback and reference operations.
  Host adapters keep debugger callbacks, profiling, deadline clock and register-tier
  selection; no thread-local pool or std::time::Instant enters the portable path.
- Both initializer/default bodies and ordinary POUs use that dispatcher. Establish
  the shared instruction/depth/deadline budget before any frame initializer.
  Frame-entry lifecycle is centralized across every supported entry path.
- Mutable reference contents need recursive runtime lifetime/staging checks in
  addition to A3 static admission. Preserve normalize-all-before-output-copyback.
- Move storage-backed native FB/timer state transitions with their hidden state;
  retaining only Ton::step would omit existing timer identity/width behavior.
- Keep the 1.x HIR/Expr initialization adapter solely for legacy compatibility.
  Fresh 2.0 construction/execution cannot depend on a compiler or Expr interpreter.
  Full hosted register-tier migration remains the later HOST program as specified.

Independent read-only seam audit by `/root/a1_readonly_review` identified the
pre-existing local-init-before-budget ordering and raw-CALL frame-entry difference.
Neither is accepted for new executable initializers. No tests were run by review.

## Historical run-1 validation plan

All builds/tests run on trust-builder using the pinned 1.95.0/edition 2021 tuple,
locked dependencies, a dedicated target/TMPDIR and target lease. Coordinate the
heavy slot with A1/A2 release validation; do not launch intermediate compiles.
Finalize the exact deduplicated script/features and expected assertions before
freeze, based on the complete implemented diff.

- Core all-features native tests, portable no-default native and i686 execution,
  F401/C6 no-default library checks. Native tests must use saved STBC with no HIR.
- Saved-fixture trace: exact 10 ms samples; 25 ms task runs at 30/50/80/100 ms and 40 times
  through 1000 ms, zero overruns; TON, interface state, array and changing reference
  initializer, edges and retain/restart, numeric/control corpus.
- Initializer budget/depth/deadline exhaustion, nested/aggregate reference escapes,
  failed preparation/replacement preserving old state, output/fault boundaries,
  ordinary/default/after-restart frequency and native output-copyback ordering.
- Host full unit suite and affected VM, initializer, restart, scheduler, retain,
  interface/OOP, core behavior locks and runtime vertical integration suites.
- Affected Clippy, runtime cross-target warnings, architecture full-map then diagram
  rendering/drift, formatting including touched fragments, and diff integrity.
  Supply-chain check if dependency edges change. Provenance maintenance is advisory.

A4 scope evidence is not firmware execution, hardware timing qualification, native
macOS/Windows execution, an exact-SHA release guard or release permission.

## Historical implementation checkpoint — 10 October 2026

All changes below are authored only, unformatted and unverified. No A4 validation
has run. The worktree deliberately contains incomplete integration while the
source-free state implementation is being completed; it is not commit-ready.

- Shared storage moved into core; hosted public storage path re-exports it. The
  hosted cache lock behavior remains; no_std uses checked single-owner cache borrows.
- Actual storage-backed native FBs/timers, scalar standard functions, conversions
  and value display now live in core. Host SystemTime remains an adapter. Conversion
  built-in names and generic-name rejection retain their prior error behavior.
- Actual decoded VM metadata, type policy, references, opcode loop, native binding,
  call/output normalization and edge transactions moved into core. Hosted entry
  selection/debug/profiling/register tier and legacy HIR initialization are adapters.
- Shared frame initialization establishes fuel before defaults. Every supported
  frame entry invokes the lifecycle hook; native calls preserve the same budget.
- Source-free frame identities and ownership transfer for suspended caller locals
  are authored, with native regression tests. No cloned snapshot substitutes for
  live ancestor reference writes. Native reads/writes have mandatory context gates.
- Private PreparedModule preparation and RuntimeState scaffolding are authored.
  RuntimeState construction, initialization, access/I/O, restart/retain, admission,
  lifetime/visibility policy and saved-artifact execution are still incomplete.
- A separate implementation agent owns shared typed construction/coercion under
  vm/construction/values.rs, including exact callback frequency tests. That agent
  also authored scalar and call extraction; another reviewer must review those edits.

Before freezing: finish all state/lifecycle paths and fixtures; reconcile extraction
imports/visibility, architecture ownership and diagrams; replace stale hosted-owner
metadata references; complete independent review of the entire diff. Run no partial
compile or formatter as a substitute for that source completion.

## Historical pre-run-1 safety closure — 10 October 2026

The worktree remains unverified; no A4 build, formatter or test has run. Independent
source review found and implementation closed these concrete gaps:

- Deep assignment shape/type normalization now covers direct/dynamic VM writes,
  call bindings/copy-back and external access; it never evaluates defaults.
- In-process restart constructs at the preserved logical time, remaps retained owned
  objects and non-owning interface/reference bindings, and preserves old state on
  failure. Resource globals and function statics retain distinct restart rules.
- Native output groups stage storage/current locals, including suspended ancestor
  values, so conversion/work/deadline failure cannot expose partial copy-back.
  Empty output groups avoid snapshots. Callee execution is not rolled back.
- Artifact preparation resolves actual native imports/capabilities, rejects ambiguous
  multiple-resource compositions in this first profile, checks decoded and encoded
  limits consistently, and accounts for expanded metadata/constants separately from
  validation scratch and runtime construction. Concrete accounting was independently reviewed; physical memory peaks are not claimed.
- Review corrected invalid source fixtures before execution, including reserved
  identifiers, writes to FB outputs and unsupported dynamic scalar defaults.

The numeric/control companion source is `numeric.st`. The existing fixture generator
will produce `numeric-v2.stbc` and its disassembly once in the authorized batch;
`source_free_numeric` is dependent on that generation. It loads saved bytes without
HIR, checks the finite numeric corpus and exact control decisions, and prints the
observed final numeric bit patterns. No placeholder artifact or invented trace is
checked in. The original program-v2 artifact remains the A3 saved-fixture baseline.

Bring-up allocation remains explicit: owned map/array/string storage, per-call local
vectors, construction metadata, snapshot transactions for input/output groups and
retain/restart, and standard-library binding buffers. This is allocator-backed Scope
A4, not Scope C zero-allocation RUN qualification or firmware memory-fit evidence.

Release integration is separate: PR129/v0.24.70 is fully released. PR130 corrections
at b703482be are locally committed/reviewed and their additional batch is authorized;
that does not consume this A4 batch.


## Historical pre-run-1 review closure and batch handoff

Current review records are [admission](a4-review/admission-review.md),
[assignment and COW](a4-review/assignment-review.md),
[lifecycle](a4-review/lifecycle-review.md), and
[batch preparation](a4-review/batch-review.md). Their JSON manifests identify the
reviewed bytes and each verdict states author-owned exclusions. Later deltas are
recorded explicitly; none of these reviews is compilation or execution evidence.
The final external-alias inspection found and corrected physical-declaration
shadowing. The [independent addendum](a4-review/assignment-review-external.md)
records the accepted-wire regression and the seven-path correction identity.

The public RuntimeState has a private internal engine, explicit typed writes and
immutable storage inspection. It does not implement mutable dispatcher context
traits. Read-only declaration checks protect direct, dynamic and native-output
stores. COW charges precede path writes, including writes into staged input storage.
The decoder charges typed allocation requests before reservation; those charges
flow into preparation. Retained graph transfer uses owner/root indexes and actual
lookup work rather than repeatedly charging full global scans.

The allocation limits are concrete logical charges, not a complete allocator
ceiling: frame vectors, map bookkeeping, diagnostic/native-call temporaries and
allocator overhead still require later allocation instrumentation and bounded
storage work. No zero-allocation, total heap cap or MCU memory-fit claim is made.

Local and builder worktree base is `77b91381f`; canonical agent files match the
primary checkout (21 files). The builder has 16 CPUs and configured Cargo jobs=6.
A1 owns the heavy validation slot first; A4 will use its isolated mounted-volume
target and TMPDIR after that slot is released. No A4 validation has started.


Implementation and source review are complete for run 1. All RTP-A4 items remain
open until executable evidence passes. The pre-format source manifest records the
reviewed implementation; the batch will retain the formatted/generated source
manifest separately. The setup files are retained in
[a4-evidence/batch-setup](a4-evidence/batch-setup/README.md), including the reviewed
mounted-volume lease tools inherited by A1 integration. No A4 commands have run.


### Current continuation authorization

After being told that A1 awaited another validation authorization, the owner said
“dont ask you fix it”, followed by “and continue a4 until its done”. Work continues
through correction, independent review and consolidated validation without another
permission question. This is not permission to retry individual failing commands,
edit a running batch, bypass a release gate, or claim failed checks passed. Each
batch retains its complete failure ledger before any correction and its reviewed
source identity before a subsequent consolidated run. A4 commits, publication and
hardware remain separate from the implementation scope.


## Run 1 and correction scope

The complete first batch ran 05:36:26–05:40:04 UTC on 10 October. Its
[retained ledger and evidence](a4-evidence/run-1/README.md) record every failed and
unrun layer. No core or hosted runtime behavior assertion executed: compilation
failed before those suites. Only the two helper tests and 16 mutation-tooling tests
passed. All required A4 acceptance boxes remain open.

Corrections fix the missing shared-frame presence field and unused bindings,
extract cohesive helpers from the three oversized functions, and classify the
already-separated shared VM namespace. They also move the active retain-mutation
binding to its actual core owner and improve failure diagnostics. Historical
mutation measurements stay untouched. Existing execution assertions are retained.

The canonical release skill's guard tests also exposed missing tooling dependencies
on this older branch. The reviewed `cargo_target_path.sh`, lease helper and idle-only
removal helper are adopted unchanged from the A1 integration candidate, closing the
actual dependency rather than bypassing those tests or emulating their behavior.
Their bytes are already retained in the batch-setup evidence. The next run uses the
local checked-in lease helper and disables Python bytecode-cache writes so tool
execution cannot change copied canonical cache files.

The owner’s standing correction authorization applies. All corrections are reviewed
before the next consolidated batch; no individual failed command is retried. A1 has
the shared builder while A4 corrections and review proceed locally.


## Reviewed run-1 corrections and run-2 plan

The complete correction set is source-reviewed in
[the six-file compiler/construction review](a4-review/run-1-a1-corrections.md) and
[the assignment, provenance and tooling review](a4-review/run-1-correction-review.md).
The [run-2 batch review](a4-review/run-2-batch-review.md) checks its commands and
reconciles run-1 counts. Its wording finding is resolved by labeling all pre-run-1
planning/safety/handoff blocks as historical. Current state remains unverified.

Run 2 keeps the same core/native/i686/MCU, hosted, lint and architecture gates,
adds the affected mutation-contract test module and disables Python cache writes.
All source corrections precede its final formatting/provenance preparation. It
waits for A1 to release the builder, then runs as one consolidated batch; there
are no per-edit or individual-command retries.


## Run 2 result and reviewed corrections

[Run-2 ledger](a4-evidence/run-2/ledger.tsv) records 16 required passes,
eight required failures and two required unrun prerequisites; three advisory
steps passed. The 303-record manifest and complete logs are retained with their
artifact index. Core all-features had 297 passes and one failed admission fixture;
portable no-default had 259 passes and the same failure. The i686 lane passed 181
tests. Saved primary STBC execution passed on both native architectures. F401/C6
core checks passed, with no firmware or board execution. Numeric generation and
hosted tests failed at compilation. No hosted assertion pass is claimed.

The reviewed correction set removes unused host bridge exports/imports, documents
the moved FB registry export, preserves formatter tests through the public API,
removes an identity type helper and an unused initialization helper argument,
uses `size_of_val` for the equivalent slice charge, and supplies valid construction
metadata to the profile rejection fixture. The mutation selector now binds to
its actual source operator/function rather than an obsolete line number.
Review records are in `a4-review/run-2-a1-corrections.md` and
`a4-review/run-2-correction-review.md`. These corrections remain unrun.

Run 3 also fixes the validation harness dependency: register the candidate in an
isolated Git index before mutation contracts, pass that index only to those
contracts and reuse it for metadata. Focused mutation-runner tests run separately
with GIT_INDEX_FILE removed because their fixtures create temporary repositories. This preserves the tracked-source requirement without
staging or committing the real worktree. The complete required batch is retained;
no individual failed command is retried. Standing owner authorization for reviewed
correction/validation cycles applies. No A4 commit, push or hardware work.


## Run 3 result and correction scope

[Run-3 evidence](a4-evidence/run-3/README.md) retains every command and the
364-record frozen source identity. Required ledger: 23 passed, five failed;
two advisory steps passed. No step was unrun. Numeric generation/replay now passed,
and both MCU core checks and Windows cross-compilation passed. Hosted execution
reached 3840 passing unit assertions and 508 passing integration assertions;
one unit and 15 integration tests failed. The core profile fixture still failed
its positive wire-admission prerequisite in both feature configurations.

Corrections address the owning causes: resolve global reference addresses without
requiring a VM frame; preserve native regression assertions; update two source
architecture checks to their actual moved owner/function boundaries; extract the
provenance digest helper; repair admitted-wire fixtures without relaxing validation;
use legal ST enum names, FB state mutation and configuration paths; preserve
initializer-frequency intent with correctly ordered type defaults; and investigate
the producer's function-static FB explicit-initializer restriction against spec 12.
No pre-fix behavior failure is claimed for fixtures rejected before execution.

Run-3 source remained frozen throughout. The complete output and formatted source
were copied back before corrections. All A4 boxes remain open. Standing owner
continuation authorization applies to the next independently reviewed consolidated
correction batch; no A4 commit, push or hardware work.


### Run-4 candidate

The reference-address fix retains the failing tier-1 assertion unchanged. Source
fixtures retain their original lifecycle assertions: local-reference cases now
redirect a typed global LOAD_REF_ADDR operand to a typed local reference of the
same encoded width and require positive admission. No NULL encoding assumption
or permissive validator change remains. Retain-domain rejection rebinds an
existing physical root rather than adding an unowned External declaration.

The 2.0 producer's function-static FB overrides are now explicitly distinguished
from unchanged legacy 1.x rejection in spec 12 §11.5.9 and the changelog. Class
static explicit initializers remain unsupported. Run 4 retains every run-3 command,
changing only artifact/setup paths; independent source reviews are retained under
`a4-review/run-3-root-corrections.md` and
`a4-review/run-3-fixture-and-batch-review.md` plus the producer/fixture review.
No per-edit tests were run. Run 4 is unrun at this checkpoint.


## Run 4 result and remaining corrections

[Complete evidence](a4-evidence/run-4/README.md): 27 required steps passed,
one failed; both advisory steps passed. Native totals: core 299 all-features,
261 portable, 181 i686, hosted unit 3841 all passed; integration 521 passed and
two failed. No skipped step or filtered numeric fixture remains. All lint,
cross-target, supply-chain, architecture, diagram and metadata gates passed.

The next correction adds the relocated core source directory to the architecture
test's declared source-oracle roots. Its original body assertions remain unchanged.
The valid retained-reference fixture remains unchanged while the owning runtime
path-type resolver gains artifact-backed POU member traversal. The observed
TypeMismatch occurs in the first scan, before restart. It must not be described
as a measured retain-transfer failure. Independent review and the consolidated
run-5 plan remain required; no per-edit tests or A4 publication/hardware work.


### Reviewed run-5 candidate

The reference-type correction follows admitted POU member declarations and parent
templates, handles untyped program roots, and shares physical-member lookup with
write-permission checks. External aliases cannot shadow physical type metadata.
Work/deadline failures propagate unchanged; retained-field metadata queries consume
the staged restart candidate budget. The original failed retained-reference test
is unchanged. New native tests cover inherited/public global fields, a valid
non-owning External shadow on a program-root path, budget error classification,
and retained-object copying with exhausted old scan fuel.

Independent source reviews:
`a4-review/run-4-reference-types-review.md` (seven paths, identity
`31e1281c11ef7e31edb54d36ff3392b0d8fd4f93cdc67ebe16cff8c48000bda9`),
and `a4-review/run-4-architecture-batch-review.json` (source-oracle root plus
unchanged command plan under run-5 paths). No per-edit execution occurred.
All run-4 failures remain historical; the complete run-5 batch is the next gate.


## Final A4 scope closeout — 10 October 2026

Run 5 passed all 28 required steps and both advisories. Exact evidence, counts,
command/environment tuple, frozen source identity and the full failed-run history
are in [the final evidence README](a4-evidence/run-5/README.md). Core counts are
301 all-features and 263 no-default (including doctests); selected i686 183;
hosted units 3841; hosted integration 525 across 64 binaries. All have zero failures.
The saved primary/numeric artifacts and expected logical trace are pinned there.

The final tested manifest is
`c14b104c6b93d047148b87af4beb26a1fb20d920687dc8f26011f0cdbc8a9458`
(478 records). Tested Rust, manifests, metadata bindings and fixtures were copied
back unchanged. Subsequent edits are closeout documentation/evidence only.
Source review manifests and their findings remain under `a4-review/`; final
reconciliation is `a4-review/run-5-evidence-review.md`. Previous “unrun/open”
statements above describe their named historical checkpoints, not this closeout.

RTP-A4-01..03 are complete for shared-engine software integration. A1–A4 complete
the aggregate host-side scope A/M2A-H. The M2A physical checkpoint remains open
until scope B runs the same artifact on NUCLEO-F401RE. Allocation-free RUN, full
hosted migration, durable retain/install recovery, hardware memory/timing and
all-platform qualification retain their later scope gates. This closeout does
not authorize or claim A4 commit/push, release approval or physical hardware work.


## External-review correction scope — active

The owner requested every finding and recommendation fixed. The checklist's
A4-RH1/RM2/RM3/RL1..7/RQ1..5 rows own the complete slice; Scope B remains unstarted.
The specification now requires indexed metadata access, destination-only journals,
binding-based I/O work, borrowed reference checks, one work budget, bounded physical
deadline polling, dedicated fault identities and missing 2.0 execution assertions.
Raising the default work limit or weakening existing assertions is not a correction.

Implementation ownership: prepared indexes/record predicates/shared registry;
transaction/I/O/reference borrowing/cache changes; root integration of budget,
fault classification, state grouping/public facade, evidence hygiene and missing
coverage. Every authored slice receives review by a different agent. A full dirty
Rust manifest will classify exact moves, import-only adaptations and authored code,
with explicit independent coverage of all three classes. Final evidence reconciliation
must be performed by an agent who did not author the candidate being reconciled.

No builds, tests, formatters or validators have run for these new corrections.
Finish all implementation and test authoring, then independent review, freeze and
one consolidated builder batch. Coordinate the heavy slot with A2 publication.
No A4 commit, push, additional platform or hardware work is authorized.


### External-review correction interruption

Agent execution was interrupted by the account usage limit during implementation. No correction validation batch has started. The prior run-5 evidence is historical and does not validate the current source. No A4 commit, push, or hardware execution is authorized by this checkpoint.

Prepared indexes, registry precharge, destination-only rollback, borrowed references, binding-only I/O, cache borrow recovery and their source cross-reviews are present. Root added dedicated diagnostic variants, preparation/profile rejection routing, specification updates and source-free block/I/O/deadline test authoring. These changes remain unexecuted.

Outstanding: finish the context-owned execution-budget migration and deadline stride (ExecutionBudget/ExecutionEntry and the host field are scaffolding only); classify remaining protection errors and update exact native assertions; group EngineState; finish the documented private VM facade; author the forged dynamic initializer execution regression; externalize raw evidence with verified copies and location records; correct trace header and provenance; independently review the complete final source, then freeze for the consolidated builder batch. Add source_free_execution_cost, borrowed_reference_allocations and source_free_blocks_and_io to the batch. Preserve every historical failed ledger.

Agents stopped on the usage limit before completing the newly assigned budget and facade tasks. Inspect actual working-tree bytes before resuming: shared files may contain partial edits. The A2 integration agent received root clearance for its preparation-record/docs-script review, but then also stopped on the usage limit; inspect its branch and processes before claiming a commit or guard launch.

### Resumed external-review corrections

All three agents resumed from the preserved source. A4 remains implementation-in-progress with no new validation run. The runtime state is grouped into lifetimes, construction, images and resource accounting. Dedicated fault identities now distinguish protection, profile and decode limits; existing readonly assertions were updated to the owning codes. Added native source authoring for standard blocks, falling edges, hierarchical I/O and physical deadline interruption, plus a private forged-initializer dispatcher regression.

Historical raw scratch and source archives are preserved externally by the SHA-256 location map `a4-evidence/external-artifacts.json`; original run inventories remain unchanged. The authored trace header now identifies its historical replay without claiming current verification. Run-6 orchestration is prepared outside the checkout under `/home/johannes/projects/.artifacts/runtime-portability-a4/setup-run-6`, not launched. It adds the execution-cost, borrowed-reference allocation and block/I/O suites and the additional include-fragment formatting paths. Final review and source freeze still precede the batch.

### Run-6 source review and builder preparation

Fresh independent full-source review cleared the corrected candidate without an
implementation blocker; inventory `a4-review/run-6-full-source-inventory.json`
has 237 Rust/deletion records and SHA-256
`d2fd819d99d6112b6baeb90c8836c04fa2cbfd954557b7053781310b4c99c8f8`.
The separate independent budget review's findings were corrected before this
clearance: logarithmic physical-destination journal deduplication, final restart
prepublication deadline, prepared task/retain lookups, lifecycle fragment formatting
and explicit formatter parity for reviewed unsafe-site locations. New native
regressions include 2,000 distinct scalar input bindings and restart expiry after
candidate construction. Full findings/dispositions remain in the review record.

Builder preparation preserved its prior dirty source (528 records) under
`~/.cache/trust-portability-a4-evidence/pre-run-6-builder/` before synchronization.
All 210 historical crate records matched run 5; there were no unexplained builder
crate edits. The synchronized candidate matches all 237 reviewed Rust/deletion
records and all 21 canonical instruction files. The first inventory comparison
script assumed deletion records had a current hash and stopped with KeyError;
reading the schema and checking deletion absence separately completed comparison.
This was source-copy verification, not a build/test attempt.

Run 6 has not started. The separate A2 release batch still owns the builder heavy
slot; recheck resources and its completion before launching the reviewed A4 batch.
No A4 source changes or additional test runs are authorized during frozen validation.

### Run-6 failure and reviewed correction

Run 6 finished: 17 required PASS, 10 compile FAIL, 2 fixture-dependent UNRUN;
both advisories PASS. Helper 4 and Python 16/20 assertions ran; core/host behavior
assertions did not compile. All 48 raw artifacts match local/builder hashes;
the 571-record frozen manifest is
`3a41ebb08ae3990a7754decd366af48632ec904bd59bacb1197f32f7e6be7681`.
Raw logs/post-batch source are external at
`/home/johannes/projects/.artifacts/runtime-portability-a4/run-6/raw`; small
ledgers and manifests are in `a4-evidence/run-6`.

After completion, formatted source/generated diagrams were copied back. Five
corrections restore the accidentally removed still-used value accounting method
byte-for-byte from run 5, fix RetainedGraph's own once-map references, remove an
unused import, import the no_std vec macro and use a precise VmTrap pattern assertion.
Independent review `run-6-failure-and-corrections-independent.md` pins all five
files at aggregate 9293d401c14f6da85e587d2ecf2a072a2bdfce45e7493b46124644dff4b98631.
The earlier read-only review missed these compile errors; no compilation proof is
claimed from either review.

The existing standing authorization explicitly permits reviewed correction cycles
without repeated permission questions. Run 7 therefore repeats the full consolidated
allocation after these completed/reviewed corrections, with identical commands and
only evidence/setup path changes from run 6. No command retry within a batch or
source edits during validation are permitted. Run 7 is not yet launched at this record.

### Run-7 failure and reviewed feature-boundary correction

Run 7 finished with 17 required PASS, 10 compile FAIL, 2 fixture-dependent UNRUN
and two advisory PASS. No core/host behavior assertion executed. Its 48 raw artifact
hashes match local/builder copies, with 584 frozen records at
`fcf05bff2ed1c9fa0cec6b3308f3f1ad752bf81ebc9a9767459f60a426acd5c6`.
The prior five compile errors are gone; this run exposed no_std dead hosted metadata
and profiler paths, an unused retired helper, a misattached variant comment and
a needless borrow in the initializer-result store path.

All known failures are corrected without suppressions. Derived hosted debug maps
and retained function-name indexes are feature-gated, while portable raw debug
decoding/validation, temporary native-call resolution, reference typing and existing
unit fixtures remain. Budget inspection remains available for host/test callers.
The obsolete helper was removed exactly, the comment attached to its owning variant,
and the borrowed slice passed directly. Independent report
`a4-review/run-7-failure-and-corrections-independent.md` and its eight-file manifest
pin `82ecade5fffa75c07c4051885d1c43e39d7728c907bfd2b70ca6582503d52a73`.
No compilation claim is made from that review.

Run 8 uses the identical consolidated allocation with only evidence/setup paths
changed, under the existing reviewed-correction authorization. It is queued behind
the separately authorized A2 exact-SHA guard; no A4 test launch until the heavy slot
is released and resources rechecked. No per-command automatic retry, A4 commit/push
or hardware work.

### Run-8 failure and reviewed correction

Run 8 completed with 19 required passes, eight failures, two fixture-dependent
unrun steps and two advisory passes. Core all-features had 313 passing / 2 failing
assertions, portable core 273 / 2, and i686 196 / 1. Both MCU library checks passed.
The first two core lanes filtered the saved numeric fixture assertion because
host fixture generation failed. Hosted unit/integration assertions did not execute.
All 48 raw artifacts were verified locally and on the builder; the 598-record
frozen manifest is `f008f9f574853ebb3b0c9d8dfc65bf566676c636a4862e72fd38903adf0f362f`.

After preserving the complete batch, four files were corrected: the exact stable
code inventory now counts 83 codes, program-root replacement expects its dedicated
fault while preserving unchanged-state assertions, the obsolete hosted sizeof
re-export is removed, and five hosted helper calls pass already-borrowed slices
directly. Independent correction identity:
`3f799b197a212dabba58fd2d2cb85abeeaaecef8512b0d51ec8d05ca6667d8da`.
No warning suppression or weakened assertion was used. Run 9 repeats the consolidated
command allocation under the standing reviewed-correction authorization, without
per-command retries. No A4 commit/push or hardware work.

### Run-9 failure and hosted compatibility import

Run 9 completed with 26 required passes, three failures and two advisory passes.
Core all-features passed 316 assertions, portable core 276, i686 197 and hosted
unit 3,841, without filtering or ignores. Both MCU library checks and fixture
regeneration/parity passed. Integration assertions did not execute: one resource
limit test still imported ensure_global_call_depth through the now-private root
instead of the public hosted facade, also blocking Clippy and cross-warning checks.

All 50 raw artifacts were hash-verified. The frozen 612-record source manifest is
`0b37c8fbaacd6f101e916f793cc549f3c211b8beebd2062a04a44a72eaf162f2`.
After the complete batch and archive, only the test import was corrected; its
constants and all assertions remain unchanged. A full workspace caller audit
covered both crate names, re-exports, aliases, tests and examples and found no
other missed private-API callers. Independent review precedes run 10, which keeps
the same consolidated command allocation under standing correction authorization.
No runtime implementation, assertion, limit or suppression changes in this repair.

Independent run-9 reconciliation and correction identity: `55a6d89cb517bc4ffbe40ffc1d41b06e163e65980503957a60880d9349e14c24`.

### Run-10 behavior failures and owning corrections

Run 10 completed with 28 required passes, one failure and two advisory passes.
Core all-features/portable/i686 passed 316/276/197 tests; hosted unit passed 3,841.
The 67 hosted integration binaries executed 534 passing and two failing tests.
No assertions were filtered or ignored. Clippy, both MCU checks, Windows cross
warnings, supply chain, architecture, diagrams and formatting passed.

The counter test fails during instantiation because the shared assignment policy
rejects descriptor 0x0100, although the producer and construction layer implement
specification 12 section 11.5.4's NULL/unbound ANY_INT state. Correct that shared
policy and add direct native allowed/rejected-value tests without replacing generic
CTU/CTD/CTUD with fixed-width variants. The hierarchical negative case uses bit 99,
which the parser correctly rejects before the binding API. Use a valid unbound
address and assert InvalidIoAddress from the binding operation itself.

All 50 raw artifacts are preserved and verified. Frozen manifest:
`f04283f6b39067c84478b3e675572b4fd796b994baa4d2832fc00a4c908e4844`.
Implementation and independent review precede the unchanged consolidated run-11
allocation. The A2 exact-SHA release guard currently owns the builder slot.
No source changes occurred during run 10 and no A4 commit/push is authorized.

The five-file correction passed independent source review and native counter lifecycle inspection.
Correction manifest identity: `44ab23dc4d327410ffc8dec8e0fad3b90a9f755862d45e0036aec039a4732928`.

The run-11 pre-launch copy comparison detected three stale Rust files, then a
complete dirty-file comparison detected stale specification/evidence records too.
No test was launched. Explicit transfer followed by checksum-based full sync
produced a matching 659-record dirty/deleted/canonical inventory. The original
run-10 bytes remain archived. A successful transport exit alone was not treated
as proof of source parity.

### Run-11 passing batch

All 29 required and both advisory steps passed on the pinned Rust 1.95.0 /
edition 2021 builder tuple. Core all-features passed 318 tests, portable core 278,
selected i686 musl 199, hosted unit 3,841 and hosted integration 536 across 67
binaries. There were no failed, ignored or filtered assertions in those suites.
Both isolated MCU core checks, Clippy, host/Windows cross-target warnings, supply
chain, architecture, diagram rendering/drift, formatting and diff checks passed.
Metadata validated 1,029 records; helper/tooling assertions are retained separately.

The generic counter fixture is unchanged and now executes successfully. The
hierarchical case reaches the intended unbound-input rejection and then samples
and publishes through actual bindings. Execution-cost, sparse-image, allocation,
shared budget, deadline, restart and forged-initializer protections all pass.
These are finite software regressions, not hardware timing/memory qualification.

Frozen 643-record identity:
`abc68ced75de1b39e1510028c4ce9669dd435ce310fa5be2892d9e2d4599e83b`.
All 50 raw artifacts are copied and hash-verified; source/diagrams are copied back.
See [run 11 evidence](a4-evidence/run-11/README.md). Failed runs remain unchanged.
Independent reconciliation must finish before checklist closure. No A4 commit,
push, exact-SHA release guard, firmware link or physical board run occurred.

### External-review disposition evidence

These rows describe implementation and the run-11 software evidence. The independent
final reconciliation is recorded separately; this execution note is the author's
closeout, not a substitute for that review.

| Finding | Owning correction and passing evidence |
|---|---|
| High 1 — program-size scan costs | Prepared owner/slot/POU/member/initializer/edge indexes; destination journals and bound-input staging; publication charges bindings. source_free_execution_cost passes 3,000 declarations / 2,000 stores and 2,000 input bindings; source_free_cycle_contract passes sparse 1 MiB marker publication. Limits were not raised. |
| Medium 2 — reference allocation | Borrowed ValueRefView reaches storage traversal; hosted_field_load_and_store_do_not_allocate_reference_paths passes an actual allocation counter. Owning address instructions still construct references. |
| Medium 3 — fault identity | Dedicated protection/lifetime/alias/state/profile/decode-limit variants and stable codes; exact core code inventory and source-free protection assertions pass. Specs describe the owning faults. |
| Low 1 — raw artifacts | Raw scratch/source archives preserved externally with verified SHA/location manifests; /.artifacts/ ignored. Lean run ledgers and manifests remain reviewable. |
| Low 2 — no_std caches | Failed cache borrows are misses; native portable cache contention/invalidation cases pass without changing storage semantics. |
| Low 3 — deadline polling | Shared stride plus explicit entry/completion checks; cycle, engineering-write, restart and mid-cycle expiry tests preserve old destinations/output state. |
| Low 4 — duplicate budgets | Context-owned ExecutionBudget is shared across nested calls, initializer dispatch and hosted tiers; no fresh initializer allowance. Core and hosted budget/resource-limit cases pass. |
| Low 5 — dead paths / trace | Retired construction helpers removed. Trace comment now points to the execution ledger; all bytes after its first comment remain identical to run 11. |
| Low 6 — scope boundary | RTP-A4-02 freezes software replay for later B/E; no board replay is claimed by A4 closure. |
| Low 7 — execution coverage | TOF/TP, CTU/CTD/CTUD, triggers, bistables, falling edges, hierarchical I/O and physical-deadline callback faults execute through STBC 2.0; a forged dynamic initializer store is rejected. Generic counter ANY_INT policy has direct positive/negative alias/value-class tests. |
| Quality 1 — state grouping | EngineState groups lifecycle, construction, images and execution resources by ownership; host and portable lifecycle/restart tests pass. |
| Quality 2 — named representation rules | Record predicates and shared PartialAccess::from_wire remove duplicated flag/type decisions; partial-access/assignment and I/O cases pass. |
| Quality 3 — registry reuse | PreparedModule owns the immutable native library; preparation charges demand before allocation; instantiate/restart reuse it. Admission/restart/budget cases pass. |
| Quality 4 — public facade | VM internals are private; documented portable state/preparation APIs and a HIR-only hosted compatibility facade remain. Compile-fail contracts and all affected hosted callers pass. |
| Quality 5 — review coverage | Complete 239-path Rust/deletion map, conservative move classification and each focused correction are pinned independently. Final format and evidence reconciliation is separate. |

The post-batch trace comment change is documentation only. Run-11 CSV SHA:
`01dcc96b546bb5ba403f770f769f4abc50f037471273eb8eeead232924348732`;
current CSV SHA:
`f58fa24d4bb4b0d8fed473b9cbccc1565ac58c698d90178fff55081801245c54`.
The native reader filters comment lines; no expected row, STBC byte or Rust source
changed after validation. Other post-batch edits are evidence/closeout documentation.

### Final independently reconciled closeout

[Final independent acceptance](a4-review/run-11-final-independent-acceptance.md)
reconciles all 50 raw artifact hashes, the 643-record frozen manifest, all native
counts and every external-review disposition. It also rechecks the 255 archived
raw-evidence location entries and all generated diagram/STBC hashes. The final
239-path Rust identity is [recorded separately](a4-review/run-11-final-rust-identity.json).

Acceptance SHA-256:
`683af7f5ecccb5c4dde06a95bbd32f792592ce60f2aac2717e31b8fcdbba435a`.
Final Rust-identity SHA-256:
`657f202d26f8d15a6b7ab54b706fb21bf974eac428bdcf4cb01e12db2574b4e0`.

The one pre-format Rust-file difference was independently reconstructed to its
already recorded SHA and compared with the tested bytes: only wrapping/trailing
commas changed. Its reconstruction is labeled explicitly, not passed off as a
previously archived file. The post-batch CSV first-comment edit leaves every byte
after that line unchanged. Other post-batch work is evidence/closeout documentation.

RTP-A4-01..03 and all external-review correction rows are now closed for software
scope. M2A still needs physical F401 Scope B. No further A4 test run, commit, push,
release guard, firmware link or hardware execution was performed or implied.

### Authorized local checkpoint commit

The owner requested “commit then start next step.” Commit preparation preserves
the verified Rust/STBC bytes and recorded raw evidence. Staging the formerly
untracked logs exposed their intentional trailing whitespace; .gitattributes now
applies the same byte-preservation policy as A1–A3 evidence. One extra checklist
EOF blank line was removed. These are commit bookkeeping changes, not a new
validation run or a claim of release-guard approval. Release version/integration
work remains separate from this local implementation checkpoint.
