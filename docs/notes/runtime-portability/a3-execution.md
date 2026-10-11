# A3 execution record

A3 is **scope-verified and committed locally** on `feat/runtime-portability-a3`, based on
A2 commit `9f62fd09181c222ae6f8802128600708e3a304ea`; implementation commit `7e6938a75`. Production source and full
core/host/MCU-check evidence pass in run 5. Run 6 closes its three fixture failures
and passes every required/advisory step. No A3 push, source-free execution or hardware qualification is claimed.
The records below preserve the implementation and failed-batch history. Sections
marked historical describe their earlier checkpoint; the verified closeout below
is the current state.

## Implementation delivered (historical preparation notes)

- Draft STBC 2.0 records and codecs: storage declarations (40 bytes), persistent
  roots (24), initialization actions/recipes (60) and access aliases (20). All four tables
  are mandatory, including empty tables. Legacy 1.0/1.1 reading remains separate
  from source-free 2.0 admission and the legacy producer remains the default.
- Initializer-result references, ordered default/explicit stages, configuration
  actions, lifetime checks, declaration/path indexes, root-demand accounting,
  edge-phase association and native FB state reservation.
- On-demand `DEFAULT_VALUE` instruction for disabled-call results; its 2.0 timing
  difference from legacy authoring-time capture is explicit in spec 12 §11.5.7.
- Frontend lowering separated from legacy materialization. Original globals,
  configuration actions and program type/instance identities survive lowering.
- Structural source binding helpers, instance/global reservation, declaration
  templates and receiver-aware default type selection. The explicit
  `CompileSession::build_bytecode_module_for_version` path now connects the 2.0
  producer. It remains unverified pending the consolidated batch; connection and source
  review are not proof of a successfully produced artifact.
- Direct-address syntax moved to the portable core with the hosted re-export;
  configuration actions preserve direct address area, width, bit and hierarchy.
- Initial version-boundary, authoring-boundary and address-parser tests authored.
  They are not a completed A3 regression corpus and have not run.

## Historical pre-run source/design review

Read-only review traced the actual constructor and encoder paths. Its findings
changed the draft rather than being deferred:

- Preserve the separate default and explicit passes and nested-construction order.
- Preserve per-instance ordered VAR_CONFIG actions and global RHS name resolution.
- Keep original program POU identities before configured instance renaming.
- Do not evaluate parameter or disabled-call defaults during source-free authoring.
- Carry edge metadata, phase seeds and limited program warm-restart semantics;
  do not invent inherited-FB or serialized-retain guarantees.
- Carry native hidden-state slots and access alias names/permissions.

A subsequent spec-only review identified three wording contradictions (mandatory
section count, destination/result type and exempt initialization roles), corrected
in the draft. It also requested an explicit disabled-call recipe and precise
frame/static failure boundaries; §11.5.7 records those decisions. At that pre-run checkpoint, final implementation review remained outstanding.
Subsequent reviews and validated outcomes are recorded below; this early design
review alone was not code acceptance evidence.

## Historical run-1 batch preparation

Implementation and native test authoring are complete for the current candidate.
Independent read-only reviews covered the connected producer, metadata admission,
wire changes and compile/API/batch shape. The last correction derives static
visibility from a budgeted owner/declaration index; the reviewer found no further
blocker in that correction. These inspections do not prove compilation or runtime
execution. The final pre-format source identity is retained with batch preparation.

The single batch will format ordinary modules and all changed `include!` fragments
once, refresh advisory mutation selectors by discovery, then freeze source bytes.
Its command script is retained with the evidence. It covers core all-feature and
no-default tests, i686 musl execution, both MCU library graphs, 2.0 fixture production
and portable readback, source-authoring and legacy hosted bytecode suites, the
runtime vertical, runtime unit tests, affected Clippy, cross-target warnings,
supply chain, architecture, diagram rendering/drift, formatting and diff checks.
Metadata validation remains advisory. The generated fixture and diagrams receive
separate output identities; they do not masquerade as pre-run inputs. Independent
steps collect all failures; dependent steps remain explicitly unrun.

The builder checkout is a new detached worktree at the A2 base, separate from the
historical A1/A2 copies. The task target and TMPDIR use the mounted storage volume.
Preflight found 16 CPUs, approximately 30 GiB RAM and 268 GiB free on that volume;
six Cargo jobs balance parallel compilation against memory use. #129 has no
builder workload at this checkpoint. No artifact execution is authorized until
A4, and no board run or A3 commit/push is included.

#129 remediation remains owned by its separate merge agent and candidate checkout.
A3 preserves its unmerged A1/A2 ancestry until integration is explicitly undertaken.

## Historical pre-run typed initializer decision

The independent design review accepted shared TYPE_TABLE-driven construction
and coercion with compiled default callbacks through the same dispatcher. The
unshipped REVERSE_VALUES draft was replaced by explicit aggregate builders
and typed value operations. Recipe bodies are distinguished from lifecycle
actions and have bounded lexical context and activation-local staged results.
Specification 12 §11.5.8 now owns this contract. The producer and admission implementation is present; validation
and A4 execution are still pending. This decision is not execution evidence.

## Historical pre-run implementation checkpoint

New source paths compile aggregate values, typed default/coercion recipes, frame
slots, sequential FB overrides, configuration actions, access aliases and I/O
bindings from declarations. Existing I/O layout helpers are reused. Independent
read-only review corrected global-static grouping, frame EN/ENO defaults and
concrete-instance lookup bypasses before the first batch. Recursive runtime
reference-lifetime and restricted initializer-storage checks are explicit A4
requirements; A3 static admission does not prove dynamic reference contents.

New tests are authored for recipe wire identities, invalid contexts, opcode
boundaries, repeated aggregate evaluation order, bounded repeat expansion, explicit
version selection and authoring without executing a faulting initializer. At that pre-run checkpoint, none
had run. The source corpus and read-only review were prepared; generation,
formatting and all executable evidence remain part of the upcoming batch.

The connected-path review additionally found and corrected missing SINGLE global
typing, direct startup image sizing, lost global CONSTANT flags and extra declared
initialization before explicit locals. New source assertions cover those cases.
Function statics now carry ordinary/after-restart triggers; their eager and lazy
contexts are represented separately. Static records carry their original lexical
name independently of the private backing name. Scoped declaration indexes supply
2.0 FB-call signature lookup; legacy VAR_META lookup order remains unchanged.

The fixture generator and source now exist under the separately named portability
2.0 fixture directory. At that pre-run checkpoint, no binary or disassembly had been generated. Run 4
subsequently generated and verified both saved artifacts; run 3 rendered diagrams
and run 4 checked their drift. The original preparation statement is historical.


## Historical run-1 outcome and corrections

The consolidated batch ran on the builder from 18:55:13 to 18:58:48 UTC on
9 October 2026. See [ledger](a3-evidence/run-1-ledger.tsv),
[frozen source](a3-evidence/run-1-formatted-source-manifest.json),
[commands](a3-evidence/run-1-commands.txt) and
[artifact identities](a3-evidence/artifact-sha256.json).

Twelve required steps failed: eleven compilation-dependent steps and architecture.
Three dependent checks were unrun: saved-fixture parity, rendering and diagram drift.
Formatting preparation/check, source freeze, both feature trees, supply chain and
diff integrity passed. Advisory selector refresh, isolated metadata indexing and
validation passed (1,029 records). Feature trees are dependency evidence only;
both MCU compilation checks failed. No native assertion ran successfully.

Compilation exposed four core root causes: an unbound version value, missing
reference-type imports in two modules, a temporary-budget closure returning a value
where the accounting API intentionally requires unit, and a u32/usize comparison.
Read-only follow-up review found the same comparison issue in four hosted aggregate
bounds, hidden behind the core failure. Corrections now bind/reuse the decoded
version, import reference types explicitly, keep direct-I/O compatibility inside
the budgeted scratch scope, and compare each limit in the collection/operand's
actual integer domain without changing the limit or checked arithmetic.

The architecture failure was `apply_stack_instruction` exceeding 200 lines.
Reference-stack operations were extracted unchanged into a focused helper; no
waiver or raised threshold was added. The first formatting pass also reached the
previously unformatted `io/interface_contract_tests.rs`; that change is formatting
only. Real mutation discovery refreshed the relevant selector locations/digests;
it did not run or reclassify historical mutation results.

All frozen source bytes and raw evidence were preserved before corrections. The
post-run changes have not been formatted, compiled, tested or revalidated. A second
batch requires explicit user authorization; none has started. The three A3 checklist
items stay open, and the 2.0 artifact/diagram outputs remain absent/stale.


Independent correction review found no actionable findings in the seven changed
Rust files relative to the frozen run-1 source. The [review identity](a3-evidence/post-run-1-review.txt)
was recomputed locally. No validation was performed after those corrections.
The proposed run 2 retains the native/core/32-bit/MCU/host/lint/architecture/diagram
commands, refreshes metadata bindings, and reuses run 1's supply-chain and no-dev
feature-tree evidence because dependency declarations and lockfile are unchanged.
It is prepared only, not authorized or started.


## Continued A3 authorization

The user approved the additional batch and instructed that repeated approval
questions are unnecessary. Run 2 is authorized, with subsequent A3 correction,
independent review and necessary consolidated validation covered by that direction.
Each failure remains recorded; no unchanged-command retries, gate bypasses, commits,
pushes or A4 work are implied. Builder scheduling is coordinated with #129.


## Historical run-2 outcome

Run 2 passed 221 core tests including the borrow compile-fail doctest, 70 portable
loader tests, 139 i686 musl tests, both F401/C6 core checks and architecture. It
paused explicitly while #129 held the hosted test slot, then resumed at six jobs.
Hosted compilation failed at a private reexport hop in the compiler facade and an
unused FrameId import. E0614 partial-access diagnostics accompany the unresolved
private enum import; the correctly borrowed match is preserved. Diagram rendering
failed on literal multiline quoted component labels. Metadata validation, format
and diff checks passed. See [ledger](a3-evidence/run-2-ledger.tsv) and
[frozen source](a3-evidence/run-2-formatted-source-manifest.json).

The prepared correction exposes only the intended crate-level model/type helpers,
keeps implementation modules and other lowering helpers private, removes the unused
import, and uses PlantUML escaped newlines in both diagrams. Core source is unchanged;
its passing evidence is retained. The next consolidated batch targets hosted
fixture/tests/unit suites, lint/cross warnings, architecture/diagrams, formatting
and metadata. Standing user authorization covers this reviewed follow-up; no
unmodified failing-command retries are being performed.


## Historical run-3 outcome and semantic corrections

Run 3 passed 219 legacy hosted integration tests, 3,862 runtime unit assertions,
affected Clippy, Linux/Windows warning-deny checks, architecture, canonical diagram
rendering/drift, formatting and metadata validation. One new unit fixture failed
source admission. In the new authoring suite 7 passed and 9 failed; saved-fixture
parity remained unrun because generation failed. See [ledger](a3-evidence/run-3-ledger.tsv)
and [frozen source](a3-evidence/run-3-formatted-source-manifest.json). The successful
diagram outputs were copied back; failed run-2 renders remain historical archives.

Source inspection and independent review identified the owning causes:

- Local VAR_META must reuse the existing `@local` identity produced by LocalScope.
  The producer now takes that name from PendingLocalVarMeta; initializer results
  remain a distinct reference location with their own names.
- Hosted I/O normalizes STRING's tag while retaining its Bytes(n) extent. The 2.0
  encoder now selects the already declared bounded STRING type by that extent.
  Core checks and the 13/28-byte image assertions, including 12/27 rejection,
  remain unchanged. Added coverage includes aliases, arrays and struct leaves.
- Mutable scalar and integer-division-by-zero fixtures were rejected by the
  existing source checker. Accepted constant LREAL expressions now exercise
  runtime initializer faults; both producers retain E202 for mutable scalar
  initializers. The disabled-call fixture declares its EN parameter explicitly.
- STEP is a reserved keyword. The shared method is now Advance. Its changing-input
  local uses `REF(history[delta])` against persistent instance storage; delta rotates
  through 1, 2, 3. IEC §6.4.4.10.2–3 was checked in the local standard. No source
  restriction or lifetime rule was relaxed. Spec 12 §11.5.10 and spec 34 v0.11
  record this clarification, with A4 execution still required.

A further native regression checks that both hosted module and byte APIs reject
valid 2.0 artifacts while the original legacy program continues correctly. This
was reviewed as a draft during the queue wait and is part of the next candidate,
not run-3 evidence. All corrections are untested pending the reviewed follow-up
batch. No core files changed, so the passing run-2 core/MCU evidence is retained.

The complete eight-file run-3 correction has independent review without actionable
findings; [review identity](a3-evidence/post-run-3-review.txt) was recomputed locally.
Run 4 reuses unchanged core/MCU proof from run 2 and canonical rendering from run 3;
it checks diagram drift and runs the affected hosted/native/lint/metadata gates.


## Verified A3 closeout

Run 4 completed on the builder at 20:18:10 UTC, 9 October 2026: **13 required and
3 advisory steps passed**, including 20 source-authoring, 219 existing hosted
integration and 3,863 runtime unit tests. Clippy, Linux/Windows compilation with
warnings denied, architecture, diagram drift, formatting/diff integrity and
metadata validation passed. The complete command list and ledger are retained:
[run 4](a3-evidence/run-4-ledger.tsv), [commands](a3-evidence/run-4-commands.txt).

Its [231-record frozen manifest](a3-evidence/run-4-formatted-source-manifest.json)
has SHA-256 `12b4f83aa462c238b170b7a00ee9a8d1bb9ac8fc5defef0fcb509e62cd69f9ff`.
All copied local bytes matched before closeout edits. The 46 modified core files
still match run 2, which passed 221 core tests, 70 portable-loader tests, 139 i686
musl tests and both MCU library checks. Canonical diagrams were rendered in run 3
and passed drift checking in run 4. Cargo manifests/lockfile are unchanged from
A3's base, retaining run 1 dependency-audit and no-dev feature-tree evidence.

The separately generated [STBC 2.0 artifact](../../../crates/trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc)
is 37,296 bytes, SHA-256
`47d5cc75e01bd81e5479d58930ff2330bce6b4a66ee4c23e2509d2d0e2f24afa`.
Its [disassembly](../../../crates/trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.disassembly.txt)
has SHA-256 `96b450d9748beba42b82c31b0f708485e7c34e97822f21d9324116e27df08840`.
The generator and saved-fixture test independently decode/validate it and compare
fresh emission byte for byte. The legacy OSCAT artifact is unchanged. The
hosted rejection regression proves both application entry points preserve the
working legacy program when offered valid 2.0 bytecode.

Existing behavior tests remain in place for the shared-engine migration. They do
not introduce or require another execution engine. Version-specific 1.x reader
retirement follows the P/Q window; P/Q product versions must be named before P
ships, not inferred from this development scope's workspace version.

Scope limits: no 2.0 initializer or POU has run in a source-free engine; no firmware
link, board run, timing/peak-memory qualification, A3 release guard, push or native
macOS/Windows execution was performed. Cross-target compilation is not execution.
A4 remains open and unstarted. The next implementation scope needs its own explicit
authorization; the current continuation authorization covers A3 validation only.

Post-validation edits are closeout documentation and retained evidence only. They
update the checklist/requirement ledger and spec 12's status sentence without
changing the validated format or Rust implementation. No extra validation was run
for those bookkeeping edits.


## Independent closeout reconciliation

A separate read-only audit recomputed native counts, all 140 then-retained artifact
hashes, 24 run-4 original/copy identities, the frozen manifest and generated fixture
hashes. It confirmed the 46 core files and Cargo manifests/lock remain unchanged
for the reused proof, with no post-run-4 Rust edits. The only finding was ambiguous
historical-status wording in this note; those headings/statements are now explicitly
historical. Fixture files are saved, untracked and not ignored, pending a separately
authorized commit. This audit ran no compiler, test or validator.

## Historical post-run-4 correction preparation

The user requested every finding and recommendation fixed. This includes staging-only
initializer writes across control-flow merges and native writable arguments;
reachable stdlib templates; canonical-context recipe sharing; native authoring
coverage for edges, retain, partial/configuration access and classes; strict path
prefixes; structured diagnostics; stable disassembly; grouped producer state and
prebuilt action indexes. The former run-4 closeout does not certify these changes.

A3 continuation authorization remains in force for complete fixes, independent
review and a consolidated follow-up batch. Required proof includes core all-features,
no-default and i686 execution, F401/C6 core checks, source authoring/fixture replay,
the existing hosted integration and full unit suites, Clippy, cross-target warnings,
architecture, rendering/drift and formatting. Dependency audit may reuse unchanged
manifest/lockfile evidence. Advisory discovery and provenance refresh are planned
inside that batch. No tests or formatter had run at that preparation checkpoint; R5/R6 results follow below.

## Historical run-5 outcome and final fixture corrections

Run 5 completed without retries. All required checks passed except source authoring
(28 passed, 3 failed). Passing evidence: 224 core tests, 70 portable-loader tests,
142 i686 musl tests, both MCU core checks, 219 hosted integrations, all 3,863 runtime
unit tests, Clippy, Linux/Windows warning checks, architecture, rendering/drift,
formatting and diff checks. Advisory metadata validated all 1,029 records.
The generated fixture is 6,164 bytes, down from the historical 37,296-byte artifact,
while adding an edge input and retained activation state. This is artifact size,
not a measured runtime RAM or board performance claim.

The failed fixtures are corrected without changing production Rust or HIR rules:
- The negative frame-type test now explicitly declares BOOL instead of relying on
  an unrelated unused stdlib template to introduce it.
- SIZEOF stores its DINT result in a DINT fixture variable, preserving the new
  expression-only referenced-type discovery assertion.
- The source test covers an admitted partial alias and whole-WORD configuration
  assignment. A separate native lowered-record test covers partial configuration,
  round-trip validation and producer/consumer out-of-range rejection. Source
  VAR_CONFIG does not accept that partial target form; its E202 rule is unchanged.

Run 6 was prepared as a focused consolidated correction batch: all 31 source-authoring tests,
the four authoring-boundary unit tests, affected Clippy, cross-target warnings,
format/diff checks and advisory metadata. Unchanged production/core/MCU/full-host
and rendered evidence is retained from run 5. Standing A3 continuation approval
covers this reviewed correction; no automatic unchanged-command retry, commit,
push or A4 work is performed here.

## Verified review-correction closeout (runs 5 and 6)

All eight required and two advisory run-6 steps pass. The 31 source-authoring tests
and four boundary unit tests include all review regressions. Run 5 supplies current
production/core/i686/MCU checks, full hosted suites and canonical diagram rendering.
Only two Rust test files changed after run 5; the newly added partial-configuration
assertions executed in run 6. No production Rust or assertion was weakened.

Run-6 frozen 307-record manifest SHA-256:
`2faf6f16710ae55a60e227c52b9c2c5e965be541be335d88bdea14a403700717`.
Run-5 production manifest SHA-256:
`21c8b6ab9fbfb6f7e67490882342ab6730ff2392ad2a094a1a47de1bc0ef062e`.
The 6,164-byte fixture SHA-256 is
`0a09a7190f26bbe3481ca7ae5b7f16a130ae4dcb2ee21937b456dd8581a87e5a`.

The evidence index includes its README, commands, ledgers, raw outputs and current
review-input manifests/archives. Historical pre-format review digests are labeled
as historical; unavailable bytes are not reconstructed or claimed verified.
Closeout updates the specification revision, checklist, execution note and READMEs
after the freeze without changing Rust or generated fixture bytes. A4, board work,
A3 commit/push and release qualification remain outside this closeout. A separate
agent is integrating A1/A2 onto merged main; it must preserve these scope records.

## Local commit after verified closeout

The owner subsequently authorized committing A3 and starting A4 separately.
Canonical workflow files are recorded in `19fbdad18`, byte-identical to the reviewed
A1 integration candidate's rules. Implementation/specification/fixture commit is
`7e6938a75`; this following evidence commit completes the A3 series. No new test
execution, push, tag or release is implied. Earlier uncommitted-state references
describe their historical validation checkpoints.
