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
