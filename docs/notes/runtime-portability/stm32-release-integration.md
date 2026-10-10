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
| Dependency resolution/LSP prepush parity | Remaining hygiene/IEC/diagram contract, Windows GNU LSP compile with CC/CXX unset, eight one-attempt mesh/TLS iterations; guard overlaps deduplicated. |
| Tool/provenance and advisory metadata | Native xtask tests via guard; applicable Python gate/contract/provenance suites; metadata statuses do not claim an unexecuted proof. |
| MSRV and Rust API documentation | Rust 1.95 locked all-target checks, stable all-feature Rustdoc with denied warnings, matching the distinct CI lanes. |
| Public documentation | Separate clean candidate docs checkout: media/IA/links/examples/OpenOT, strict MkDocs, assets/search and entrypoint assertions. |
| Physical candidate | Root executes reviewed F401 installation/UART capture only after software and ELF pass. Rust verifier checks final image traces, memory, depth and watchdog; acquisition exit status alone is insufficient. |
| GitHub/release | Push once passing artifact and supplement; collect all current-head jobs and retry-rescued artifacts before corrections; guarded merge, annotated version tag on green main, complete release/assets/Marketplace verification. |

Builder source is isolated at the same named checkout. Reuse the idle leased target
`/mnt/HC_Volume_107089260/builder-storage/cargo-targets/trust-portability-a1-integration`.
The builder currently reports Rust stable 1.99.0; the portability and firmware lanes
remain pinned to Rust 1.95.0, edition 2021. Native compiler overrides are unset for
cross checks. Raw logs, binaries, maps and source archives remain outside Git under
`/home/johannes/projects/.artifacts/runtime-portability-publication/` and the builder's
corresponding task-owned evidence directory. Preserve every failed/unrun stage.

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

Current stage: final reviewed publication record ready for commit and exact-SHA batch. No guard, final-image board replay or push
has occurred.


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
all product/fixture/firmware bytes are unchanged. The successor exact-head guard
remains required before its push. GitHub native platform checks, guarded merge
and full public release verification remain separate stages.
