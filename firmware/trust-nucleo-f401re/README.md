# NUCLEO-F401RE Scope B composition

This is a standalone Rust 1.95.0 / edition 2021 bare-metal firmware workspace.
It uses the shared source-free engine, not a second PLC dispatcher. Its bounded
bring-up heap measures allocations; it does not claim allocation-free RUN or
production controller qualification.

Build on the authorized builder from the repository root with
`cargo xtask portability build-firmware <build-config.json>`. This canonical command
reads the checked-in firmware target flags, retains safe ICF and overflow checks,
and normalizes checkout/registry/rust-source prefixes for every compiled crate.
It records the full flags before invoking the locked release build. Direct Cargo
invocations without those remaps are not the qualified composition. The complete Scope B batch owns lockfile resolution, review,
release linking, map/section inspection, independent application packing,
SWD installation and UART verification. No successful fit is assumed.

## Flash and RAM

| Region | Address / size | Purpose |
|---|---|---|
| Flash sector 0 | `0x08000000`, 16 KiB | Reset/exception vectors plus selected immutable firmware data |
| Flash sector 1 | `0x08004000`, 16 KiB | Separately installed application bundle |
| Flash sectors 2 and 3 | `0x08008000` and `0x0800C000`, 16 KiB each | Reserved independent checkpoint slots; no persistence implementation |
| Flash sectors 4–7 | `0x08010000`, 448 KiB | Firmware code, constants and data load image |
| SRAM low region | 80 KiB | 72 KiB allocator plus at most 8 KiB static data |
| SRAM high region | `0x20014000`, 16 KiB | MSP and interrupts |

The linker selects the pinned float-parse table and small immutable tables for
sector 0; merged diagnostic strings stay in upper flash. It bounds that data
after vectors within sector 0 and retains
separate application/checkpoint sectors. Firmware loads must never be flattened
into an image that writes across those reserved gaps. The ELF inspector requires
at least 16 KiB free in the upper region; this is a bring-up margin, not a promise
that future installation/retain features fit. Map reports distinguish relocated
bytes from total size reduction. Application-only installation must leave sector 0
and upper firmware untouched. Explicitly erase only firmware sectors 0 and 4–7,
then write the ELF without automatic erase. Erase application sector 1 separately.
OpenOCD image-wide automatic erase can erase gaps between discontiguous sections;
verify the checkpoint sectors and firmware regions remain byte-identical around
the separate application write.

The linker prevents static/heap overlap with the reserved MSP. `cortex-m-rt`
paints unused RAM before Rust entry. Measurements scan only the high 16 KiB,
including live IRQ usage. They are high-water observations, not a worst-case
recursion or interrupt proof. Call depth is limited to four.

The GPIO fixture additionally executes one through four nested POU levels.
Before each, the runner saves the cumulative stack peak and repaints only memory
below the current MSP minus a 256-byte guard while interrupts are masked. `STACK`
records report each observed depth; the original preparation peak is retained in
`STACK_PEAK` and subsequent `MEM` records. The native execution result must equal
the requested nesting depth before a record is emitted.

The 32-byte application header is magic `TRSTB002`, three little-endian u32
lengths (main, numeric, GPIO), then their three CRC32 values. Payloads follow
without padding in that order. The two A4 payloads remain byte-identical to
the saved artifacts. No payload is linked into firmware. Firmware does not
erase/program flash; the stopped SWD test installer owns installation. This is
not power-loss-safe serial installation or durable RETAIN.

## Trace and supervision

USART2 runs at 230400 baud, 8N1. `B1` records use decimal integers except identity
registers and floating-point bits (hex). Main `TRACE` samples include the actual
hardware-derived logical millisecond, state, deadline, overrun count, measured
cycle microseconds/DWT ticks, and observed SysTick interrupts. The 101 scheduled
samples run from 0 through 1000 ms in real time; late samples remain observable
and do not get replaced with the requested schedule time.

The numeric fixture similarly runs 101 hardware samples, reports final bits and
maximum scan cost. GPIO records distinguish the actual unchanged PC13 input
from deliberately injected image values. PA5 IDR readback is electrical pad
evidence, not optical LED inspection or a manual button-toggle claim.

`MEM` fields are phase, allocator-used estimate, peak estimate, allocation calls,
failures, MSP used, MSP remaining, target Value size and alignment. Phase counters
reset before preparation, instantiation and RUN; stack high-water is boot-wide.
`PREP` cumulative logical charges are separately reported and are **not** peak
heap measurements.

Each prepared-module control object has one fixed `Box` owner in the existing
72 KiB heap, allocated before PREP/MEM reporting. Preparation completes in a
separate frame before instantiation; the borrowed runtime state drops before its
module owner. This avoids holding the large immutable control object on MSP for
an entire fixture. The normal terminal OOM/safe-output policy below still applies.

After STOP and a real engine deadline fault, outputs are forced off. The final
phase writes a backup-register cookie and withholds IWDG feed. Only a later boot
with both the expected cookie and independent-watchdog RCC flag emits `DONE`.
Panic/OOM/hard fault and an unexpected reset stop the runner with safe outputs;
they do not create an automatic retry loop. Normal RUN feeds IWDG only after a
successful complete scan. The final stopped supervisor can feed it to retain
diagnostics. No UTC clock or persistent RETAIN store is claimed.

Dependency API sources: [cortex-m-rt 0.7.5](https://docs.rs/cortex-m-rt/0.7.5/cortex_m_rt/)
and [embedded-alloc 0.7.0](https://docs.rs/embedded-alloc/0.7.0/embedded_alloc/struct.LlffHeap.html).
