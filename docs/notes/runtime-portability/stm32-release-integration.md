# Shared runtime and F401 release integration

The user authorized a publication agent to integrate the verified A3, A4 and Scope B
checkpoints, validate the exact committed candidate, push, guarded-merge and verify
its release. Necessary reviewed correction/revalidation cycles are authorized by
the subsequent instruction to run as many tests as needed. This does not authorize
later ESP32 hardware, modernization or production qualification work.

The isolated checkout is `/home/johannes/projects/trust-platform-portability-release`,
branch `integrate/runtime-portability-stm32`, based on released main
`3bed89b47b4e0ed8ecc11410b592228fff5f0112` (A2/#131, version 0.24.72).
The clean source checkpoint `feat/runtime-portability-b` at
`bc16f252afbb8b93705a57e9a79168f6426c25c0` remains unchanged.
Only the two A3 commits, A4, Scope B and its version metadata were replayed.
Already merged A1/A2 and their publication corrections were not replayed.
Canonical agent files and the entire skills directory were manually copied from
`/home/johannes/projects/trust-platform`; 21-file parity holds in both checkouts.

## Integration decisions

Conflicts were limited to the checklist checkpoint, changelog and version fields.
The later Scope B checkpoint is retained, all upstream changelog fixes remain,
and the candidate version is 0.24.73. Main's security, fleet shutdown, LSP deletion,
Git-marker and current-proof metadata corrections remain. The current canonical
workflow improvements already exist on main, so the old workflow-only commit
was omitted. Core, adapter, firmware and xtask source match the verified Scope B
checkpoint byte-for-byte before final preparation; the package-lock retains
main's concurrently security update.

The earlier physical evidence belongs to its recorded 0.24.71 image. It is preserved,
but a fresh final-version firmware link, inspection and physical replay are required
before this candidate is pushed. No function cuts, threshold reductions, toolchain
changes or overflow-check changes are permitted. Required headroom remains 16 KiB
upper flash and 2 KiB measured MSP, with the 72 KiB heap and 16 KiB MSP partitions.

## Final validation command map

| Requirement | Command and prerequisite |
|---|---|
| Reviewed final source preparation | Builder stable formatter plus explicit touched include fragments and firmware formatting (the completed preparation used builder stable; the final firmware formatting check uses Rust 1.95); active provenance refresh, architecture/full-map then canonical diagram rendering. Copy back and review before commit. |
| Clean exact-SHA release artifact | Canonical `release_candidate_guard.py prepare --intent feature`, clean builder candidate and released origin/main base; selected target must have 80 GiB free. |
| All native workspace/default-core/hosted vertical/LSP/debug tests | Guard `just test-all`, once. This is not repeated in the supplement. |
| VS Code compile/lint/native UI tests and capture lifecycle | Guard's version-change stages; short task-owned Unix temporary path. |
| Clippy, native/Windows runtime warnings, root and firmware supply-chain locks, architecture | Guard-owned steps; firmware lock audit is included by shared supply-chain script. |
| F401/C6 portable graphs and isolated no_std behavior | Rust 1.95 locked no-default core tests, both MCU checks, no-dev feature trees. |
| Actual 32-bit execution | Rust 1.95 no-default lib plus five selected source-free/foundations/load/frame suites on i686 musl using rust-lld and self-contained linking. |
| Firmware and adapter checks | Rust 1.95 firmware/native tooling regressions, adapter cross-check and firmware release Clippy. |
| Final release firmware identity/size | Canonical `cargo +1.95.0 xtask portability build-firmware`, retained map and `portability inspect`; no speculative linker knobs. |
| Dependency resolution/LSP prepush parity | Remaining hygiene/IEC/diagram contracts and Windows GNU LSP/xtask test compilation with CC/CXX unset; unchanged mesh/TLS retains its prior source-specific eight-iteration proof, and guard overlaps are deduplicated. |
| Tool/provenance and advisory metadata | Native xtask tests via guard; applicable Python gate/contract/provenance suites; metadata statuses do not claim an unexecuted proof. |
| MSRV and Rust API documentation | Rust 1.95 locked all-target checks, stable all-feature Rustdoc with denied warnings, matching the distinct CI lanes. |
| Public documentation | Separate clean candidate docs checkout: media/IA/links/examples/OpenOT, strict MkDocs, assets/search and entrypoint assertions. |
| Physical candidate | Publication agent executes the reviewed F401 installation/UART capture only after software and ELF pass. Rust verifier checks final image traces, memory, depth and watchdog; acquisition exit status alone is insufficient. |
| GitHub/release | Push once passing artifact and supplement; collect all current-head jobs and retry-rescued artifacts before corrections; guarded merge, annotated version tag on green main, complete release/assets/Marketplace verification. |

## PR #132 complete correction allocation

The final metadata head `75b42a667` passed its exact-SHA builder guard and fresh
firmware inspection; that ELF is byte-identical to the physically qualified
`0597a300...` image. PR #132's complete first CI attempt has 26 terminal checks.
Linux and every area check passed. Windows failed the native remapping assertion
because it assumed Unix separators; macOS failed the actual supply-chain wrapper
because the system Bash 3.2 lacks `mapfile`. Both assertions failed identically on
the automatic second attempts; the aggregate release report failed consequently.
The automatic code review additionally identified image carry-over overwriting
direct-I/O restart initialization. Security review completed without findings.

The publication correction covers all three owning causes together: native path
construction and assertions, Bash 3.2-compatible audit argument transport with
unchanged four-check policy, and staged process-image carry-over before ordered
restart initialization. Required native cases cover empty and populated audit
exceptions, both restart modes, flat/hierarchical image targets, overlapping action
order, unconfigured values and failed replacement. At that allocation, all remained unrun pending complete review, formatting
and freeze; the corrected run-5 outcomes are recorded below.

The owner explicitly authorized as many necessary publication tests and retests
as required. After independent review, the consolidated correction uses the
exact-SHA guard for shared/native/hosted/VS Code/architecture/supply-chain checks,
plus deduplicated Rust 1.95 no-default/i686/MCU and firmware checks and updated
diagram rendering/drift. The changed engine requires a fresh retained linked map,
ELF inspection and physical replay; earlier image proof is historical. Functions,
overflow checks, toolchain and 16 KiB flash/2 KiB measured-stack floors are preserved.
No partial-results correction push or merge is allowed.

Run 5 final preparation passed all seven retained rows: environment, workspace
formatting, explicit fragment/firmware formatting, active provenance discovery,
full-map architecture, diagram rendering and complete diff retention. It changed
only the reviewed Rust formatting and the rendered execution diagram/manifest.
The completed source and scripts have independent read-only acceptance; their
source review does not claim compilation or execution of the new regression.
At the pre-validation checkpoint the correction was pending exact-SHA software
validation and fresh physical proof; its completed run-5 outcome is below.

Run 5 preparation records the complete formatter/provenance/full-map/render diff
before the correction commit. The new guard owns all default native suites; the
supplement separately executes Rust 1.95 no-default core tests, the selected i686
lib/five suites, both MCU checks/no-dev graphs, firmware tests/adapter/Clippy, the
MSRV and Rustdoc lanes, Windows xtask test compilation with native CC/CXX unset,
and final format/diagram/evidence checks. Strict public-docs checks use a clean
checkout at the same committed SHA. The final-image suffix forces a fresh pinned
link, retains its map/ELF as required stages, inspects only after retention, and
packs the saved application. Hardware begins only after every required software,
docs and ELF stage passes; it qualifies that actual newly linked ELF. No old ELF
identity is assumed for this engine correction.

Builder source is isolated at the same named checkout. Reuse the idle leased target
`/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a1-integration`.
The builder currently reports Rust stable 1.99.0; the portability and firmware lanes
remain pinned to Rust 1.95.0, edition 2021. Native compiler overrides are unset for
cross checks. Complete CI job logs, binaries, maps and source archives remain
external under
`/home/johannes/projects/.artifacts/runtime-portability-publication/` and the builder's
corresponding task-owned evidence directory. Selected test outputs and small proof
files are also retained byte-for-byte in the tracked publication-evidence tree.
Preserve every failed/unrun stage.

Independent root review accepted the integrated source at 07e01c777. It verified all
286 core/adapter/firmware/xtask files against Scope B and checked the preserved main
corrections. Final preparation passed all seven ledger rows: environment, workspace
formatting, explicit fragment/firmware formatting on builder stable 1.99, real active-selector discovery,
architecture/full-map, diagram rendering and diff retention. It changed no tracked
source or generated bytes. The source checkpoint and previous physical evidence
remain preserved; the final committed candidate still requires its fresh guard,
portable supplement and hardware replay.

Root accepted the source, command map and scripts after preparation. Documentation
validation also checks final source cleanliness after generators/build. The final
source remains unchanged; the five-suite i686 allocation is explicitly selected,
not a claim of complete 32-bit corpus coverage.

Historical stage at `5b7f2ea55`: final reviewed publication record ready for its
first exact-SHA batch; no guard, final-image replay or push had occurred then.
The active stage is the PR #132 correction allocation above.


## First publication batch and unsafe inventory correction

Run 1 at 5b7f2ea55 retained one required guard failure: the external AST scanner
excluded firmware and all test files while the reviewed unsafe-site policy
registered eleven firmware and seven hosted allocation-probe sites. Each site
exists at its exact line; deleting registrations would hide the owning defect.
All 22 required supplemental gates and their advisory metadata check passed,
as did all twelve public-docs steps. The isolated native no_std suite passed
362 tests; the selected i686 lib/five-suite lane passed 270; firmware tests passed
two. The fresh 0.24.73 image passed inspection with 18,720 bytes upper flash free.
Hardware remains unrun because the guard failed. Guard Clippy, full workspace,
MP parity and final cleanliness were unrun after the architecture prerequisite.
VS Code passed 519 tests, with formatting, cross warnings and both-lock supply
chain checks passing before that failure. Complete raw ledgers remain outside Git.

The correction moves the touched external scanner inventory and admission decision
into Rust xtask. It reuses the doctor's existing crate/third-party/firmware file
inventory, preserves the old external test exclusions exactly, and additionally
scans only exact registered excluded test paths. The existing six AST patterns,
raw/normalized/unregistered/missing/unsupported reports, exact line ownership
and delegated-path conditions remain. Shell is a command wrapper. Native fixtures
cover firmware and exact registered test inclusion, unrelated test exclusion,
unregistered firmware rejection, missing/shifted registrations, and malformed
external locations. No unsafe registrations or delegations are removed.

The correction preparation compiled the new Rust gate and formatted it, but the
full-map doctor caught the former single-file module owner path: adding a child
module changes its modeled path from `xtask/src/full_map.rs` to `xtask/src/full_map`.
Only that existing owner-path binding is corrected; the size allowance, rationale,
unsafe sites and delegation rules remain unchanged. Diagram rendering stayed
unrun until this prerequisite is corrected. This failed preparation is retained.

Root independently accepted the formatted correction and the sole owner-path
rebind. The recorded preparation suffix passed architecture/full-map and canonical
diagram rendering; final generated outputs were copied back. No runtime, core,
adapter or firmware source changed during the correction. The next exact-SHA
guard covers the previously unreached Clippy/workspace/MP stages and the actual
external AST gate. A focused supplement executes the new Rust gate regressions,
checks the wrapper syntax and diagram drift, then relinks/inspects the final
firmware tuple. Portable/32-bit/MCU/native firmware/MSRV/Rustdoc/TLS/provenance
proof from run 1 remains source-specific reused proof, not a claimed rerun.


## Second publication batch and empty-match tool contract

Run 2 at fed5dc457 passed VS Code, formatting, cross warnings and supply chain,
but its actual external AST gate rejected a normal empty match chunk: ast-grep
0.42.1 returns exit 1 with the complete JSON array `[]` and empty stderr when a
chunk has no matching unsafe construct. The independent raw tool observation is
retained as `run-2/no-match-contract.json`. The four native inventory/admission
regressions and all eight focused suffix steps passed; all later guard stages
remain unrun. No physical replay occurred. The final firmware inspection passed.

The corrected Rust normalizer accepts that exit status only after parsing a
complete empty JSON array and confirming empty stderr. Nonempty facts under
exit 1, malformed/missing JSON, stderr errors, other statuses and signal termination
remain failures. A new native regression asserts every branch. Ownership
registrations, delegated paths, firmware and runtime source remain unchanged.

Root independently accepted the normalizer and requested success/nonempty and malformed-success assertions, both included. Run-3 preparation passed all seven steps; only formatting of this reviewed correction changed. No runtime or firmware source changed. The correction is ready for its exact-SHA guard and final-image supplement; hardware remains gated on software approval.


## Reviewed publication proof and final evidence freeze

The corrected exact-SHA guard passed at
`ca205132e0f5626046353ffbe13fe402246da38f`: all required commands passed,
including 8,689 native assertions (315 result blocks; 24 ignored), 519 VS Code
assertions, Clippy, both-lock supply chain, cross warnings, real AST ownership,
MP parity and source cleanliness. Planner/catalog results remain advisory.
The final-image retention suffix passed seven steps; it corrects the map-copy
collector's omitted failure accounting without changing product source.

The final 0.24.73 image passed a fresh physical replay with the reviewed B-R4
protocol. The Rust verifier and independent root reconciliation agree on all
101 oracle samples, 40 activations/zero misses, numeric checks, GPIO safe outputs,
depth/IRQ probes, memory margins and IWDG reset/DONE. ELF and raw UART identities,
retained ledgers and explicit limitations are in [publication evidence](publication-evidence/README.md).
The new closeout consists only of documents and byte copies of that evidence;
all product/fixture/firmware bytes were unchanged by that closeout. Its successor
exact-head guard passed at `75b42a667`, and PR #132 was pushed. Its first native
platform checks and automatic review produced the complete correction allocation
above; guarded merge and full public release verification remain pending.


## Corrected candidate qualification and evidence successor

Run 5 at `a8937a07c12042fa655297a53338fa787c1a978c` passed the exact-SHA
guard: 8,690 native tests, 315 result blocks, 24 pre-existing ignored and 519
VS Code tests; every required command passed. The independent preparation/source
review was completed before the commit. The 20-command pinned supplement, its
advisory metadata step and twelve strict documentation steps passed. The new
ordered direct-I/O restart case executed successfully in default native,
no-default and selected i686 lanes. Native Windows/macOS execution still belongs
to the upcoming corrected PR head, not these Linux cross checks.

The fresh seven-step pinned image suffix passed; inspected ELF
`4761d2bc5153b3892da352ca1f0be1f7041f421457158bfc2a9682ddbbb39a61`
passed physical install/replay and the separate Rust UART verifier. Actual UART
`1a15c7b9c615e6893699addc04988f836f6dbc84e7a0135eb1c64df5bd18171b`
again has 101 oracle samples, 40 activations/zero misses, numeric/GPIO/depth/IRQ
and complete IWDG reset/DONE. Flash free is 18,720 bytes, heap peak 28,272 of
73,728, and MSP peak 13,596 of 16,384 with 2,788 free. Region/checkpoint
comparisons and hardware protocol readbacks passed. These are fixture measurements
and bring-up qualification; manual observations and production/C6 gates stay open.

The retained run-5 closeout modifies only documents/evidence. Product Rust, saved
artifacts, firmware configuration and version remain byte-identical to tested
`a8937a07c`. Independent read-only reconciliation accepted all retained hashes, source bytes,
raw software counts and physical preservation/readback proof before this metadata
commit. Its clean successor requires another exact-SHA guard and fresh link/image-identity
check before the correction push. All first CI failures and prior checkpoints
remain retained. PR #132's green native platform checks, guarded merge and full
release verification remain pending, with existing correction/retest authority.

## Metadata-successor failure and owned transport-fixture correction

The exact-SHA guard at `15391576e1387c00174d48b495780e54ca42f203`
failed one pre-existing runtime unit case:
`service_retries_unreachable_influxdb_during_initial_open` observed `Starting`
and zero retries at its one-second timer. The other 3,858 runtime unit cases
passed. All other reachable guard commands passed, including 519 VS Code cases.
The native workspace command stopped at that failed library; its remaining
runtime binaries/integration targets and remaining workspace packages were then
collected on the same frozen SHA with `--no-fail-fast`. Both commands passed,
with no additional failure. The complete guard artifact, native outputs, commands
and ledger remain under the external publication `run-6` evidence directory.
The independent source trace is `run-6/review/openot-fixture-trace-review.md`.

Specification 33's startup and typed transport-error contract is already covered;
this is a test-harness correction, not a changed retry policy. The old fixture
released its ephemeral port and timed asynchronous TLS/spool preparation as if an
actual remote failure had already happened. The exact slow preparation subphase
was not measured. The shared replacement owns its listener until teardown,
observes actual request/ClientHello bytes, and transfers the held connection to
the test. Startup must have returned while that peer is held, with `Starting`,
zero retries and zero committed cursor/documents. The existing post-start record
publication stays in place. Direct socket closure then starts the unchanged
one-second retry assertion window; retry count, pending/head/cursor and shutdown
preservation assertions stay intact. The standalone HTTP classification case
uses the same real disconnect and keeps its typed connection-error assertion.
No mocks, dependencies, production error reclassification or longer behavior
window were introduced. Listener/read waits are interruptible and teardown joins
the fixture thread, including during assertion unwinding.

Run 7 command allocation: independently review the complete fixture/module/test
correction; prepare the stable formatter plus touched fragments/pinned firmware
formatter, architecture/full-map and derived diagrams on the builder; review and
commit the prepared source. Run the canonical exact-SHA guard for all native,
Clippy, cross-warning, supply-chain and VS Code commands, with no duplicate full
native batch. Fresh pinned final-image linking, required retention and inspection
must prove byte identity against physically replayed ELF
`4761d2bc5153b3892da352ca1f0be1f7041f421457158bfc2a9682ddbbb39a61`.
If identity holds, that actual physical replay remains applicable; otherwise a
fresh reviewed hardware replay is required after software/ELF prerequisites.
Unchanged portable core, public-doc and fixture bytes retain their explicitly
recorded run-5 scope evidence. Native Windows/macOS still require the corrected
PR checks. No run-7 result is claimed before execution; unlimited reviewed
correction/retest authorization remains in force, and merge/release are pending.

Run-7 preparation passed all five stages: environment, the single final formatter
(including explicit fragments and Rust-1.95 firmware), architecture/full-map,
derived diagrams and complete source-diff retention. Derived diagrams did not
change. Formatter output reordered imports in the new test module and rewrapped the
remaining imports in `service_tests.rs`; the prepared eight-path correction
requires final independent reconciliation before commit. Native assertions remain unrun on this correction until the exact-SHA guard.
