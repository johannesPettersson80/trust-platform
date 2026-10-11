# U modernization execution

The initial authorization covered U implementation, test authoring, independent
review and consolidated validation after local Scope B commit. Later publication
authorization and its active preparation are recorded at the end of this note.

Source: `/home/johannes/projects/trust-platform-portability-u`, branch
`feat/runtime-portability-modernization`, base
`5b7f2ea55bb7803e594ebd84983eb9e60285f898`. This includes current main corrections
and the verified source checkpoint. Canonical `/home/johannes/projects/trust-platform`
AGENTS.md, CLAUDE.md and complete skills were copied manually; all 17 source files match.
Canonical files and the publication candidate stay untouched. Publication corrections
will be integrated before U validation where they affect shared gates or code.

## Scope and selected baseline

Specification 34 §16 owns MOD-01 through MOD-06. U covers all in-scope dependency
and project-tool decisions, selected migrations, Rust 1.99.0/MSRV 1.99/edition 2024/
resolver 3, pinned Node 24.21.0 LTS, CI/release/developer-tool parity and the actual
WASM/extension/MCU compositions. ESP32 physical work remains E; its availability
does not block U. Every IEC/runtime function, numeric/fault contract, F401 memory
partition and acceptance margin is preserved. No relaxed warning/link/validation
policy, prerelease product compiler or reduced platform capability is permitted.

Refreshed dependency decisions: [table](u-dependency-decisions.md). Raw registry
metadata is external at `/home/johannes/projects/.artifacts/runtime-portability-u/`.
83 Cargo packages, 30 npm packages and five Git declarations are covered before
tool/transitive/patch review. The historical inventory remains dated evidence.

## Implementation order

1. Finish dependency/tool dispositions and API/lifetime migration design from
   source and primary upstream documentation. Write specification corrections
   before code where support policy or behavior is affected.
2. Apply selected manifests/toolchain/CI/docs, lock resolution and source migrations.
   Edition 2024 process-environment mutation is replaced by explicit configuration
   at owning boundaries; adding unsafe blocks is not a migration strategy.
3. Add necessary native regressions and preserve existing assertions/saved artifacts.
   Adopt new language/library facilities only for concrete defects or simplification.
4. Independently review the full completed diff, then one final source preparation,
   freeze and consolidated validation. No per-edit compile/test runs.

Dependency resolution/fetching during implementation is not test execution. All
Cargo/npm commands, including lock generation, run in isolated builder source under
a target lease. No U compilation or validation runs concurrently with publication
until resource contention and storage capacity are coordinated. New project tools
use task-owned installs; the publication environment is not upgraded underneath it.

## Validation allocation (commands finalized before freeze)

- Frozen source/compiler/dependency/tool identities, supply chain for all Cargo
  and firmware/fuzz locks, npm peers/audit disposition and full MOD-01 ledger.
- Rust 1.99 root syntax/HIR/IDE/LSP/debug/runtime workspace corpus, all features
  where required, runtime vertical and affected protocol/library regressions.
- Core no-default native/i686 and isolated F401/C6 graphs; final F401 firmware
  link/region inspection and physical trace/stack/heap/depth/GPIO/watchdog replay.
- Same saved STBC main/numeric artifacts and labeled exact/tolerance oracles;
  compare like-for-like extracted/modernized numeric and footprint observations.
- Actual WASM bundle/generated binding compatibility and browser wiring; VS Code
  lint/compile/native tests and captures, frontend build/real rendered surfaces.
- Architecture/full-map and unsafe inventory, then diagrams/render/drift, formatting,
  native/cross-target warnings, public docs and relevant tooling contracts.

Unavailable native macOS/Windows or C6 physical evidence remains explicitly open;
Linux cross compilation is not a substitute. Applicable production qualification
continues in later scopes. A future publication requires its own exact-SHA guard.

Current state: selected manifests and lock resolution are implemented for the root,
firmware and both fuzz graphs. All three npm projects resolve peers under privately
installed Node 24.21.0; lockfiles have returned from the builder. Edition 2024
configuration migrations are authored at verification, register-executor, PKI and
persistence boundaries. Production wrappers keep their existing environment reads,
including persistence reconnect reads; tests inject their own values/paths. Native
stamp rejection/default/clamp/symlink/startup assertions are preserved and a non-UTF-8
stamp regression is added. A short diagnostic condition uses a let chain; no numeric
or interpreter behavior is rewritten to demonstrate a new compiler feature.

Builder U destination is `/home/johannes/projects/trust-platform-portability-u`,
detached at the same base. Manual canonical copies match combined SHA-256
`0d0fd96eabb83f9d93d470aee8f7547b274e6a74ef819bf68b4dbc69804d54b3`.
The builder main checkout's agent files are older; it is not the canonical source.
Rust 1.99.0 installed (b940084d7, 28 September 2026). Node archive verification
passed against official SHASUMS256.txt. At this historical source-only checkpoint,
no U product compile, test, formatter or hardware run had occurred. Independent source review accepts the implementation for final preparation;
private tool preparation and the final formatted/generated-source review remain
before the validation freeze. Release target is 0.24.74;
no U commit or publication is authorized.

Implementation findings after source-only API review: EtherCrab 0.7's new default
lock parameter is now explicit in the hosted group alias; opcode/PLC logic is
unchanged. Root/firmware/fuzz Cargo and all npm locks have returned from the builder.
Both fuzz locks are intentionally retained by .gitignore exceptions. Existing
TypeScript application logic was not extended. Node16 module resolution preserves
CommonJS extension output under TypeScript 6 while respecting dependency exports.
The extra U release slot is 0.24.74, distinct from the publication agent's 0.24.73.

Shared gate correction from publication commit `fed5dc457` is incorporated as
uncommitted U source: Rust owns external unsafe inventory/admission, with firmware
and exact registered tests included; no policy predicates or registrations are
relaxed. The same reviewed module is selected with U's ast-grep 0.50.0 tool version.
Publication evidence remains for its own build tuple; U will validate the new
compiler/dependency/tool graph independently. The original apply attempt failed
atomically at the locally updated script-version line; the conflicting wrapper
was then copied explicitly and all other hunks applied together. No source hunk
was dropped and no implementation/validation run was hidden.

Control-purpose start checkpoint: the Rapier dependency migration concerns part 2,
plant simulation, recovered in this worktree's existing hosted simulation service.
The owning simulation contract remains specification 11's existing simulation
profile, plus the control-purpose contract. Existing controller/plant/ST examples
and their semantic assertions remain unchanged; `simulation_workflow` and hosted
simulation/Control.PID tests are required in U. This task does not claim the full
Control Studio/course acceptance journey or substitute a handwritten controller.

The extension harness passes an explicit test-version environment value to the
upstream test-electron SDK, with its existing stable default unchanged. U runs the
full wiring corpus on minimum supported VS Code 1.109.0 and current stable as two
separate target identities in the same batch. Build/format/binary preparation is
shared; neither execution is a retry of the other. The harness's Cargo invocations
use `--locked`. Native Rust assertions continue to prove logic; extension runs
prove the wiring/rendering at both supported SDK boundaries.

Rustfmt parses edition 2024 but preserves the repository's style edition 2021.
This separates the supported language migration from an unrelated whole-repository
format rewrite; final preparation still formats touched source/include fragments.
The owned-frame iterators already use `+ use<>`; borrowed iterators retain their
borrowed result contracts. The memory cache read guard is consumed inside its
and_then before a write guard is acquired. Source review does not substitute for
the final native lifecycle/borrow assertions.

The broader plant specification was recovered read-only in
`trust-platform-control-tutorial/docs/specs/33-simulation-plant-models.md`; it extends
spec 11's existing physics/coupling producer. U does not import that unmerged
feature implementation into the portable baseline or claim its course acceptance.

## Consolidated command map

Commands run only after full independent review and source preparation. All Cargo
and npm stages are inside the task's target lease, exact Rust 1.99.0 and private
Node 24.21.0/tool PATH. Each independent stage records its own result; prerequisites
prevent invalid downstream proof. No retries or filtered rescue runs are embedded.
`--no-fail-fast` retains all reachable Rust test failures.

| Stage | Command / composition | Evidence / prerequisite |
| --- | --- | --- |
| Source preparation | `cargo +1.99.0 fmt --all`; explicit touched include fragments with the repository config; firmware/fuzz formatter scopes; generated browser bundles through existing scripts | One preparation phase; review output before freeze; no intermediate compile claims |
| Frozen identity | Source files + compiler/Cargo/Node/npm/tool versions + four Cargo and three npm locks + research snapshot | SHA-256 manifest before validation |
| Supply chain | `bash scripts/supply_chain_gate.sh`; deny/audit for both fuzz locks; each npm `audit --audit-level=low` | Existing expiring exceptions only; all graph results retained |
| Lint/native warnings | `cargo clippy --locked --all-targets --all-features -- -D warnings`; root/firmware/fuzz formatting and warnings | Cross environments unset CC/CXX; no lint suppressions |
| Native workspace | `scripts/cargo_test_fast_link.sh test --locked --workspace --no-fail-fast` | Includes runtime unit/integration/vertical/simulation, syntax/HIR/IDE/LSP/debug and all new configuration assertions once |
| Isolated core | `cargo test --locked -p trust-runtime-core --no-default-features --no-fail-fast`; selected i686 musl lib + existing five-suite allocation/layout/execution lane | Native no_std and executed 32-bit proof; no claim of complete i686 corpus |
| MCU graphs | Separate `cargo check --locked -p trust-runtime-core --no-default-features --target thumbv7em-none-eabihf` and `riscv32imac-unknown-none-elf`; `cargo tree ... -e features,no-dev` | Library graph and ABI guards; cannot substitute for firmware linking |
| Firmware | Firmware native lib tests, warnings/lint; `cargo run --locked -p xtask -- portability build-firmware <report>` then `inspect <ELF> <report>` and baseline map comparison | Full functions, overflow checks, safe ICF, existing partitions and >=16 KiB upper margin |
| Cross-host compile | `scripts/check_runtime_cross_target_warnings.sh --install-missing --require-cross`; `scripts/prepush_ci_gate.sh` Windows-LSP lane | Linux cross evidence; native macOS/Windows remain GitHub evidence |
| WASM/browser | Actual `trust-wasm-analysis` wasm-pack bundle and existing browser/WASM contract tests; frontend type/build and capture workflows | Matching wasm-bindgen 0.2.129; inspect captured PNGs from live rebuilt surfaces |
| Extension | npm ci/audit/lint/compile once; full test-electron execution for 1.109.0 and stable with shared native binaries | One run per target, no retry; look at required real captures |
| Tooling/docs | Relevant scripts/verification/native xtask tests, strict public-doc build using pinned requirements, container compiler/project smoke | Advisory metadata recorded separately; no invented gate passage |
| Architecture/diagrams | `cargo xtask architecture-doctor --full-map`; external unsafe + existing fail-closed gates; render/drift only after factual checks | New gate integration and exact registered sites intact |
| Physical F401 | Reviewed B-R4 backup/program/verify/trace flow, new source/compiler/ELF tuple; Rust verifier | Only after required software + ELF inspection pass; full main/numeric/GPIO/depth/heap/MSP/IWDG record |
| Reconciliation | Frozen-source stability, each artifact digest, source-qualified old/new observations and open native-platform/device limits | No reuse across changed build tuples; no commit/push in U |

Core all-features and explicit runtime-vertical commands are not added a second
time when covered by the complete workspace run. Minimum/stable editor targets
and separate no-default/32-bit/MCU compositions are distinct evidence, not reruns.

The publication batch exposed the external tool's legitimate no-match response:
exit 1, complete `[]` stdout and empty stderr. U incorporates the reviewed
`decode_scan_output` fix and its native success/empty/error/malformed/signal
assertions before freezing. Only that tuple is accepted as no-match; ownership,
stale-site and unsupported-form rejections remain intact. Source incorporation
is not a claim that U's ast-grep 0.50 execution already passed.


Independent read-only review accepts the 76-file source checkpoint; the report
and manifest are retained externally. Its firmware CLI correction is incorporated.
The architecture wrapper now preserves caller-selected tool PATH precedence.
Cargo independently prefers CARGO_HOME/bin for plugins: PATH alone still selected
the publication machine's older plugins. U therefore uses an isolated Cargo home,
with verified release binaries in its own bin, shared registry/Git download caches,
and its own six-job native configuration. Global installs/configuration remain
unchanged. CLI version/help observations confirm cargo-deny 0.20.2, mutants 27.1.0
and llvm-cov 0.9.1 through that private Cargo home. These are tool observations,
not executed gates. Cargo-deny 0.20 moved --config to the root command; the firmware
policy command and its native orchestration regression now assert that API while
still running every graph and preserving failure aggregation.

The docs Pages path filter includes the pinned requirements file. Both standalone
fuzz workspaces explicitly select resolver 3. Current firmware/capture setup docs
and the gate-inventory compiler description agree with U; historical Rust 1.95
results and B acceptance contracts remain unchanged historical evidence.


Publication identified one advisory stale catalog binding after A4 removed the
host budget module. U rebinds the existing logical test record to the unchanged
core ExecutionBudget assertion using the repository discovery generator. The
current denominator review follows that native identity. Existing invariant and
evidence IDs stay intact; no old evidence is claimed for the new compiler. The
expected-result sentence now describes the actual exact-charge/next-rejection/
unchanged-budget assertions, rather than attributing a whole-program workload to
this unit assertion. This is current bookkeeping maintenance, not a new feature.


Final source review found that the proposed Zenoh 1.10.1 client conflicts with the
qualified 1.7 router/plugin version policy and baseline gate. Independent review
confirmed bounded retention of exact 1.7.2, with root and main-fuzz locks re-resolved.
There is no protocol-family or capability cut in U. Both Vite library configs use
the maintained Rolldown single-bundle options; live generated-asset evidence stays
in the final batch. At that historical checkpoint source preparation had not started; the private public-api
0.52.0 tool source installation completed successfully in 1m36s at two jobs.
That tool installation is not product compilation or a passing U check.


Validation configuration review: native workspace tests use the complete default
product feature graph (which already enables every hosted protocol/database), and
Clippy compiles all features. `openot-real-database-tests` is an instrumentation
feature that requires real products and serialized contract execution, so it is
not enabled blindly in the workspace run. Its configured seven-product gate is
planned separately inside the same batch, with isolated owned containers, ephemeral
CA inputs and teardown. This preserves real protocol assertions rather than
silently skipping them or executing them without required environment inputs.
Core std/hir features are unified by the complete workspace; isolated no-default,
32-bit, MCU and compiled-out-backend compositions remain distinct evidence.


## Source preparation result and prepared correction

The single source-preparation phase completed with one failure: root/touched/
firmware/fuzz formatting and all three npm installs passed; CodeMirror generation
passed; Monaco generation failed on its package entry-point resolution. The
provenance generator was left unrun because preparation was incomplete. No U native
or browser tests, hardware run or consolidated validation batch has started.
The complete preparation ledger, commands, outputs and failed partial bundle are
retained externally. No retry has launched.

Owning cause: Monaco 0.57 exports JavaScript subpaths relative to `esm/vs`.
The old `monaco-editor/esm/vs/...` imports therefore resolve to a nonexistent
nested path under the new export map, even though the underlying files exist.
All JavaScript imports now use the supported exported subpaths, preserving every
editor contribution. The separately published CSS file has no package export;
Vite binds that one existing asset explicitly, preserving inline style injection.
The native module-import wiring assertions follow the supported specifiers.
Both Vite configs use Node's supported `import.meta.dirname` instead of the legacy
config-loader shim. No library is externalized or capability removed.
The incomplete 1.65 KiB output is archived before restoring the last complete
bundle; it cannot be used as a verified product asset. The corrected source and
remaining command scripts require independent review before another build.

Independent correction review found that Vite's literal alias matcher sees the
query suffix before resolution. The final CSS alias therefore matches the exact
`editor.main.css?inline` import and preserves `?inline` in the resolved file path.
The existing Monaco import contract now imports the actual config and asserts
that binding and the published asset's existence. No CSS extraction or editor
contribution is lost. A separate Playwright wiring case starts the actual compiled
WASM worker, observes a valid document, an undefined-symbol diagnostic and recovery
in the live browser, and retains screenshots. Node worker mocks remain distinct
from this compiled-WASM evidence.

The validation launcher requires the correction phase's explicit success marker;
a completed failed preparation cannot admit a validation launch. The original
failed preparation is immutable. The prepared suffix repeats only Monaco generation
and performs the previously unrun current-provenance generation, then requires a
fresh full-source review/freeze. No correction suffix or validation batch has run.

Current manual bootstrap verification covers 17 source instruction/skill files;
the earlier 21-file receipt also included four generated Python cache files.
All source instructions match the primary checkout byte-for-byte. Generated
interpreter caches are not canonical instruction sources.

Validation preparation now includes owned real-database provisioning and teardown,
using the existing pinned product/TLS runner and exact database gate. Generated
connection settings are read as data and never logged; no shared deployment or
credentials are touched. This qualifies the affected maintained driver graph;
Zenoh's separately qualified 1.7 family remains pinned.

The frozen source inventory includes all tracked and nonignored new files,
including absent/deleted paths, and detects newly introduced source paths. Only
expected rendered diagram outputs, their manifest and public-doc images are
excluded from stability comparison; these outputs receive their own retained
hashes and visual review. Runtime bundles, Rust, JS, CSS, HTML, tests, specifications,
locks and rule files stay frozen.

Old/new F401 observations use the retained publication source `ca205132e`, Rust
1.95.0, product 0.24.73 and its exact same three saved artifacts as the baseline.
The private command inputs retain the baseline map, ELF-region report and raw
physical-verifier report with digests. U compares regions, preparation/RUN heap,
MSP/depth/IRQ and fixture/numeric times against that tuple. Firmware acceptance
remains the existing >=16 KiB upper-flash and >=2 KiB observed MSP margins, 72 KiB
heap and unchanged trace/numeric/GPIO/watchdog assertions. A speed difference
belongs to the combined compiler/dependency migration; no isolated compiler gain,
whole-profile timing qualification or AM6442/other-host performance claim follows.
Native Windows/macOS, ESP32 physical and broader hosted-performance qualification
stay open until their actual target evidence is available.

Documentation tools are installed in a private U virtual environment using the
pinned requirements and pip 26.2.1. Tool installation is not a documentation gate
result. No U consolidated test or hardware batch has started.

The source-only helper review rejected the initial Python source checker and
runner-environment bridge under the canonical all-logic-in-Rust rule. Those
unexecuted drafts are retained externally as rejected drafts. Their active
replacements are small `xtask::portability` modules: source identities include
canonical ignored instructions and the actual ignored WASM web inputs; the
runner parser rejects malformed, duplicate and unexpected entries without
including secret values in errors. Settings are applied through `Command::envs`
only to the existing gate process, with the runner's already-created shared CA.
No process-global environment mutation or second database qualification rule is
introduced. Native tests cover changed/added/deleted identities, generated-output
boundaries, literal shell-looking values and malformed/duplicate environment data.
These tests are authored and unrun; the first workspace batch owns their execution.
The correction preparation formats these new tooling modules once before freeze.

Electron minimum/stable executions use independently owned short IPC temporary
directories, preserving the reviewed release-guard socket-path fix. Only these
editor processes use the short directory; compilation and retained evidence remain
on the volume. U does not inherit the publication agent's unlimited rerun scope.
The failed source generation is retained, the correction is prepared for review,
and a further generation attempt requires explicit authorization under AGENTS.md.

The runner bridge accepts only the existing producer's closed setting names
(state/prefix, the five network URLs, Influx host/token and the six CA/container
pairs). Unexpected names in that namespace are rejected too. Existing TLS
credentials and exact native database assertions remain unchanged.

Supplemental independent source review accepts the completed 109-path prepared
checkpoint (external `prepared-correction-review-3.json`, SHA-256
`92a41c661117ab0ce80a1cd61afa32056f3179d9c8191ef618bf422c18cb071d`).
The reviewer accepted the Rust identity/runner modules, public Monaco imports,
queried CSS binding, actual config/artifact assertions, live compiled-WASM case,
short Electron temporary directories, CLI/spec/diagram ownership and shell
orchestration. The readable report is retained externally. This is not compiler,
formatter, generated-bundle, test or hardware proof. The next concrete action is
the prepared correction phase, final generated-source review/freeze, then the
consolidated batch under the applicable execution authorization. No U commit or
publication is authorized.

Current continuation checkpoint: the owner's “continue” follows the retained
source-preparation failure and permits carrying the reviewed correction phase
through the previously planned first consolidated U validation. This is one
correction preparation followed by generated-source review/freeze and the first
U batch, not standing authorization for automatic later retries. The original
failure remains retained, and another failed correction/batch stops test launches
until further explicit authorization. Publication's unlimited retries remain its
own separate scope. No U commit/push is included.

## Correction preparation result and prepared SHA output migration

The authorized correction phase completed: new tooling formatting passed and
Monaco generation passed (869 transformed modules; complete JS/CSS output
retained). Current-provenance generation then failed at compilation in
verification-cases: sha2 0.11's output is a hybrid array with no LowerHex trait.
The failed ledger/logs are retained separately as preparation-correction-1.
No provenance body executed, no U product test/hardware batch started, and no
further generation retry was launched.

The prepared owning correction removes reliance on output-array formatting at
every affected caller: verification case/trace hashes and the generator, xtask
artifact identities, runtime configuration/revisions/ADS evidence and native
simulation/bytecode-case helpers. The already resolved `hex` 0.4.3 encoder is
used directly in the three owning crates. SHA-256 bytes, lowercase representation,
prefixes, canonicalization and all existing expected hashes remain unchanged.
New known-vector assertions pin the verification and artifact boundaries; the
runtime already has the independent exact `abc` revision assertion. These tests
are authored, unrun, and belong to the consolidated batch. Lock resolution is
implementation work, not compilation or validation. Further execution needs
explicit authorization after the retained correction failure.

Independent SHA-output correction review accepted the complete 15-path identity
`05cf5db750bd626c2b3b50e0b2cd558123248b2d7aa90720ffe31a4bcba843e5`,
including both Cargo-owned lock changes, all affected digest sites and native
vectors. The readable report is external. Only formatting and execution remain
unverified for this correction. The prepared next phase runs one digest formatting
pass and the previously uncompleted provenance generator; Monaco is not rebuilt.
Freeze and the first consolidated U software/conditional-board batch follow only
if preparation succeeds. No further retry is launched without authorization.

## Current approval and landed baseline integration

The owner answered “yes” to one correction preparation followed by U's first
consolidated validation and conditional STM32 replay. No automatic later rerun
or U commit/push is authorized. Both failed source-preparation ledgers remain
retained. Before this approved freeze, U incorporates the final code actually
merged through PR #132 at main b7d8ffbf2 and released as v0.24.73: transactional
restart image carry-over before initialization, native Windows path remapping,
Bash 3.2-compatible audit argument transport, and deterministic owned HTTP/TLS
transport-failure fixtures. U preserves cargo-deny 0.20's selected CLI and its
edition-2024 explicit configuration injection in the moved initial-open test.
All retry deadlines, heartbeat ordering, assertions and production policy remain
unchanged. Final integration source review precedes the authorized preparation.
The initial integration copy stopped on an incorrect core test path after copying
three core files; the test actually belongs to the hosted crate. Remaining sources
were pre-read and applied at the correct paths, with no validation run involved.

The U comparison baseline is refreshed to the actual released 0.24.73 source
c3def9e14/main b7d8ffbf2 and its image 4761d2bc… (Rust 1.95.0), whose byte
identity to the fresh run-5 physical replay was checked at the final head. The
older ca205/0597 comparison inputs are historical only. The map, ELF inspection
and physical-verifier JSON are copied into private U inputs with SHA-256 receipts.
The publication target was declared retired after all release checks/audits
passed, and its gate evidence was archived before idle-only generated-cache
removal. Source worktrees, retained raw proof and U's own target remain intact.

## Approved preparation closeout and first batch freeze

Preparation-correction-2 passed both required steps: explicit touched-source
formatting and the Rust current-provenance generator. Earlier failures remain
retained separately. The final independent reconciliation accepted 119 current
source paths, manifest SHA-256
`c11a5270f9bc0cdc9ce199aa18c277256b7f7b262f1c370b98a3773da7c45e7c`,
plus the required application-pack ledger and unchanged path-only board protocol.
All seven active mutation selectors retain actual discovery receipts; case bodies,
historical mutation measurements and report blobs are unchanged. Formatting-only
reconciliation uses command logs, hash deltas and source review; there is no
pre-format content archive and no claim of mechanical token-by-token equivalence.
This note and the checklist status follow that reviewed identity; the source
freeze binds their final bytes along with all implementation and instruction files.

The next action is one freeze, then the already-authorized first consolidated U
batch on the isolated builder under its target lease. Source edits stop during
that batch. The required application pack is explicitly UNRUN if link or retained
artifacts fail. Physical F401 work starts only after every required software step
and ELF inspection passes. Authored regressions and older release proof are not
reported as executed U proof. No automatic later retry or U commit/push is included.

Preflight: builder has 425 GiB free on the U target/TMP volume, 29 GiB available
RAM and no competing Cargo/native workload. Private Cargo parallelism is six jobs
on sixteen cores. Canonical root AGENTS.md, CLAUDE.md and all 15 skill source files
match the Pi primary checkout; remote copies are refreshed manually before freeze.
The full function set, overflow checks, device-side admission and all existing
flash/heap/MSP/trace/watchdog thresholds remain unchanged.

## Failed first freeze and prepared ELF API correction

The approved first freeze attempted to compile xtask on Rust 1.99.0 and failed
with exit 101: sixteen diagnostics all belong to the object 0.40 ELF flag API.
The inspector still converted section constants to integer masks, matched the
old section-flags shape and compared typed program/file flags with integers.
The compiler log and failure ledger are retained externally under U freeze-1.
No source manifest was created; the consolidated U product batch and physical
phase are UNRUN. No compilation, formatter or test retry followed this failure.

The prepared owning fix migrates every object caller in the ELF inspector to
its typed SectionFlags/FileFlags/ProgramFlags API. contains preserves required
bits and intersects preserves forbidden bits; the integer ARM hard-float constant
is wrapped in the upstream FileFlags type. The newly exposed section-type field
is deliberately ignored, preserving the prior region/flags contract. All margins,
error strings and existing assertions remain. The existing immutable-tail native
case additionally asserts missing allocation is rejected and an unrelated merge
flag is accepted. These new assertions are authored, unrun.

The one-use freeze-correction-1 script is prepared, not launched: format this
owning file and retry the source freeze. The previously planned first consolidated
U batch remains conditional on successful freeze; STM32 remains conditional on
all required software/ELF gates. Another execution requires explicit owner approval
under AGENTS.md's failed-validation rule. U commit/publication remains excluded.

Independent targeted review accepted the complete ELF correction manifest
`6cb7a17c1898b50e484cf9f6bfb30268ca2cfea67bc150674cee7970208d45cc`
and owning source `fb11710cee9723dc44719ab1eca8f372b1fcccfbeed23f61f9783e12c4803e35`.
The reviewer read exact resolved upstream definitions and searched all callers;
no omitted owning migration or changed safety predicate was found. No execution
proof is implied. The correction is ready for one approved format/freeze attempt,
then the previously planned first software batch and conditional STM32 replay.

## Owner approval of ELF correction execution

The owner answered “yes” to one freeze retry followed by the already planned
first consolidated U batch and conditional STM32 replay. Execute
freeze-correction-1 once, then run-1 only if the frozen source is created
successfully. Retain earlier failures. No automatic additional retry, U commit
or publication is authorized.

## First U batch result and completed owning corrections

The owner-approved correction freeze passed: xtask compiles with object 0.40;
only one line-break change followed the accepted ELF correction. The completed
run-1 source manifest has 8,431 records and SHA-256
`f35f148e3e8d2cb84beb14bd79d4e91281e0334a61f16506a1bffc9afd2d4740`.
Run-1 ended with exit 1. Its full command/ledger/log/map/ELF evidence is retained
externally; original preparation and freeze failures remain unchanged. No test
rerun, physical replay, U commit or push followed the failed batch.

Passing evidence: portable native and i686 suites, both F401/C6 library graphs,
firmware native tests/Clippy/adapter/link, actual ELF inspector/map/application
packing, tooling/path/IEC hygiene, smoke/observability/Zenoh contracts,
architecture/diagrams/drift, Node worker contracts, extension lint and strict docs.
These do not substitute for failed/unrun hosted, browser or physical qualification.
New ELF `7fc1d55429300de5ceead43680f0480b887de6228f339dc935ab549c94bcdfe4`
leaves 23,520 B upper code free (existing floor 16 KiB), 256 B sector-0 free,
73,728 B heap reserved and 16,384 B stack reserved. Actual U heap/MSP/timing on
the board remains unknown. The released baseline leaves 18,720 B upper free;
the combined new compiler/dependency tuple shrinks that span by 4,800 B.
No isolated compiler or package causal saving is inferred.

Complete known corrections are prepared, unvalidated:

- The only Rowan owning call uses green().to_owned(); the registered-name find
  predicate has an explicit outer reference for edition 2024. All callers were
  source-reviewed. Existing anonymous type-name and source-edit/cache assertions
  remain unchanged. The owner specifically requested fixing the HIR errors.
- Explicit Node/Mocha ambient types restore the intended extension-host/test
  typing under TypeScript 6; no TypeScript product logic or assertion is changed.
- Both standalone fuzz packages declare the repository license; exact
  libfuzzer-sys 0.4.13 gets its reviewed NCSA license allowance only at that
  crate/version. No private package, source, advisory or audit check is skipped.
  Remove obsolete spin 0.9.8 exception absent from every current lock.
- Canonical guard test agrees with caller-first PATH plus Cargo fallback.
  Rebootstrap copies after that canonical test edit. Manually correct the two
  formatting omissions; final preparation formats the whole completed workspace
  and its include fragments, firmware and both fuzz scopes.
- Monaco's audited DOMPurify 3.4.15 pin is replaced only on its dependency edge
  by patched 3.4.16 with official integrity. Only package-lock resolution ran;
  install/regeneration/audit and rendered compatibility remain unrun.
- Source-stability failure is exactly 16 tracked extension media files regenerated
  by compile; no Rust, rule or other source bytes changed during the batch.
  Retain those outputs, generate all shipped assets before the next freeze, and
  keep them protected as source. Do not widen the generated-output exclusions.
  The existing source-identity native case now explicitly protects an extension
  bundle alongside the embedded runtime bundle.
- Split Docker compiler proof from the host project workflow. Run-1 built the
  compiler image and printed Rust 1.99, then the combined step failed on a missing
  hosted binary after HIR failure. The next workflow records its dependency as
  UNRUN; it does not describe host project execution as inside the container.
- Advisory metadata identifies six current CI file/job digest bindings. Refresh
  only those data constants through existing projections after reviewing workflow
  changes; predicates, triggers, target selectors, dated campaign/report blobs
  and native assertions remain unchanged. Advisory metadata stays nonblocking.

The distinct run-2 preparation/freeze/batch scripts are prepared, not launched.
Preparation generates patched Monaco and all six tracked extension asset groups,
formats every required scope and refreshes current provenance/discovery. Review
returned outputs before freezing. The next batch retains all independent checks,
requires the affected source-contract suite and separates the container/native
prerequisite. Same function set, checks and memory/oracle floors. Independent
correction review precedes execution; another run needs explicit owner approval.

Independent correction review accepts the owning HIR/type/configuration, exact
DOMPurify patch/integrity/API, scoped license, removed obsolete exception, current
workflow pins and canonical PATH changes. It also accepts the prepared asset-first
freeze sequence and explicit container/native-project prerequisites. Source review
is not execution. A further preparation/freeze/validation launch remains pending
owner approval after the retained failed run-1.

Final independent correction identity:
`4ba56880caa7bd14d3f6c4e7d3cb4641b6d482e56317e45ea2b40505c659a9ee`,
15 source paths all matched, including the shipped-asset negative case and
MOD-06 sentence. Subsequent execution-note/checklist status is separate metadata.
The new canonical PATH test intentionally requires the reviewed caller-first
policy; older candidate branches with a prepended fallback need the owning gate
script migrated when they refresh canonical files. This does not authorize edits
or release work in those branches. No further execution or U commit is launched.

## Owner approval of run-2 execution

The owner answered “yes” to one preparation/freeze/validation retry and conditional
STM32 tests. Run run-2-preparation once, review returned source/assets and freeze
run-2, then execute its one consolidated batch. Physical replay requires every
required software/ELF step to pass. Earlier failures stay retained. No automatic
additional retry, U commit, push or later scope is authorized.

## Run-2 preparation result and blocked output review

The approved preparation passed all fourteen steps. All fifteen accepted source
hashes remained unchanged after formatting; current provenance/discovery reconciles
and shipped extension assets were generated before freeze. The prepared-output
manifest has 38 paths, SHA-256
`f55ec1bb40bba17902898cd06884893fcdcebf22fa29682a9502114ed7f481c2`.
No run-2 freeze or validation was launched.

Independent output review rejects the sanitizer shipping claim: although the
installed and locked DOMPurify dependency is 3.4.16, Monaco's prebuilt domSanitize
imports its vendored 3.4.15 module. The rebuilt runtime bundle remains exactly
`ecb421c35e48f227bc9c7bb083230324d73b651a1108604a05e06f897cfa555d`,
contains 3.4.15 and lacks the patched rejection sentinel. A passing install/build
or dependency audit alone cannot qualify the deployed sanitizer.

The complete owning correction is prepared: bind the exact vendored relative
import to the locked upstream ESM module through Vite, preserving the same default
singleton/API. No dependency-source edits, postinstall patch or application logic
is added. Existing import-contract assertions pin the actual Monaco import,
resolver target and patch version; the shipped-asset assertion requires 3.4.16,
rejects 3.4.15 and pins the removed-root rejection sentinel. Reviewed upstream
patch signatures/exports/config remain compatible. Neither test was executed.

A one-use Monaco-only regeneration script is prepared, not launched; its required
success is explicit in both run-2 freeze and validation prerequisites. It does not
repeat formatting, other assets, provenance or tests. Prior preparation/evidence
is retained; no automatic additional rebuild or validation starts while the
prepared-output review is blocked. The previously approved validation/hardware
allocation remains conditional on accepted prepared outputs and software/ELF
success. U commit/push remains excluded.

Independent targeted review accepts the three owning source changes and one-use
generation/prerequisite scripts. Both assertions are already included in the
existing worker-contract command. Current old generated output is copied
externally before any future replacement. No source-generation retry or run-2
validation was launched; final generated-output acceptance is still required.

The owner now explicitly approves one Monaco-only regeneration and continuation
of the previously approved freeze/validation and conditional STM32 allocation.
No other preparation stage is repeated. Review the actual returned asset before
freeze; no automatic later retry or U commit/push is included.

## Accepted final sanitizer output and run-2 freeze

The explicitly authorized Monaco-only regeneration passed. Local and builder JS
match `dea6a1c40415105c3708bb694d552f1d3b3d92a95a12d9bc324603f77a2ab1f3`,
4,440,537 bytes, with 3.4.16 once, no 3.4.15 and the patched rejection sentinel.
All required editor contributions remain; CSS is unchanged. Independent output
review accepted the returned artifact and unchanged three owning source hashes.
Only the Monaco JS changes among the previously reviewed 38 prepared records.
No other preparation step or test was repeated. Earlier unsuccessful output and
failed ledgers remain intact. This is prepared-output proof, not rendered/native
validation. The remaining approved action is one run-2 source freeze and batch,
with physical F401 only if all required software and ELF gates pass.

## Run-2 result and authorized complete correction batch

Run 2 finished at 2026-10-11T05:30:57Z: 41 steps passed, seven failed and
13 prerequisite-dependent steps remained unrun. Frozen source manifest SHA-256
`038e1ffad65ddca00084db9cdd9bb6795a27a78fef435cc61e8066f398ce3a1c`;
final Rust source-stability check passed. Raw logs, commands, manifest and complete
ledger are retained outside Git under `.artifacts/runtime-portability-u/run-2`
on the editing machine and the corresponding builder evidence directory.
No firmware installation or physical replay occurred.

Failures and owning corrections:

- Strict Clippy: 113 HIR nested-guard findings; migrate eligible workspace guards
  to reviewed Rust 2024 let chains, preserving evaluation order, bindings, boolean
  precedence, branches, comments and borrow/temporary lifetimes. HIR-only edits
  would leave the same edition migration incomplete in dependent crates.
- Native workspace and host cross-warning step: the same assertion fails to
  compile because SmolStr now has several AsRef implementations. Use its explicit
  string view; preserve every assertion and expected message.
- Firmware evidence retention: cached Cargo success did not recreate a deleted,
  undeclared map side effect. The Rust build owner now requests a fresh final
  binary link, reads Cargo's exact executable artifact and retains the new ELF/map
  pair with hashes. Missing, cached or occupied artifacts fail closed.
- Verification-source contract: a resolver-2-specific fixture replacement did
  nothing on resolver 3. Insert the forbidden default-members at the workspace
  header and assert the fixture changed; keep the rejection assertion.
- Compiled-WASM browser: the fixture's unconfigured program legitimately reports
  an unused-program warning. Register it in a configuration/resource/task;
  preserve the strict zero-diagnostic, undefined-name and recovery assertions.
- Extension typecheck: declare stylesheet side-effect modules at the webview
  boundary and consume the library's OnNodeDrag callback type. No UI logic or
  product behavior changes are introduced.

The owner explicitly approved fixing all errors and running tests again while
run 2 was active. That authorizes complete corrections, independent review, one
preparation/freeze and consolidated run-3 batch, with physical F401 conditional
on all required software and ELF checks. It does not authorize U publication or
an automatic subsequent retry. Run-2 results remain historical evidence for
that frozen source, not passage of corrected source. All native workspace,
minimum/stable extension and physical-board qualification claims remain open.

## Run-3 prepared identity accepted for freeze

The complete 228-owner let-chain migration and all root-cause corrections were
independently reviewed before preparation. The seven-step approved preparation
passed; formatter outputs and the required network source map were copied back.
The independent reviewer accepted final 239-source identity SHA-256
`eadf1fc5112106fd9dbf36ad6c86378bc591b415f3357fc9d39912d1ceb399ba`,
matching every record on both editing machine and builder. Necessary original
expression groups, guard order, comments, branch association and lock-bearing
paths were reviewed against the complete pre-wave archive. The initially
incomplete 226-owner manifest is historical; both omitted manual owners were
added with before bytes matching run 2.

The network JS/CSS/CSS-map remain byte-identical to the run-2 frozen inputs.
Only the JS-map's edited source and mappings change. Mutation metadata refresh
changes four source digests and one HIR selector line (245 to 240); seven current
source digests and actual discovery selectors reconcile, with historical results
unchanged. Patched Monaco JS remains `dea6a1c4…`. This is source/preparation
acceptance, not native or hardware passage.

Run 3 now proceeds under the owner's existing approval: one freeze and one
consolidated batch with accumulated independent failures and explicit dependent
skips, no source edits during execution. The final firmware build retains its
own newly linked ELF/map pair; database and board evidence use fresh run-3
namespaces. All required software checks and ELF inspection must pass before
any physical replay. No U commit, push or later scope is authorized.

## Run-3 result and reviewed remaining corrections

Run 3 completed at 2026-10-11T06:04:53Z. Its 8,434-source frozen manifest is
`96e4eb5c0336c17e7fc4912bee450402208295b8f97a014ddee9cd5eceab495f`.
The complete ledger records 49 passing steps (including advisory metadata), two
failed required steps and ten unrun prerequisite-dependent steps. Source stability
passed. Workspace summaries total 8,701 passed, one failed and 24 ignored tests;
the one failed assertion is the VSIX release-contract expectation, not a runtime
or HIR behavior assertion. Ignored tests remain ignored, not passed.

The portable native/i686 suites, F401/C6 graph checks, firmware tests/lint/link,
Linux/Windows runtime warning checks, Windows LSP, Rust docs, tooling, architecture,
diagrams, audits, compiled WASM worker/browser, extension lint/typecheck, public
docs and container compiler all passed. Browser screenshots were opened: the
configured program shows zero diagnostics, the bad edit shows E101 for
MissingValue, and restoration clears it. This does not establish the skipped
minimum/stable editor, live runtime/browser or physical-board acceptance.

Fresh retained firmware ELF SHA-256:
`091b64fe35815a0174771f593f5e63287bae9475fe3a456f76d86e3274cb54d1`;
map `2d3b2e63d7ec93cf83e13ef57fb050f6863f486d379ac6c9e75be26376f88f40`.
ELF inspection passed: 23,520 bytes upper-region headroom, 256 bytes in sector 0,
73,728 bytes heap reserve and 16,384 bytes stack reserve. These are linked/reserved
sizes. No U heap/MSP physical measurement or installation occurred.

Remaining failures and prepared corrections:

- Clippy reports two existing let-and-return forms, now unmasked after the guard
  migration. Six narrowly reviewed direct-tail corrections include four analogous
  hosted owners, preserving checked arithmetic, error propagation, profile events
  and owned values. The JSON snapshot's guard drops inside its map closure.
- The release-contract fixture expects the old npx command spelling. Its reviewed
  data fragment now requires the actual `npx --no-install vsce package --target`
  invocation and retains every binary-copy fragment. New native Rust regression
  authoring covers the complete positive workflow and eight specific rejection
  mutations, using the existing Python checker only as fixture wiring. No new
  Python validation algorithm is added.

Final review found a coverage bookkeeping gap: the 239-record correction manifest
covered 233 changed Rust owners, but the actual run-2 to run-3 change set contains
246. Thirteen omitted owners were independently reconstructed and byte-checked
against both frozen manifests. Eleven contained unrelated, semantically equivalent
parenthesis removal and two formatting only. Ten whole source files are restored
to their run-2 bytes; admission's original grouping is restored while retaining
its new direct return. The two reviewed formatting changes remain. All 228 guard
migration before files also match genuine run-2 bytes. The complete reconciliation
and initial incomplete records remain retained externally; no prior test result
is rewritten or promoted.

Independent review accepts final 19-path prepared correction identity
`db1743e41920f1f65e0898551e7f4e5c5361a3607b17466eddc58dc8610194e7`.
Review record SHA-256:
`40cc525ad7fafb0055e8e258ec0088bbaffed9ea5b98abee94d34fd739f6611b`.
All correction authoring is complete, but final formatting/provenance and tests
remain unrun for these bytes. Raw run-3 evidence and the review/reconstruction
records are outside Git under the task evidence root.

A further batch needs explicit owner authorization under AGENTS.md's failed-run
rule. Planned allocation: prepare and refreeze these corrections; strict full
Clippy and native workspace checks; portable/i686/MCU and fresh firmware
link/inspection; then the ten previously unrun protocol/database/conformance,
producer-consumer, minimum/stable editor, browser/codeserver/project and conditional
F401 gates. Keep applicable runtime vertical coverage deduplicated within the
workspace. Preserve already-passing unchanged dependency/assets evidence; record
all source relationships rather than claiming an exact-SHA release artifact.
No new test run, hardware action, U commit, push or later scope is launched.

## Owner approval of run 4

The owner answered “yes” to one additional consolidated correction batch,
including previously skipped checks and conditional STM32 execution. This
supersedes the pending-approval checkpoint above. The complete reviewed fixes
are implemented. Prepare/format the affected source and current provenance,
review returned outputs, freeze run 4, then execute the existing complete
command allocation once. Keep native runtime vertical coverage inside the
workspace command; no redundant runtime run. No frontend source changed in
these corrections, so shipped bundles require no preparation regeneration.
The selected build/tool environment, full function set, overflow policy and
all board acceptance margins remain unchanged. All earlier ledgers are retained.
Hardware requires every required software check and ELF inspection to pass.
No automatic later rerun, commit, push, release or later scope is authorized.

Run-4 script review confirms unchanged validation commands/prerequisites, fresh
run-4 evidence/DB/board paths, expanded explicit include-fragment formatting and
one-shot execution without retries. Canonical AGENTS/skills manually refreshed
locally and on the builder; all nineteen reviewed source hashes matched both.
Preflight: mounted target/TMPDIR volume has 351 GiB free, approximately 29 GiB
available memory, and no concurrent compiler or test workload. The approved
preparation is now running under the existing target lease.

## Run-4 prepared identity accepted

The approved preparation passed all six steps. Independent review accepts final
nineteen-path identity `1a25630c8d988a4fc0016367656628340fb24f28e3dc200a461c8e2d49074941`,
matching every source record locally and on the builder. Four hashes differ only
by formatting: admission line wrapping, native VSIX test formatting, and whitespace
in the two restored include fragments. Original parentheses remain. Current
mutation provenance and network JS/CSS/maps match their run-3 frozen identities;
provenance refresh reported no changes. No reviewer builds, tests or formatting
were executed. Source acceptance is not validation passage.

Freeze and run the approved full run-4 allocation now, with no source edits during
execution. Retain every reachable independent result and prerequisite skip.
Physical F401 remains conditional on all required software and ELF checks. No
additional approval is needed for this already-authorized batch; no automatic
later rerun or U publication is included.

## Run-4 result and targeted correction authorization

Run 4 finished at `2026-10-11T06:47:16Z`: 56 PASS, 5 FAIL and 1 UNRUN.
The frozen 8,435-record manifest has SHA-256
`bd89c3b7a13f0b0e065a7d89a3beba9e39f074221b593ddd96c5b19cb9fe9e02`.
Full native workspace: 8,703 passed, 0 failed, 24 ignored. Strict Clippy,
portable native/i686, F401/C6, firmware build/inspection, cross-target checks,
tooling, supply chain, mesh/TLS, conformance, producer parity, architecture,
diagrams, WASM/browser, extension compile/lint and documentation passed.

The required failures are database TLS provisioning, canvas browser launch,
both editor evidence launchers, and the project pipeline's fixture selection.
The latter four reported no failing product assertion. PostgreSQL/TimescaleDB
startup logs show permission denial at `/tls/server.crt`; mode evidence confirms
their mount directories inherited 0700 for the host UID under umask 077.
Chrome stderr was discarded, so its historical underlying launch cause is not
established. Both editor targets stopped before tests because the wrapper omitted
`TRUST_VSCODE_TEST_SOURCE_COMMIT`. Tutorial 21 is README-only and explicitly
selects runnable Tutorial 11. No physical U firmware replay occurred.

Retained fresh ELF `52c523123047d7e8c7c8248cc4108f80b5054a41f8ca8f68812acbebf6f77d2a`
and map `3fec54fe4f868449b38477eeea8d741a78cb921d4f2eed1e2094dcd1d4a2a01a`
leave 23,520 bytes in the upper region and 256 in sector 0. Heap/MSP reservations
remain 72/16 KiB; these are reservations, not new physical high-water results.
The final source-stability check passed. Complete raw evidence and original
launched helper copies remain outside Git in the task's `run-4/` directory.
Browser/WASM/code-server screenshots were opened for AI visual review; this is
not owner UI acceptance or native editor qualification.

The owner explicitly changed the correction cadence: fix these errors and run
focused checks first; run full suites afterwards only when required at this step.
Run 5 therefore targets the new native TLS permission regression and decoder
tests, xtask lint/formatting, four canvas fixtures, editor selection/evidence
smokes on minimum/current versions, the corrected project pipeline and the
failed real-database gate. Independent review precedes freeze and execution.
If all focused checks pass and both editor smokes execute nonzero tests, run the
still-required full minimum/current editor suites on the same frozen source.
Do not repeat the already-passing full native workspace or unchanged MCU builds.
Physical replay is conditional on all required software proof, source/ELF
correlation and existing inspection margins. No U commit/push is authorized.
Any later failure remains a failed/unverified result, not permission to retry.

## Final software and F401 reconciliation

The owner authorized running existing tests, followed by the remaining project
build/validate check, physical STM32 tests and checklist reconciliation. No U
commit, push or release is included. No product code, compiler selection rule or
new helper was introduced during this final supplement.

Run 4's five required failures and skipped board phase remain in its original
ledger. Run 5 closed TLS permissions, native provisioning regressions, the real
database gate and all four canvas fixtures. Its pipeline script failed before
product commands because of quoting. Run 6 repaired that invocation but used an
inappropriate deployment fixture: Tutorial 11 contains an unbound TEST_PROGRAM
under CONFIGURATION and lacks bundle configuration. The ordinary compiler's
rejection is unchanged from base; the test command explicitly registers test
instances. Calling that rejection a compiler regression was incorrect.

Final project evidence uses the existing `tests/fixtures/ci/green` fixture, exactly
as the native CI-command corpus does. A disposable copy outside the repository
passed existing `trust-runtime build --project <copy> --ci`, `validate`, and
`test --ci --output junit` / `test --ci --output json` commands. Both report formats
contain the two passing ST tests. Tutorial 21's inherited contradictory build
instructions remain a documentation issue; this result does not certify them.

| Supplemental check | Result | Evidence under external task root |
| --- | --- | --- |
| Existing native command suites | `build_command`: 3 passed; `st_test_cli_command`: 8 passed; 0 failed | `direct-tests-20261011/native-command-tests.txt` |
| Minimum VS Code | 1.109.0, full unfiltered suite: 519 passed, 0 failed | `direct-tests-20261011/editor-1.109.0.txt` and `.json` |
| Current VS Code | 1.141.0, full unfiltered suite: 519 passed, 0 failed | `direct-tests-20261011/editor-stable.txt` and `.json` |
| Project CLI pipeline | Build/validate successful; 2 ST tests passed in each report format | `direct-tests-20261011/project-build.json`, `project-validate.json`, `project-tests.xml`, `project-tests.json` |
| Source stability before hardware | All 8,436 frozen records match | `direct-tests-20261011/source-stability.txt`; run-5 manifest |
| F401 physical acquisition | Installation, preservation and bounded capture exited 0 | `run-4/board/`; mirrored under `direct-tests-20261011/physical/` |
| Rust UART verifier | Exit 0, all required physical records accepted | `direct-tests-20261011/physical/verification.txt` and `report.json` |

The native command is the existing fast-link wrapper with
`test --locked -p trust-runtime --test build_command --test st_test_cli_command
--no-fail-fast`. Editor execution uses existing `node
editors/vscode/out/test/runTest.js` under Xvfb and the Cargo target lease, without
a grep selector. Both editor JSON files correctly identify the base commit;
dirty tested source is identified separately by the frozen run-5 manifest,
SHA-256 `47f9783bf4d0b5d81d14d4f6303be26d7f44398a7612dc4e66cc04212c29377b`.
No full workspace, real-database gate or firmware build was unnecessarily repeated.

Hardware uses the unchanged reviewed `board-run4.sh`; its fresh destination is
the previously unrun `run-4/board/`. The original run-4 ledger is not rewritten:
physical execution occurred after its authorized software supplements. The runner
backs up all flash, explicitly erases firmware sectors 0 and 4–7 and application
sector 1, verifies both images, preserves checkpoint sectors 2–3, compares firmware
before/after the application write, clears only the recorded reinstall cookie,
and captures from before the single commanded run reset. Byte comparisons passed.

Firmware/core/adapter/portable-fixture/UART-verifier records are identical between
run-4 and run-5 frozen manifests (selected-record SHA-256
`5d25e83271060374cebdabba9abecff49fa904b220edb4b1a6db773956ce1963`). The installed
ELF is `52c523123047d7e8c7c8248cc4108f80b5054a41f8ca8f68812acbebf6f77d2a`;
application `71cb8c62f8b13462455046cec44e58857cfe82bd33de8ba5c5f996e9258edb61`.
The retained UART is `ba3fe2707a39d951332a1f86402a4ca6fbeca9d012ecc3a3ac03825f76eb3149`;
Rust report `79b91dadb2bd643567749180eb2ad8238d4f5c99cfd7ec2e2cc6ff288b7d2a6b`.

| F401 observation | Released baseline | Modernized tuple |
| --- | --- | --- |
| Upper-region flash free | 18,720 bytes | 23,520 bytes |
| Sector-0 free | 224 bytes | 256 bytes |
| Measured heap peak / reserve | 28,272 / 73,728 bytes | 28,272 / 73,728 bytes |
| Cumulative MSP peak / reserve | 13,596 / 16,384 bytes | 13,652 / 16,384 bytes |
| MSP remaining | 2,788 bytes | 2,732 bytes |
| Observed maximum main scan | 5,762 us | 5,631 us |
| Observed maximum numeric scan | 9,346 us | 9,306 us |

The 101 real-time samples execute 40 periodic activations with no missed periods.
The numeric values match the saved exact/tolerance checks and are bit-identical
to the baseline report; depths 1–4, actual PC13 sampling, injected input cases,
PA5 electrical safe-output readback, deadline fault and independent-watchdog reset
passed. Heap accounting reports no failures and zero live memory after drops.
The measured MSP reserve exceeds the existing 2,048-byte minimum by 684 bytes;
upper flash exceeds the existing 16 KiB floor. These are observations, not WCET,
worst-case stack or causal compiler-speedup proofs.

Independent read-only reviewer `/root/b_independent_review` verified software
counts/source relationships and independently compared flash snapshots and raw
watchdog/UART records and accepted the final documentation closeout. Its review
is retained at `direct-tests-20261011/independent-scope-closeout.txt`, SHA-256
`1ddf96c645c35977d18751307297c9e762790a3464c20a7fac4a7caf6c403657`.
All seven U checklist items are now checked for this recorded scope. Only the
checklist, this execution record and dependency table are edited after the tested
source freeze; those edits do not change tested code or artifacts.

Native Windows/macOS execution, ESP32-C6 hardware, GitHub Actions execution,
manual board markings/button transitions/optical LED inspection and production
qualification remain unverified. U results qualify the recorded Linux/editor/
WASM/portable and F401 tuple only. Public-release exact-SHA guards remain separate.
Publication preparation also has eight trailing-whitespace notices in the already
tested generated `editors/vscode/media/ladderWebview.js`. No generated asset was
edited after its tests to hide those notices. They remain a pre-publication
format/reproducibility issue; this closeout does not claim a clean full-range
`git diff --check` or a push-ready branch.
No U commit/push was performed. Raw evidence is retained outside Git at
`/home/johannes/projects/.artifacts/runtime-portability-u/`, mirrored on the builder
under `~/.cache/trust-portability-u-evidence/`.

## Publication handoff and next development scope

On 11 October the owner requested live verification that preceding portability
work was pushed/published and, if so, delegated this scope's publication to a
subagent. Live GitHub checks confirm #130/#131/#132 merged, main and the annotated
v0.24.73 tag at `b7d8ffbf242ed5bbef4c971afd43a179757fc1b6`, the Release workflow
successful and GitHub Latest v0.24.73. The existing release skill's read-only
Marketplace query confirms 0.24.73 on darwin-arm64, darwin-x64, linux-arm64,
linux-x64 and win32-x64. Earlier records remain historical; U publication is now
authorized, not already complete.

The delegated publication scope owns preservation of this dirty U checkpoint,
reviewed/generated-file preparation, commit and integration with current main,
the clean exact-SHA guard, affected platform CI and final-image hardware evidence,
push/PR, guarded merge, annotated version tag and full release verification.
Carry preceding publication failure/root-cause corrections into the command map;
do not create another compiler semantic change to accommodate a bad fixture.
Scope tests do not substitute for the final release artifact. Preserve failed
attempts and honor the owner's standing authorization for the publication agent's
necessary tests/retests without repeatedly asking. No guard bypass, capability
reduction or acceptance-margin change is authorized.

H (M2B) is the next development scope: finish shared hosted engine/library and
construction coverage, preparation of the internal register tier, preserved
hosted/debug/HMI/protocol/restart/update workflows and true runtime-only/full/
development dependency compositions. It needs its own bounded scope/batch before
implementation. C6 bring-up remains E when hardware exists; bounded operation
and production qualification remain later scopes. This publication delegation
does not itself start H or any later implementation.

## U publication preparation (11 October)

The publication agent manually recopied canonical AGENTS.md, CLAUDE.md and the
complete skills from `/home/johannes/projects/trust-platform` into the U checkout
and builder copy. Full file hashes match. The pre-publication dirty source and
patch are retained externally in `runtime-portability-u-publication/input/`.
Current fetched main is `b7d8ffbf242ed5bbef4c971afd43a179757fc1b6` / 0.24.73;
U remains 0.24.74. No preceding published implementation is replayed as new U work.

The eight ladder-bundle whitespace notices originate only in esbuild's retained
Konva StackBlur MIT license comment. Independent review accepted the exact-file
`blank-at-eol` attribute; executable bundle bytes, other whitespace checks and
source-stability assertions stay unchanged.

The guard previously ran initial editor npm installation/compilation and formatting
outside its internal target environment. Its reviewed optional `--remote-tool-path`
and `--remote-cargo-home` command forwarding now covers every recorded remote
command. Defaults preserve the earlier transport exactly. Publication uses the
existing isolated U Cargo home, Node 24.21.0, selected tools, Rust 1.99.0 and six
Cargo jobs. Native CC/CXX and cross environments, target leases, the 80 GiB floor
and clean exact-head checks remain enforced. Shared builder tools are unchanged.

One final source-preparation phase formats the workspace, all 283 modified/new
Rust files (including fragments), firmware and both fuzz workspaces, and runs the
new guard regressions. Existing tested assets are preserved. The final command
map allocates guard-owned suites once; uncovered portable/firmware, dependency,
browser/platform and physical final-image prerequisites remain explicit. Preparation
and scope verification do not claim exact-SHA guard, push, CI, merge or release.
Preparation completed seven required stages with no failures: all Rust formatting
left the 283 files unchanged and byte-identical between local and builder copies;
all 41 guard regressions passed. The retained ladder bundle remains
`df2e551576704abb7fc627ba859ed34eb1f3aeebc6327f25d80e49fbae6b13de`.
The agent will retain a reviewed local U checkpoint for the separately authorized
H development, then integrate main and validate the final committed candidate.
