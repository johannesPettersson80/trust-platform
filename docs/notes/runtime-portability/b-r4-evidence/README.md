# B-R4 evidence: shared call-stack correction and F401 fixture

**Current result: the run 8 physical fixture passed on the connected NUCLEO-F401RE.**
The successful second installation and complete UART trace are retained under
`run8/board-attempt2/`; `run8/board-report.json` records the Rust verifier result.
The independent acceptance is `review-run8-physical/review.md`. This is fixture-level
Scope B evidence, not production release or general PLC qualification.

Final frozen source manifest SHA-256:
`a95bcff98b5d4a51e70acf7e29cf0113b585ac69bcfc39c4cc5fbac6b1e8337c`.
Final firmware ELF SHA-256:
`a0e6ed2d7f725fd17746d30c229d02e5b0125c00c494015f3be40322e293ec1e`.
Successful UART SHA-256:
`9601cceef51d46170961c74e0afe2aba33b44250d3795225f6df60055a315704`.

## Current physical evidence

- All 101 main logical samples match the saved A4 oracle. The 25 ms task sampled
  every 10 ms reaches 40 activations, nominal deadline 1,000 ms and zero misses.
- Maximum recorded main scan is 5,798 us; numeric scan is 9,464 us. These are
  measurements of the fixture, not a general worst-case execution-time bound.
- Maximum painted stack use is 13,596 of 16,384 bytes: 2,788 bytes remain, 740
  above the unchanged 2,048-byte minimum. Depths 2, 3 and 4 each use 10,348 bytes.
- Maximum measured heap peak is 28,272 of 73,728 bytes. All three dropped-runtime
  records return live heap to zero. No allocation-free RUN claim is made.
- Upper flash span is 440,032 bytes, leaving 18,720 bytes; sector 0 leaves 224
  bytes. The 16 KiB upper-flash margin remains unchanged.
- GPIO image publication, STOP/fault safe output and the real independent-watchdog
  reset pass. Firmware regions survive separate application installation; checkpoint
  bytes match the original preflash snapshot. `B1,DONE` terminates the trace.
- Manual button transition, optical LED observation and physical PCB/MCU markings
  remain unverified. ESP32 execution, persistence, production installation, M3
  bounded storage and release readiness are outside this result.

## Retained attempts

Counts below are ledger rows, not test counts. A passed software ledger is not
hardware acceptance. Independent steps and separately authorized attempts retain
separate records; no earlier failure has been replaced by the final success.

| Attempt | Required ledger rows | Result and boundary |
| --- | --- | --- |
| Run 1 | 36 pass, 4 fail | Hosted fixture parsing, two lint lanes and flash-margin inspection failed. |
| Run 2 | 40 pass | Corrected software allocation passed; not physical acceptance. |
| Run 3 | 30 pass, 1 fail | Physical instantiation saturated the painted stack; board guard failed. |
| Run 4 | 40 pass | Full allocated software passed after construction-frame ownership changes; no board replay. |
| Run 5 | 12 pass | Preparation/link/inspection measurement only; no native or hardware batch. |
| Run 6 | 12 pass | Measurement-only shallow-state experiment; stack worsened and the source experiment was reverted. |
| Run 7 | 39 pass, 1 fail | Allocated software passed; physical stack reserve was 1,716 bytes, below 2,048. |
| Run 8 initial | 12 pass, 1 fail | Link/inspection passed; reuse-parity precheck rejected changed architecture metadata before tests. |
| Run 8 validation2 | 18 pass | Corrected parity allocation and affected software gates passed; final row verifies the successful physical trace. |

The first run 8 installation failed during programming, before fixture execution.
Its logs and diagnosis remain in `run8/board/` and
`review-run8-install-failure/`. An unexpected reset led to execution of a partly
programmed image; the reset trigger remains unresolved and is not labeled a
watchdog root cause. The second attempt retains the same ELF write order and enables reset/HardFault
vector catch during installation, clearing the catches before fixture execution. It uses the same ELF;
its successful installation does not turn the first attempt into a pass.

## Software coverage and reuse

Run 4 executes 401 core all-feature tests (including four doctests), 362 portable
core tests (including two doctests), 282 i686 tests, 3,841 hosted unit tests and
542 hosted integration tests across 68 binaries. Run 7 reruns 401/362/282 core and
i686 tests and 170 affected hosted integration tests across 20 binaries. It does
not rerun the full hosted unit suite. `review-runs4-7/` reconciles the logs,
source manifests, measurements and exact experiment/reversion chain.

Run 8 changes firmware ownership and its architecture metadata. Validation2
checks 169 unchanged source records and separately pins three changed inputs.
It reuses run 7 core/affected-host evidence and run 4 wider hosted evidence where
applicable; those tests were not newly executed in run 8. Its native tools log
records 85 xtask plus six adapter tests, its firmware library log records two,
and its provenance helper records four. Mutation tooling/contracts record 16/20.
Affected lint, architecture, diagram rendering/drift, formatting, artifact parity
and packing pass. `review-run8-software/` records this allocation precisely.

The supplemental software ledger gained its final board-verification row after
software approval; earlier review/approval hashes refer to the ledger as it
existed at approval, not to an undocumented replacement of software results.

## Retention and identities

`artifact-sha256.json` covers every retained file in this directory except itself,
including this README and `external-artifact-sha256.json`. Scripts are retained
as execution evidence, not commands to rerun automatically. Historic review
identities pin their historical source or evidence; the outer artifact index pins
the current retained copies.

Large raw files remain outside the repository at
`/home/johannes/projects/.artifacts/runtime-portability-b/b-r4/`.
`external-artifact-sha256.json` maps paths relative to that external directory to
SHA-256 values verified from actual local files. It covers excluded ELF/map/bin
files, complete disassemblies, source patches/index copies, lockfile copies and
duplicate aggregate console logs. Native suite outputs, failure ledgers, unique
installation logs, compact prologues and reviews remain here. Existing large
files were removed from this retained subtree only after an exact external copy
was verified; no raw evidence was destroyed.

Some original scripts mention source archives retained only on the builder.
Those references are historical remote locations, not claims that an archive
was copied locally. The external index contains only files verified present in
the local external directory; it does not invent local records for remote-only
archives. No source build, test, formatter, linker or hardware execution was
performed by the evidence-curation step.
