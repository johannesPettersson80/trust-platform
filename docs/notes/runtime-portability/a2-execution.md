> **Current outcome:** Expanded A2 implementation and allocated validation are complete through
> the passing union of runs 6–8. Failed runs remain retained and are not relabeled as passing.
> No A2 commit, push, A3, release or hardware qualification was performed.

# A2 shared loader and validator execution

A2 was authorized for implementation, independent review and a consolidated builder batch;
subsequent explicit user instructions authorized the corrective runs recorded below.
No commits, push, A3, firmware or hardware execution is authorized.
Base: `78d4e641f589734a55b9b3b25981a722d8396e62` on `feat/runtime-portability-a2`.

## Original relocation implementation and review

Core owns the complete existing STBC 1.x container, decoder, validator, byte serializer and
metadata materialization. Host compiler lowering stays hosted. The host preserves public
container fields and runtime constructors and delegates via a borrowed core view. Portable
tree collections support validator lookup; neither raw container nor view is executable state.
The existing locked crc32fast 1.5.0 dependency gains a no_std core edge and explicit host std opt-ins.

Native format corpora run through both the portable API and the hosted compatibility API.
New assertions cover compiler-free metadata (not execution), legacy layouts, semantic rejection,
reader overflow, section/POU extent overflow and signed jump overflow/boundary targets.
Existing runtime validation-before-application remains unchanged.

Independent read-only review found and resolved stale runnable mutation source paths,
32-bit POU extent overflow, signed relative-jump overflow and relocated-crate mutation cleanup.
Reviewer concluded no remaining source blockers. Review hash, sorted path/NUL/content or
`<deleted>`/NUL across 75 changed/untracked/deleted paths:
`0467a988ef55bb8865ea97fbe59dae85d94e9ce57f176228bb5abd4bf2199977`.
No tests were run by the reviewer. Formatting and derived source-digest refresh are recorded
separately in the batch. The generic mutation runner still cleans host outputs but writes fresh
source mtimes; no stale-code defect was demonstrated, and no mutation adequacy claim is made.

## Original planned batch (historical)

Rust 1.95.0, edition 2021, resolver 2, locked Cargo inputs, warnings denied, incremental off,
two build jobs and debug info off. Builder uses its shared target lease; native suites run
serially. No retry after any failed command. Independent commands continue to collect failures;
diagrams depend on a passing architecture result.

1. Format once; check formatting; refresh mutation source digests and freeze source hashes.
2. Isolated no-default core checks and feature trees for `thumbv7em-none-eabihf` and
   `riscv32imac-unknown-none-elf`.
3. All-feature core tests; separate no-default portable container/section/resource-bound/loader
   integration tests (each nonempty; helper-only modules are not acceptance evidence).
4. Hosted container, metadata, decode bounds, sections, image, validation, optional sections,
   encoder, roundtrip, verification-case, VM/differential and behavior-lock tests; vars_access
   constructor compatibility and the api_smoke/debug_control/complete_program/runtime_reliability
   vertical. Hosted encoder unit tests retain `from_runtime*` API assertions.
5. Mutation-path tooling unit tests (runner adapter tests, not actual mutation testing).
6. Workspace Clippy, runtime host/Windows GNU warning checks, supply chain, architecture full-map.
7. Canonical diagram render and drift check, diff integrity; advisory metadata validation.

Raw batch commands, ledger, formatted source manifest and logs are retained in the A2 artifact
directory and copied back from the builder. Batch 1 is complete with failures recorded below. Historical
A1 evidence does not establish A2 acceptance. No native Windows/macOS execution, release guard,
source-free runtime construction, STBC 2.0 or physical board result is claimed.

## Batch 1 result and pending correction

The single authorized batch finished: 15 required steps passed, 2 failed, and 2 were skipped
because their architecture prerequisite failed. The advisory metadata validator also failed.
No command was retried.

- 173 all-feature core native tests passed; 32 no-default portable-loader tests passed.
- 171 hosted integration tests and 5 hosted encoder/constructor unit tests passed.
- 22 mutation-runner adapter tests passed (no actual mutation run).
- F401 and C6 isolated core checks and feature graphs passed; host and Windows GNU warnings,
  formatting, supply chain and diff-integrity checks passed.
- Clippy rejected redundant outer `allow(dead_code)` attributes already present inside the shared
  test helper. The three outer attributes have now been removed; assertions are unchanged.
- Architecture rejected an unnecessary module-size exemption (4620 lines, below the 5000-line
  threshold). The new exemption has now been removed. No architecture rule was weakened.
- Diagram rendering and drift checks were not run because architecture failed.
- Advisory metadata reported 16 diagnostics: eight source-provenance bindings affected by the
  four relocated invariant source references, plus inventory/nightly/fuzz records untouched by A2.
  The four case source bindings and eight catalog digests are now updated using the existing execution-contract digest function; case bodies/IDs and historical evidence were preserved. This correction is not yet validated. Untouched inventory/nightly/fuzz drift remains advisory; neither mutation nor complete metadata validity is claimed.

The frozen formatted source manifest has SHA-256
`ea8d89718b618ffcc055a0f7c6df46de17cfd4451d540df366b00672ce31a3d1`.
The independent reviewer confirmed all 76 records matched. Raw ledger and all logs are retained
under `projects/.artifacts/runtime-portability-a2/run-1/` (local) and the matching builder cache.
The two corrections above are prepared after the batch, not validated. A2 remains unverified
until the failed and skipped mandatory gates pass with explicit supplemental-run authorization.
No runtime implementation or native assertion changed after its passing batch-1 execution. Four case-file source fingerprints and eight catalog fingerprints changed as described above; their owning native case suites and provenance checks belong in the supplemental batch.

## Supplemental batch authorization

After batch 1 completed and all findings were collected, the user explicitly authorized one
supplemental A2 batch. The nine correction files were independently reviewed with identity
`5e70f3db7b64ae9d7caaf462de3d286179c59a68dfbdbeb8ef2bd3ee3ecea463`
(sorted path/NUL/content/NUL). They remove redundant test attributes and the unnecessary
architecture exception, and refresh case/catalog source fingerprints without changing cases.

Commands: formatting check; native `bytecode_verification_cases`, `vm_resource_limit_cases`,
`phase11_seam_contract`; mutation/provenance unit tests; workspace Clippy; architecture full-map;
canonical diagrams and drift; diff integrity; advisory metadata. Rust/features/environment remain
batch 1's tuple. No broad native suite is repeated without an affected input. No third batch,
commits, push or A3 is authorized.

## Supplemental batch result

Run 2 completed without retries. Clippy, architecture full-map, canonical diagrams/drift,
formatting and diff-integrity passed. All 40 provenance/tooling unit tests passed.
The affected native command passed 24 top-level tests and failed three trace runners at their
strict case-file checksum checks. Those three runners did not execute their contained scenarios.
The metadata correction had missed three static checksum pins in two native test files.
Those pins are now updated to the reviewed case-file bytes; integrity checks are unchanged.
No runtime code, case body or expected behavior changed. The original run-2 logs and source
snapshot are retained alongside run 1.

The advisory metadata result is now limited to eight findings in untouched gate inventory,
nightly suite and fuzz-workflow records. No complete registry-validity claim is made.

Remaining mandatory evidence is a narrowly scoped, separately authorized batch of the two
native suites owning the corrected pins (`phase11_seam_contract`, `vm_resource_limit_cases`),
formatting and diff-integrity checks. No run 3 is automatically authorized. A2 is not yet closed.

Future case provenance maintenance must account for the complete binding chain: invariant
execution digest, case-file fingerprint, catalog references and native static pins. Preserve
historical evidence rather than rebinding old results to new case files.

## Focused final batch authorization

The user explicitly authorized one focused batch after review of the three static pin updates.
Reviewed two-file identity: `14f967ca14f6d1b5004f7aec47afdbc98b19f85392783b549b5c8c9bb4cd8df9`.
Commands: formatting check; the two affected native suites (25 top-level tests); diff integrity.
No further retry, commit, push or A3 work is authorized.

## Final closeout

Run 3 passed all four steps, including 25 native tests executing the previously blocked trace
runners. The final 84-record source/deletion manifest matched both local and builder copies before
this documentation closeout; manifest SHA-256:
`516fbf6f84252da258fb63db84c6efd0432c93f10b005744395fe3756a5bf684`.

See [portable evidence](a2-evidence/README.md), [independent review](a2-evidence/review-record.md),
and the three unmodified ledgers. Runtime implementation/native assertions did not change after
run 1. Later source changes are reviewed redundant-attribute removal, removal of an unnecessary
policy exemption, metadata fingerprints and static test pins. Required diagrams were rendered
and drift-checked in run 2. Closeout documentation/evidence copies are subsequent bookkeeping,
not a newly tested runtime snapshot.

A2 moves the byte-oriented boundary only. Compiler-free runtime state construction, STBC 2.0,
shared execution and physical boards remain later scopes. Native Windows/macOS execution and the
exact-SHA release guard have not been performed for A2. Full registry metadata remains advisory
with eight recorded findings in untouched records. A1/#129 integration remains a separate task;
rebasing A2 must preserve this evidence and obtain the required validation for changed source.

## External review corrections (not yet validated)

Retained native logs now use `.txt` names so ordinary commits include them; their bytes and
hashes are unchanged. The run-2 frozen manifest is now retained alongside runs 1 and 3.
Three core test files lost a duplicate attribute after run 1; Clippy compiled them in run 2,
but those test bodies were not re-executed after run 1. No test-assertion change is implied.
Historical 75-path pre-format review identity is historical, not the current source identity.

`cargo fmt` did not traverse include! fragments. Therefore the earlier formatting PASS rows
prove only the files that command visited. The next authorized batch must explicitly format and
check every touched fragment with the repository config. Converting fragments to modules is a
named follow-up. No prior log is rewritten to claim the missing formatter coverage.

32-bit overflow rejection is currently supported by checked code and target compilation,
not by executing those overflow branches. A future 32-bit native lane must close that distinction.
Feature-tree logs include dev-dependency std edges; the isolated MCU cargo checks are the actual
no_std build evidence. Subsequent trees must use `-e features,no-dev`.

Case/test canonical ownership: core owns the pure container/section/resource-bound corpus;
host copies are compatibility assertions and must receive the same fixture/case updates, using
host imports. They exercise a different API boundary and do not replace core no_std tests.

## Expanded external-review correction — implementation in progress

The user explicitly requested every finding and recommendation, including the previously
suggested deferrals. The earlier run-3 snapshot is therefore not the current acceptance state.
RTP-A2-R01 through R06 track this expansion; no new validation run is authorized yet.

| External finding | Correction and evidence still required |
| --- | --- |
| Ignored native logs and missing run-2 manifest | Retained logs renamed `.txt`, bytes preserved; manifest and artifact index updated. Check tracked eligibility and hashes in the batch. |
| Hidden include fragments | Portable bytecode and hosted wrapper use real modules; normal rustfmt traverses them. Explicit formatting remains required for any touched host encoder fragment. |
| Validator memory amplification | One decoded list, block-entry states and fallible budgeted storage; whole-candidate work budget covers metadata, nested traversal and native argument matching. Deep straight-line, branch, loop and exact-limit regressions authored; isolated Linux RSS assertion distinguishes native memory evidence from logical accounting. |
| 32-bit arithmetic execution | Add an i686 native portable-core lane to the correction batch; MCU checks remain compilation only. |
| Header/table aliasing and alignment | Reject misaligned declared headers and section payloads overlapping header/table. Native malformed-layout assertions authored. |
| Jump/serializer truncation | Signed code-position conversion, checked alignment/counts and bounded writer; exact-limit and overflow assertions authored. |
| Duplicated wrapper | Owned From conversions, public borrowed view and section lifetime; zero-copy conversion/lifetime assertions authored. |
| Copied tests/helper binaries | Core owns the pure format corpus; host copies lock compatibility. Helpers live under tests/common and no longer create zero-test binaries. |
| Shared instructions/context | Decoder is shared by all passes; unsupported/dead parsing paths and unused parameters removed. Legacy CALL still rejects after structural checks, preserving first-error precedence. |
| Structured reasons/public docs | Named rejection reasons preserve public error variants/codes and existing text; portable records and API documented without missing-doc blanket allowances. |
| Validated construction | Immutable token requires complete validation and is required by host VmModule materialization; compile-fail borrow assertion authored. Profile admission remains A4. |
| Documentation/evidence hygiene | Changelog categories consolidated, requirement rows individualized, arithmetic rules placed in spec 12 §4/§7, original fmt/feature-tree limitations disclosed. Future graphs use features,no-dev. |

The existing compiler producer still validates its raw result; host materialization validates
again to obtain a borrowed token because callers can mutate that raw result. No cached boolean
is substituted for validation. Removing that duplicate cost requires a later ownership/API
change, not trusting a mutable container.

The active stream-bypass mutant now targets `validate_pou_index`, which owns shared decoding
and all instruction passes. This is deliberately broader than the historical removed
`validate_instruction_stream` mutant: it also bypasses POU metadata checks. Case associations
still describe opcode/jump risks, not execution claims. Old mutation reports remain historical;
no new mutation adequacy measurement is claimed. Digests/selectors must be refreshed after
formatting inside the authorized batch.

Planned correction batch: format once (real modules plus touched host include fragments),
refresh affected source bindings, freeze reviewed-source manifest; all core tests/doctests;
portable no-default tests on x86_64 and i686; F401/C6 checks and no-dev feature graphs; hosted
bytecode/VM unit and integration corpora plus runtime vertical; affected mutation/provenance
adapter tests; Clippy, cross-target warnings, architecture and diagrams, formatting/diff,
evidence hash/ignore checks and advisory metadata. No automatic retries, commit, push or A3.
Read-only prerequisite inspection confirms Rust 1.95 rust-lld and kernel IA32 support. Use i686-unknown-linux-musl with its self-contained linker; target installation is a batch prerequisite. No 32-bit execution has happened.

The external review's advisory maintenance is also included: five missing gate records,
deduplicated nightly commands/bindings, refreshed unchanged-CI fuzz source identity, and
portable native-corpus catalog records generated with the existing discovery-ID helper.
VM_SEAM_VALID_001 now links those core assertions as well as hosted compatibility assertions;
this lifecycle routing does not alter case bodies or claim new execution evidence.
All these edits require the advisory validator in the forthcoming batch.

The refactored mutation shard is explicitly `planned`, not `measured`. Historical pilot
bytes are pinned independently, excluded from active result counts and retained as provenance.
The report generator/schema and adversarial tests distinguish four active measured shards from
two planned shards. This is an evidence correction, not a new mutation campaign.

The full portable bytecode namespace now exceeds the architecture policy's 5,000-line
aggregate note threshold after documentation and analysis hardening. A module owner/split note
records the existing decode/encode/format/metadata/validation child responsibilities. Thresholds
and individual-file limits remain unchanged; host VM unit tests moved into a real `tests` module.

The prepared batch uses a temporary Git index only for metadata tools that require candidate
files to be registered. The real index and commit history remain unchanged; this is explicitly
uncommitted candidate evidence. Formatting and actual cargo-mutants selector discovery precede
source freezing. Historical reports and their hashes are never rewritten to simulate a run.

## Correction review checkpoint

Independent read-only review reports no remaining source blocker across 116 changed,
untracked or deleted source/config/spec/diagram paths. Identity:
`30ec65ad4ae1827fb9cab98e561e737cf711260fe2ff24bbfd6765d482f98008`.
It uses sorted path/NUL/content-or-`<deleted>`/NUL and excludes these evolving notes/checklist.
The review found and corrected structural-error precedence, variable diagnostic copies,
metadata work accounting and historical mutation-result reuse. It is not build/test evidence.

One correction batch is prepared at
`/home/johannes/projects/.artifacts/runtime-portability-a2/run-4-on-builder.sh`, with its
`prepare-evidence-4.py`, `prepare-metadata-index-4.py` and `check-retained-evidence-4.py` helpers.
After explicit authorization, verify the builder's historical copy against its retained
run-3 manifest, preserve any independent edits, sync only this worktree, manually bootstrap
canonical rules and verify parity, then run the script once through the shared target lease.
The script refuses a second launch through its STARTED marker and retains every failed or
unrun step. It has not been transferred or launched. Last disk inspection: 63 GB available,
above the focused-batch floor but below the separate release guard's 80 GiB floor.

No A2 commit, push, A3, new mutation campaign or physical hardware execution is authorized.

## Approved correction batch 4 — failed, no automatic rerun

The user explicitly approved the consolidated correction batch. It ran once on trust-builder
through the target lease, at detached base `78d4e641f`, Rust 1.95.0. Canonical rules matched;
all 154 candidate records matched before launch. The prior builder state matched run 3 exactly
and overwritten files were retained in a pre-run backup. No real index or commit changed.

Frozen formatted source manifest SHA-256:
`9e8542a1b08d90f95141c6f2cbea9de08eddc4c6a89c89f92b5a39fed7c64ad6`.
All records matched after copying source back locally, before the corrections below. The full
frozen source archive and raw logs remain in the workspace artifact directory; logs, commands,
manifest, environment and ledger are retained here with byte-exact hashes.

- Passed: F401/C6 no-default compilation and no-dev feature graphs; 193 core native tests plus
  one compile-fail doctest; 49 no-default portable tests; 197 hosted integration tests including
  the runtime vertical; workspace Clippy; host/Windows-GNU warning checks; supply chain;
  formatting/diff integrity; all 29 historical retained artifact hashes/ignore checks.
- Runtime unit suite: 3,857 passed and one failed of 3,858. The invalid-jump lowering fixture
  injected corruption before VM materialization; the new admission token correctly rejects it
  before the lowerer is reached. The assertion still needs to run against a corrupted test-only
  VM assembled from a valid admitted artifact.
- Native i686-musl: target installation succeeded, but `--lib` test compilation failed because
  the no-default error test lacked `alloc::string::ToString`. No 32-bit execution is claimed.
- Tooling: 82 tests ran, two failed and report-suite setup errored. Remaining failures concern
  pre-existing retained mutation records for value conversion/HIR/parser/retain: ancestry,
  source paths/digests and runner provenance disagree. Old artifacts are preserved; repinning
  them as current measurements would be false evidence.
- Architecture failed because the function-size owner note still named `from_bytecode`, now
  a small test wrapper, rather than the renamed `from_validated` materializer. Diagram render
  and drift checks were correctly left unrun. The bytecode aggregate owner/split note passed.
- Advisory metadata passed: 1,029 records. This closes the reported inventory/nightly/fuzz
  diagnostics for the tested snapshot; it does not make the separate mutation-report tests pass.

Total: 18 required PASS, 4 required FAIL, 2 required UNRUN, 1 advisory PASS. The batch result is
failed. No automatic rerun, commit, push, A3, release or device execution followed.

After preserving the frozen source, three corrections were prepared but not executed: the
no-default test imports ToString; the lowering fixture corrupts its private VM after valid
materialization without weakening its invalid-jump assertion; the existing architecture owner
note follows the materializer rename. The historical mutation-record failures remain under
source review. These edits are not covered by run 4.

Independent review of the three prepared corrections is clear, identity
`837ab3532bebaa6248cacc3d098f2b9f0fbabbe284ffdc803cb1feac76643fa6`.
No checks were run on these corrections. Read-only comparison confirms the four failing
historical focused-mutation artifacts, their relevant source files and runner are unchanged
from the A2 base. Value conversion has a runner-byte mismatch; HIR/parser artifact commits
are nonancestral; retain additionally references the old restart.rs location. The strict
checks are correctly rejecting current-measurement claims. Further mutation-record maintenance
must distinguish dated historical evidence from active planned selectors, or obtain a new
properly authorized campaign; never rewrite old artifact identities. That broader historical
record disposition is unresolved. There is no authorization for another A2 or #129 batch.

Separate storage discussion: live inspection confirms a Hetzner vServer with a 305.2 GiB
virtual disk. A 500 GB Cloud Volume was recommended; user was given Console creation steps.
No volume purchase, filesystem change or cache migration was performed. #129 remains open;
its local reviewed remediation still awaits disk capacity and another authorized guard attempt.

## Final prepared run-4 corrections

The user requested continued development while the merge agent handles #129. The inherited
focused-mutation provenance failures now have the reviewed disposition: all six active shards
are planned, with zero active measured results. The four dated focused records and earlier
bytecode pilot remain byte-pinned historical inputs; no artifact contents were edited. New
qualification paths are reserved separately so a future authorized campaign cannot overwrite
those dated records. The current retain source binding is refreshed; actual selector discovery
is a step of the next authorized batch, not a fabricated mutation run.

The measured-artifact validator still checks source equality, ancestry, contract identity and
outcomes. Tests explicitly use synthetic measured fixtures to exercise those checks; the live
report no longer assumes the historical files qualify current selectors.

Historical independent review reported no remaining source blocker, 118 paths, identity
`a2f9a43961bcb9ee828c9c04c482950c20713e3954f77a7372f4d7e2bf856c50`.
Same path/NUL/content-or-`<deleted>`/NUL method; excludes shared guard files and evolving
notes/evidence/checkpoint. No new test, formatter or validator executed after run 4.

Builder capacity is now 16 vCPU / 32 GB RAM with the 500 GB volume. Verified cold-tool/sccache
migration left 87 GiB free on root at that checkpoint. Future A2 run 5 is planned with 6 Cargo
jobs, Rust 1.95.0 and the same edition/features, after #129's active guard releases the builder.
The machine change is recorded; no timing speedup or cross-machine performance parity is claimed.
The user has been asked to authorize this one focused run; do not launch without a reply.


## Additional review corrections before run 5

The external review could not reproduce the historical 118-path identity from its stated
exclusions. It is retained as an unconfirmed historical claim, not current review evidence.
The next review freeze will enumerate every included path and its digest explicitly.

A2 remains unverified. The original RTP-A2-01 through RTP-A2-03 boxes are reopened.
The LOAD_NULL width table also changes hosted primary-instance-owner inference: scanning now
continues past a NULL literal. The two-instance native regression and changelog record this
execution correction; it is no longer described as relocation-only behavior.

Validator context now builds reusable sorted POU/name/reference indexes, validates local ranges
as intervals, and uses charged logarithmic lookups. Compact instructions derive their next PC
and jump target; conservative passes share stack effects. New native tests cover sorting,
lookup ties, exhaustion and maximum instruction/reference scale. No tests, formatter, validator,
build or new mutation measurement has run for these corrections. Run 5 must include the
changed core and hosted corpus, 32-bit execution, both MCU graphs, cross-target warnings,
supply chain, architecture and rendered diagrams; copy generated diagram outputs back.


The explicit pre-run-5 source-review manifest lists 130 file/deletion records, with an
enumerated exclusion list for evolving checkpoints and evidence. Its JSON SHA-256 is
`98670d41d8846c0200d4050933c24439d632779667cbb75691d5035f7b6992b9`. Independent incremental source review subsequently confirmed every record and all exclusions.
The source has not been formatted or validated after run 4; run-5 formatting and real
mutation-selector discovery will produce a separate executed-source manifest. They must
not be described as the same byte identity. New test files and all Rust edits are uncommitted.


Final read-only review found no new source concern in prefix-work accounting, lookup tie
handling, compound-range precedence, diagnostic identity or typed internal errors. The reviewer
independently recomputed all 130 manifest records and the JSON digest above; every dirty path
was present in the records or exclusion list, and canonical agent bootstrap parity held.
This was an incremental review using earlier source reviews, not a fresh audit of all 130
files; shared guard files received parity verification only. No build, test, validator or
formatter ran. Maximum-scale admission and memory results remain unverified.

Implementation and source review are complete for the latest findings. Run 5 is prepared but
not authorized or launched. It includes core/portable and 32-bit tests, hosted unit/integration
and runtime vertical suites, both MCU checks/no-dev feature graphs, mutation/provenance tooling,
Clippy, cross-target warnings, supply chain, architecture, diagrams/drift, formatting and retained
evidence integrity. There is no automatic rerun, A2 commit/push or A3 authorization.


## Run-5 authorization

The user answered "yes" to the explicit request for one consolidated A2 run-5 batch,
including the new regressions, platform checks and diagrams. This authorizes that batch
only; no automatic retries, commits, push or A3. Canonical bootstrap parity and the
preserved builder run-4 source were checked before syncing the reviewed candidate.


## Run 5 stopped at the freeze prerequisite

Run 5 executed once with explicit user authorization on 9 October 2026. Canonical source:
`/home/johannes/projects/trust-platform`; A2 local and remote destination:
`/home/johannes/projects/trust-platform-portability-a2`; local branch `feat/runtime-portability-a2`,
builder detached at the same HEAD `78d4e641f589734a55b9b3b25981a722d8396e62`. Canonical AGENTS/full skills and all
130 reviewed source records matched before execution. Builder: scena-rust-builder,
16 CPUs / 32 GB RAM, about 80 GiB free; planned six Cargo jobs, Rust 1.95.0.

Formatting, explicit fragment formatting and format-check passed. `freeze-source` failed:
the batch preparation helper discarded the recorded selector name, leaving three matching
BinaryOperator replacements in `Parser<'t, 'src>::recover_top_level_until`. The helper writes
the manifest/program only after all selections; neither a successful source freeze nor a
mutation run occurred. This is a batch-orchestration defect, not a runtime test failure.

The raw ledger has 3 PASS and 1 FAIL. The companion `run-5-complete-ledger.tsv` explicitly
records the 19 remaining required steps and one advisory step as UNRUN after that prerequisite.
No compilation, native tests, MCU check, Clippy, supply-chain or architecture gate started.
No diagrams were rendered. The original logs and the 196-record stopped-source snapshot are
preserved; formatted source was copied back. That snapshot is not a successful freeze or proof.

Prepared correction, unexecuted: preserve exact expression selector names, allowing only
unique whole-function substitutions to refresh their moved source position. Separate advisory
mutation maintenance from source snapshotting so it cannot block native validation. A synthetic
three-candidate regression pins exact-site selection; no assertion claims an actual mutation run.
Run-6 scripts are prepared outside the repository, but no further run is authorized. No commits,
push or A3. Independent read-only review found no blocker in this preparation correction.


Preparation-correction review identities (no execution):
- `refresh-mutation-selectors-6.py`: `84e05c825ce7c5d62bd7c3593c8ddc88df0180ea8119fbc7df558ceb8946ed82`.
- `freeze-source-6.py`: `bb9ab00202066eab25138735c98e5162cbc0b49d1f9acb02dfd1b9d9f95f068f`.
- `run-6-on-builder.sh`: `d21f7b06998eb479352cb4566611e9e45795759aaebcd4dd0f0df37653d64a4d`.
- `scripts/verification/bytecode_validator_mutation_tests.py`: `d5ae8787eabded860d6d2874087601af129ccf598c1e11c21e6262d6ffbcf153`.

The reviewer confirmed exact expression-site selection, unique whole-function matching,
independent source freeze, and no embedded retry. At that historical checkpoint, run 6 was unauthorized and unrun. The later user instruction
and executed results supersede that state in the final closeout below.


## Final A2 correction closeout — runs 6–8

The user explicitly superseded repeated retry-approval prompts with “please dont stop and ask,
just do it. and check the merge job”. Runs 6–8 proceeded under that instruction; they are
separate retained runs, not relabeled as one successful first batch. A2 commits, push and A3
remain outside scope. No A2 commit or push occurred.

| Evidence | Result and limits |
|---|---|
| Run 6 | 15 required PASS, 6 required FAIL, 2 advisory PASS. Core all-features 204 passed including the compile-fail doctest; no-default x86_64 53 passed; i686 musl 167 passed. F401/C6 checks and no-dev feature graphs passed. Hosted integration 197 passed/1 fixture compilation failed; unit compilation failed on stale imports. Supply chain, architecture, diagrams and drift passed. |
| Run 7 | 8 required PASS, 2 required FAIL, 1 advisory PASS. Runtime unit suite 3,858 passed; VM integration 43 passed/1 fixture compilation failed. All 102 tooling tests, Clippy, host/Windows warning checks, retained evidence, diff integrity and 1,029-record advisory metadata passed. |
| Run 8 | All 9 required steps PASS. VM integration 44 passed, including `vm_null_literal_preserves_function_block_instance_owner`; fragment and workspace formatting, focused Clippy, host/Windows warning checks, retained evidence and diff integrity passed. |

Run-6 failures had four causes: reserved POINTER identifier in the new ST fixture; two unused
imports left by the lowering-fixture rewrite; the mutation schema still permitting only old
dated paths; one blank line at spec 12 EOF. Those corrections were independently reviewed.
Run 7 additionally exposed the existing format debt in the touched support include fragment,
and correctly rejected the fixture's external writes to FB outputs. The final reviewed fixture
uses distinct `VAR_INPUT seed` values, assigns NULL before instance-field reads, and asserts
11/101 for the two instances. No runtime rule or assertion was weakened to accept the fixture.
Run 8 formatted the support fragment and tested the corrected fixture.

The qualified evidence is the passing union above, not an all-green run 6 or 7. Runtime unit
assertions executed in run 7; subsequent support-file changes are rustfmt-only and those unit
assertions were not re-executed. Other runtime integrations passed in run 6; the entire changed
VM integration binary passed in run 8. Together these cover 198 hosted integration assertions.
The no_std core production code tested in run 6 did not change afterward. No broad redundant
rerun or mutation campaign was performed. Historical artifacts remain byte-pinned and all
active mutation shards remain planned, with no inherited measurement claim.

Final executed-source manifest: 252 file/deletion records,
SHA-256 `3415c33e97675a74840273f472c006ab01888aa45815593d57367430887c7815`.
All 252 matched the local checkout after copying back formatter output and generated diagrams,
before this closeout/evidence update. The frozen source archive and full logs are retained under
the workspace `.artifacts/runtime-portability-a2/run-8/`; portable raw logs and manifests are
tracked in `a2-evidence`. Subsequent edits are closeout/checklist/evidence only.

A2's expanded implementation and allocated validation are complete. This does not qualify an
MCU firmware link, board execution, WCET, MCU peak memory, native Windows/macOS execution,
STBC 2.0, compiler-free executable construction, A3/A4, or publication. The merge agent owns
#129 separately and started its exact-SHA guard at `0d8700b1a` after A2 released the builder.


Final independent closeout review reproduced all reported native/tooling counts, the 252-record
run-8 manifest identity, and all 132 retained artifact hashes; none of those evidence files is
ignored. It confirmed that only four closeout/evidence files differ from the final executed
snapshot. The evidence README's historical wording was corrected and reread; no closeout
blocker remained. The reviewer ran no tests, builds, formatters or validators. This review
does not add native execution evidence beyond the recorded runs.


## Local commit authorization

After closeout the user instructed: commit the verified A2 work locally, then start A3 on a
separate branch. The A2 commit excludes the three shared release-guard parity files already
committed on #129. No new A2 validation or push is part of this local commit.
