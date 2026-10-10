# Scope B — NUCLEO-F401RE execution record

Status: **run 1 failed; physical execution unrun because the firmware does not fit**.
Two tooling corrections are prepared and unverified; no second run is authorized.
Authority: specification 34 §13.1 and Appendix D.1. This record does not grant retries.

A4 is committed locally as `df427259cc387a7a79fb81132be23e48ea1493d4`.
B branch `feat/runtime-portability-b` uses that exact base. The separate builder worktree
is detached at the same commit, with the uncommitted B overlay. Canonical agent files
come from the primary checkout; all 21 files byte-match on both destinations.

The owner requested commit and the next step, then explicitly directed remote tests
while away from home. No printed marking or manual button question blocks this batch.
Physical PCB/package markings, manual button transitions and optical LED observation
remain unverified, distinct from electronic identification and GPIO pad readback.

## Read-only inventory

Local Raspberry Pi USB ST-LINK/V2.1 `0483:374b`, serial
`0671FF575755846687183960`; UART by-id
`usb-STMicroelectronics_STM32_STLink_0671FF575755846687183960-if02`.
OpenOCD read-only inventory reported Cortex-M4 r0p1, DBGMCU ID `0x10016433`,
flash `0x200` KiB and UID `00410016 30395119 35363638`.
Raw inventory is outside the checkout at
`/home/johannes/projects/.artifacts/runtime-portability-b/inventory/`.
These observations identify the connected device electronically; they are not a
firmware test or an observation of the printed PCB revision.

## Composition and proof boundaries

- Rust 1.95.0, edition 2021, resolver 2; no modernization scope included.
- Workspace platform adapter; standalone firmware workspace uses the same core source.
- HSI PLL configured to 84 MHz; TIM2 1 MHz, SysTick 1 kHz, USART2 230400 baud.
  No independent calibrated frequency claim.
- 72 KiB allocator, 16 KiB MSP, at least 2 KiB measured stack/IRQ margin; native call
  depth at most four including the program. Cumulative logical preparation charges
  and actual simultaneously live heap are reported separately.
- Sector 0 vectors, sector 1 application bundle, sectors 2–6 code, sector 7 reserved.
  Flash inspection rejects ELF loads in the application/persistence regions.
- Main and numeric A4 artifacts remain byte-identical. A third source-authored STBC
  exercises PC13/PA5 process-image binding and nested calls, because the A4 artifacts
  contain no I/O bindings. Injected process-image samples are labeled injected.
- A genuine watchdog reset requires both the backup-register cookie and RCC IWDG flag;
  unexpected reset/failure stops the run rather than retrying it.
- Run 1 proves the planned firmware does not fit flash. Live heap and stack/timing fit remain unknown until a valid linked image and board measurements exist.
  Failure does not authorize enlarging limits or removing the fixture.

## One consolidated batch command map

All Cargo commands run on `trust-builder` through the target lease, with a task-owned
volume target and TMPDIR. Keep host compiler flags out of MCU and Windows cross checks.
Record compiler/target versions, source manifest, dependency locks and complete exit ledger.
The batch has preparation, frozen validation, and dependent physical execution phases.
No source correction or command retry occurs while it runs.

| Step | Command or evidence | Dependency |
|---|---|---|
| Preparation | Resolve only required new dependency edges; format root and standalone firmware once; generate GPIO artifact once with `cargo run -p trust-runtime --example portability_gpio_fixture`; freeze complete source/generated inputs | Independent review complete |
| Host checks | `cargo test -p trust-platform-stm32f4 -p xtask --no-fail-fast`; standalone firmware `cargo test --lib --target x86_64-unknown-linux-gnu`; runtime GPIO test plus required api_smoke/debug_control/complete_program/runtime_reliability | Frozen source |
| MCU | Platform check and standalone release firmware build for `thumbv7em-none-eabihf`, linker map retained | Frozen source |
| Quality | Changed-package Clippy, runtime cross-target warnings, supply chain, architecture full-map, then diagram render/drift; formatting and diff checks | Diagrams depend on architecture |
| Packaging | Rust `cargo xtask portability pack`, then `inspect` linked ELF; preserve SHA-256 for ELF and every application artifact | Producer/link success |
| Physical | Back up current 512 KiB flash and reset/option evidence externally; program/verify firmware and separate application sector; capture bounded UART from before reset; no mass erase or option change | ELF inspection passed |
| Device result | Rust `cargo xtask portability verify` captured UART against the unchanged A4 CSV and numeric contract | Complete physical capture |
| Closeout | Independent source/evidence reconciliation, measured sizes/heap/stack/timing table; failed/unrun checks explicit | All reachable batch steps finish |

Required physical evidence: 101 main samples at 10 ms for one second, 40 nominal 25 ms
activations and zero overruns; numeric values and scan maxima; physical/injected GPIO
records; PA5 high-to-low STOP and FAULT transitions; call-depth 1–4 stack measurements
with advancing interrupts; preparation/instantiation/run/drop heap phases; watchdog reset.
A single device trace is finite execution evidence, not a proof of all stack paths or WCET.

## Review and validation results

Run 1 completed on the builder. Raw data remains outside Git; curated text/hash records
are retained without source tarballs or firmware binaries. See the result below.

Independent source review completed before the batch. Reviewed platform HAL/PAC APIs,
startup/linker/allocator/IRQ ownership, watchdog protocol, host verifier and ELF checks.
Three findings were corrected: enforce full UART lifecycle ordering, measure the final
boot stack peak after every GPIO fault/stop probe, and check the actual Reset vector
against the Thumb ELF entry. A reordered-record regression accompanies the ordering fix.
The reviewer confirmed all three corrections. Script review additionally requested
bounded OpenOCD invocations and a separately archived pre-reset UART drain; both are
included. These reviews are source evidence, not compilation or physical proof.

Batch preparation began with one Rust 1.95.0 formatting pass for each workspace. Both
completed successfully. Formatter output was copied back; exact unsafe-site line records
were updated to that formatted source before the validation freeze. Dependency resolution
added only the required platform/tool edges to the root lock; standalone firmware has
its own tracked lock. No build or test preceded this preparation.


## Run 1 result — failed, no automatic retry

Builder batch: 10 October 2026, 11:51:05–12:05:50 UTC after the single formatting
preparation. The 22-step ledger records **18 passed and four failed**. The shell
orchestrator finishing successfully is not a passing batch: the ledger owns the outcome.

| Evidence | Result |
|---|---|
| Host GPIO execution and required runtime vertical | 29 native tests passed across five binaries; none failed, ignored or filtered |
| Standalone application-bundle decoder | One native test passed |
| Adapter and xtask native tests | Compilation stopped on the oracle include path; no passing native assertions claimed |
| F401 adapter compilation | Passed `thumbv7em-none-eabihf` |
| Firmware release link | Failed: FLASH overflow **205,024 bytes**; no valid ELF |
| Firmware Clippy | Passed |
| Root affected-package Clippy | Failed on manual range comparison in the new ELF checker; test oracle include also fails in the test target |
| Runtime host and Windows cross warnings | Passed; this is compilation, not native Windows execution |
| Dependency gate | Failed: RUSTSEC-2026-0110 marks transitive bare-metal 0.2.5/1.0.0 unmaintained; license/source/ban checks passed. Subsequent audit stage unrun because cargo-deny failed |
| Architecture, canonical diagrams/drift, both formatting checks, diff | Passed |
| Bundle packing / unchanged A4 artifacts | Passed; 14,760-byte bundle, original main/numeric bytes unchanged |
| ELF installation inspection, flash, UART/device oracle | Unrun: no linked firmware. Board firmware untouched |

The dependency gate ran 50 release-tooling tests before cargo-deny; they are separate
from the 30 passed native Rust behavior tests above.

Frozen source manifest: 6,607 path hashes, SHA-256
`189912106e71777133bfe0189736d2dbe32339775870af5baab20183b2595edf`.
Failed linker map SHA-256:
`d12d328a476fa000979f789e3154e10d28ccf22506dbd9946051c6a815f60992`.
Curated evidence: [directory](b-evidence/README.md). Raw source, map, command logs,
full manifest and generated package remain under
`/home/johannes/projects/.artifacts/runtime-portability-b/run-1/`; builder raw cache is
`~/.cache/trust-portability-b-evidence/run-1/`.

## Measured flash blocker and next decision

The failed map contains `.text` **487,876 bytes**, `.rodata` **77,580 bytes**, and
`.bss` **73,788 bytes**. Debug sections are excluded from the flash numbers.
Text plus read-only data alone is **565,456 bytes**, already beyond the entire
512 KiB device before vectors and application storage. Reclaiming the reserved
persistence sector therefore does not solve the problem. Static RAM fits its
reservation, but that proves neither live heap sufficiency nor measured stack margin.

Read-only symbol attribution identifies the shared validator (60,728 named text bytes),
engine state (42,088), preparation (38,772), dispatcher (38,412), decoder (15,058),
firmware composition (15,612) and board adapter (2,154). These family labels are an
analysis aid, not additive full-section accounting.

Two owning improvements are worth designing: avoid encoding again on the already
bounded/decoded `PreparedModule::from_bytes` path while retaining struct-built input
checks; and use one compact, identical IEEE CRC implementation for the embedded
composition. The unnecessarily reachable encoder accounts for about 7,002 named text
bytes and CRC32fast's table for 16,384 read-only bytes. Their identifiable cost is only
about 23 KiB, not the required 200 KiB. No claim that these changes alone make F401 fit.
Runtime filtering of the standard library would reduce registry RAM but retain every
linked function pointer; it is not a flash solution. Substantial shared representation
work or an explicitly admitted compile-time capability profile needs a separately
reviewed footprint plan. A larger-flash target is an alternative platform decision;
no target change or semantics reduction has been made.

## Corrections prepared after the batch

- Oracle include is now rooted at `CARGO_MANIFEST_DIR`, so moving the test module no
  longer changes its fixture path. No assertion or oracle bytes changed.
- ELF section range membership uses `Range::contains`; bounds are unchanged.
- These two source edits have not been compiled, formatted or tested again. Frozen run-1
  sources are retained independently. Rendered diagram output was copied back unchanged.
- No flash-limit enlargement, fixture reduction, validator removal, dependency suppression,
  commit, push or second validation run occurred.

[RUSTSEC-2026-0110](https://rustsec.org/advisories/RUSTSEC-2026-0110) is an informational
unmaintained advisory, not evidence of an exploited vulnerability. Published-source
inspection found even Cortex-M 0.7.9 still depends on bare-metal 0.2.4-compatible versions,
and the latest stable STM32F4 HAL 0.23.0 depends on bare-metal 1. No dependency change or
policy exception is silently substituted for resolving/reviewing this blocker. A future
batch must also explicitly audit the separate firmware lock graph; root-workspace
cargo-deny does not cover all standalone-only dependencies.

Root architecture graph evidence excludes the standalone firmware package. New
firmware allowed/forbidden edge rows document intent; they are not enforced by that
root metadata graph. The safety-source scanner does include firmware, and current
standalone manifest boundaries were independently inspected. Keep these proofs separate.

Next gate: decide and implement a credible footprint path and dependency disposition,
review the complete corrections, then obtain explicit authorization for another batch.
Scope B remains open. Physical marking inspection is not the blocker.

Independent post-batch reconciliation confirms 22 steps, 18 passes, four failures,
30 passed native assertions, and the exact failed-map sizes. All 6,607 frozen paths
remain available; of 2,041 Rust paths, only the two documented tooling corrections
changed. Reverting those expressions reproduces their frozen hashes. The correction
review is [retained here](b-review/independent-run-1-evidence-and-corrections.md);
it verifies source differences, not execution of the fixes.
