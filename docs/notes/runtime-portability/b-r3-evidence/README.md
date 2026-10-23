# B-R3 retained evidence

M1 and M2 are the two authorized size measurements. M2 is the final measured
candidate, frozen at manifest `a27301e8e9b8d89f9db9b34329508a0274c93b44b04c56768a122b0f8703cf28`.
The consolidated batch recorded 38 required PASS rows, one required FAIL row
(hosted integration: two assertions), and two advisory PASS rows, including preparation.
There were no retries. Hardware is unrun because software prerequisites failed.

M2 upper span is 441,920 B, leaving 16,832 B (448 B above the 16 KiB floor).
ELF inspection passes. Map flash section totals omit the 12-byte vector/data gap;
ELF load bytes include it. Neither establishes board execution or stack safety.

| Suite | Passed | Failed |
|---|---:|---:|
| Core all features | 387 | 0 |
| Core portable | 348 | 0 |
| Core i686 | 268 | 0 |
| Hosted unit | 3,841 | 0 |
| Hosted integration, 68 binaries | 536 | 2 |
| Platform and xtask native | 91 | 0 |
| Firmware library | 1 | 0 |
| Provenance helper | 4 | 0 |

Failed assertions are the exact 32-poll deadline regression (observed 33), and
2,000 distinct input bindings within the default work allowance (ExecutionTimeout).
Neither assertion is weakened. Corrections authored after this freeze are unverified
and require a separately authorized batch and new firmware link before installation.

Raw maps, ELF binaries, temporary indexes and source archives are outside git under
`/home/johannes/projects/.artifacts/runtime-portability-b/b-r3/`, with builder originals
under `~/.cache/trust-portability-b-evidence/`. External firmware identities are pinned
in `external-artifact-sha256.json`; retained text/JSON/scripts are pinned in
`artifact-sha256.json`. Reviews are source evidence, distinct from execution.


## Authorized run 2

Owner authorized “test and remeasure then run the hardware tests”. Frozen manifest
`8b68c8dc3978de9049405e1ecf001d581558319cf739c0e8bfbde9483cb4d25d`
contains 812 records. The 19:32:01–19:38:22 UTC batch completed with 39 required PASS,
one required FAIL (ELF headroom), and two advisory PASS rows including preparation.
No retries occurred. Both previous behavioral regressions and the added cases pass.

| Suite | Passed | Failed |
|---|---:|---:|
| Core all features | 392 | 0 |
| Core portable | 353 | 0 |
| Core i686 | 273 | 0 |
| Hosted unit | 3,841 | 0 |
| Hosted integration, 68 binaries | 539 | 0 |
| Platform and xtask native | 91 | 0 |
| Firmware library | 1 | 0 |
| Provenance helper | 4 | 0 |

The new image links with upper span 442,720 B and free space 16,032 B, **352 B below**
the unchanged 16 KiB floor. Sector 0 retains 736 B free. The inspector rejects this
candidate, so no hardware phase ran. Every other required gate passed, including
Clippy, cross-target warnings, both lock audits, architecture, diagrams and formatting.

A subsequent layout-only correction selects two existing immutable 256-byte tables
for sector 0 and asserts their combined size. No Rust function or test changed after
this run. Its predicted 512-byte relocation remains unverified until another
explicitly authorized link and inspection. It is not the measured run-2 firmware.


## Authorized focused run 3 and physical board attempt

Run 3 source manifest: 944 records,
`49bf385446ebcea7bf50a440c113555b9d2c92197fd0d0c10c392b1a59f58c8e`.
Canonical link and unchanged ELF inspection pass. Upper span 442,208 B leaves
16,544 B, meeting the 16 KiB floor by 160 B. Sector 0 has 224 B free. The two
unchanged lookup tables moved exactly 512 B; Rust sources remain run-2-identical.
ELF SHA-256: `87d622c71fa6f6dab01d13caef9db2671a174f9ccb575b0398d76389b8f61c5b`.

The NUCLEO-F401RE installation and one bounded capture ran. Full original flash was
backed up externally. Firmware/application verify succeeded; firmware regions and
both checkpoint sectors were byte-identical around the separate application write.
The script's exit 0 establishes acquisition, not runtime acceptance.

Hardware qualification **failed**: the unchanged Rust verifier rejects insufficient
measured stack headroom. Main instantiation reports 16,272 B used / 112 B free;
main-run saturates the 16 KiB painted region. Depth probes report 9,680 B at depth 1,
16,344 B at depth 2 and saturation at depth 3, followed by a panic record. Saturation
is a lower bound, not proof that execution stayed within the reservation. The panic
message is deliberately capped at 64 characters; its implausible metric values are
not reliable allocation evidence. The exact panic cause remains unresolved.

The board was halted without a reset/retry. MSP was `0x20012130`, 7,888 B below the
reserved floor `0x20014000`. Depth 4, the later high-output STOP/FAULT checks and the
intended watchdog-reset completion were not reached. No overall hardware pass,
worst-case timing qualification, physical button actuation or optical LED observation
is claimed. Main trace's 101 rows match the saved oracle; this is partial evidence.

Raw ELF/maps/flash dumps remain outside git; their identities are pinned. Text UART,
OpenOCD and verifier logs are retained here. No automatic retry or publication.

The full hardware-stack disassembly remains in the external artifact directory,
with its digest in external-artifact-sha256.json. It was externalized before the
local commit only after byte-for-byte verification; the original remains preserved.
