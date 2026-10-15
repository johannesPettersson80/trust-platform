# ESP32-C6-DevKitC-1 M2E reference composition

This is a standalone Rust 1.99.0 / edition 2024 bare-metal firmware workspace for the
ESP32-C6-DevKitC-1 (`riscv32imac-unknown-none-elf`, 8 MB flash, 160 MHz). It is M2E bring-up
evidence: it runs the same fixture as the NUCLEO-F401RE composition on the C6 adapter. It is not
qualified product firmware, and it claims no allocation-free RUN, no durable RETAIN and no
production controller qualification.

The shared fixture (runner, trace records, bundle decode, heap and stack measurement) lives in
`crates/trust-board-fixture`, and the C6 hardware adapter lives in `crates/trust-platform-esp32`.
This firmware keeps only its board composition:

- `src/main.rs`: entry, identity record, reset cookie, hardware-profile check, watchdog arm.
- `src/board.rs`: wrappers for the adapter types, and the fixture's memory value for this board.
- `src/memory.rs`: the 72 KiB allocator arena (`HEAP_MEMORY`) and the 16 KiB main-stack window.
- `src/application.rs`: the one-time copy of the `stbc` partition into RAM.

## Build

Build on the authorized builder from the repository root with
`cargo xtask portability build-firmware esp32c6-devkitc-1 <build-report.json>`. The command uses
this firmware's target, binary name and required link flags. A direct Cargo invocation is not the
qualified composition. The M2E batch owns lockfile resolution, review, the release link, map and
section inspection (`cargo xtask portability inspect esp32c6-devkitc-1 <firmware.elf> <report.json>`),
application packing (`cargo xtask portability pack <gpio.stbc> <application.bin>`), installation and
UART verification. No successful fit is assumed.

## Install and verify

Install only while the board is stopped. Firmware never writes or erases flash during RUN. Name the
board's USB port explicitly on every command. Its `/dev/serial/by-id` path contains the MAC address.

```
espflash flash --port <devkit-serial-by-id> --partition-table partitions.csv \
  target/riscv32imac-unknown-none-elf/release/trust-esp32c6-reference
espflash write-bin --port <devkit-serial-by-id> 0x200000 application.bin
cargo xtask portability verify esp32c6-devkitc-1 <uart.txt> <report.json>
```

The bundle must fit the 16 KiB `stbc` partition. The second-stage bootloader is the one bundled
with espflash 4.6.0 (ESP-IDF v6.1-beta1). M5 qualification requires a released bootloader instead.

## Flash layout

| Region | Address / size | Purpose |
|---|---|---|
| Bootloader | `0x0` | ESP-IDF second-stage bootloader from espflash 4.6.0 |
| Partition table | `0x8000` | `partitions.csv` |
| `factory` | `0x10000`, 1984 KiB | this firmware image |
| `stbc` | `0x200000`, 16 KiB | separately installed application bundle, read once per boot |
| `ckpt_a`, `ckpt_b` | `0x204000` and `0x208000`, 16 KiB each | reserved; no persistence implementation |

## RAM

| Region | Size | Purpose |
|---|---|---|
| Allocator arena (`HEAP_MEMORY`) | 72 KiB static | the fixture's bounded bring-up allocator |
| Main-stack window | 16 KiB below the fixed stack top `0x4086E610` | measured by the fixture, 2 KiB minimum headroom |
| Scan-time code (`.rwtext`) | about 303 KB | the interpreter's scan path, executed from RAM |

Scan-time code runs from HP SRAM. From flash, through the 32 KB cache, one task activation took
about 1.77 M cycles for 188,838 instructions (11.07 ms, over the 10 ms period); from RAM it takes
about 264,000 cycles (1.65 ms). `rwtext_hook.x` lists the selected modules; `build.rs` puts it on
the linker path and `.cargo/config.toml` enables esp-hal's hook. Load-time code (validation,
decoding, preparation) stays in flash. `xtask portability inspect` fails if the VM dispatch loop is
not in HP SRAM.

The esp-hal stack region spans the HP SRAM left after static data and RAM code (about 51 KiB). The fixture
measures and budgets only the top 16 KiB, below `0x4086E610`, the `_stack_start` of esp-hal 1.2.2's
C6 memory map; the window is a constant, as on F401, and `xtask portability inspect` fails if the
linked `_stack_start` differs. Exceeding that window fails the run, although the region
has room. Interrupts use the same stack on RISC-V, so the window includes them.

esp-riscv-rt does not paint RAM the way cortex-m-rt's `paint-stack` does. `memory::initialize`
therefore paints the window below the live stack once, before the first measurement. The GPIO
fixture later repaints the same kind of region before each nested-POU depth. Stack figures are
high-water observations, not a worst-case recursion or interrupt proof.

## Trace

The native USB-Serial/JTAG port carries the `B1,` records through one bounded, non-blocking writer:
512 bytes and 100 ms per record. A record that misses its deadline fails the run. The ROM and the
second-stage bootloader print about 30 non-record lines at each boot. The verifier admits them only
before that boot's first IDENTITY record, at most 64 per boot.

`B1,IDENTITY` carries the chip ID (13), the chip revision as major·100 + minor (2), the base MAC
(`0000206e` for the high bytes and `f11704c0` for the low four, as printed by espflash), the flash
size in KiB (8192), the CPU clock in Hz, `misa` and the reset reason. The verifier pins this
board's MAC, as the F401 profile pins its UID.

## Reset and supervision

MWDT0 on TIMG0 resets the system at stage 0 after 4,000 ms. The run ends with one deliberately
withheld feed. The bench starts every run with a USB-Serial/JTAG reset (reason `0x15`). A power-on
reset (`0x01`) also starts a run. Either discards a leftover cookie. An MWDT0 reset (`0x07`) with the
watchdog cookie completes the run, which emits `WATCHDOG_RESET` and then `DONE`. Any other reset,
or an MWDT0 reset without the cookie, halts with `unexpected-reset-cookie`. There is no automatic
retry.

The reset cookie is two words in RTC fast memory, a magic prefix with a complementary integrity
word. It holds the `WATCHDOG` phase (`0x5744`) or the `RUNNING` phase (`0x5255`). The firmware reads
and clears it at every boot.

GPIO9 is the BOOT button: active low, with the internal pull-up, and a strapping pin that is read
only after boot. It is the physical input. GPIO18 is the output, read back through its own pad. That
is electrical evidence, not observed actuator response. The RGB LED on GPIO8 is a WS2812, so optical
output is unverified.

The CPU runs at `CpuClock::max()`, 160 MHz. SYSTIMER (16 MHz) gives monotonic microseconds, and the
machine performance counter (CSR `0x7E2`) gives CPU cycles. A 1 kHz SYSTIMER alarm only counts
interrupts; it does not run the PLC.

`misa` must read RV32 with I, M, A and C, and without F or D, so REAL and LREAL use software
floating point. The composition checks this at boot.

A panic or trap calls `emergency_halt`. That masks interrupts, drives GPIO18 low, writes one bounded
`B1,FAIL` record and keeps MWDT0 fed, so the board halts rather than resetting. esp-hal's default
trap handler panics, so an exception takes the panic path and reports a panic-form code.

## Limits

- This is M2E bring-up evidence, not qualified product firmware. No debugger is attached during
  M2E, so supervision during a debug halt is not evaluated; STATE-06 still applies before
  qualification.
- Radios stay disabled: the firmware initializes no Wi-Fi or BLE.
- The `ckpt_*` partitions are reserved only. There is no persistence, no UTC clock and no durable
  RETAIN.
- The heap is a bounded bring-up allocator. The fixture's `PREP` figures are cumulative logical
  charges, not heap peaks.

Dependency API sources: [esp-hal 1.2.2](https://docs.rs/esp-hal/1.2.2/esp_hal/) and
[embedded-alloc 0.7.0](https://docs.rs/embedded-alloc/0.7.0/embedded_alloc/struct.LlffHeap.html).
