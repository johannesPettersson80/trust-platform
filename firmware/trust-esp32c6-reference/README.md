# ESP32-C6-DevKitC-1 truST runtime

This is the ESP32-C6 truST runtime: a standalone Rust 1.99.0 / edition 2024 bare-metal firmware
workspace for the ESP32-C6-DevKitC-1 (`riscv32imac-unknown-none-elf`, 8 MiB flash, 160 MHz). A chip
is a truST runtime and works exactly like `trust-runtime` on a computer (specification 34). It boots
the shared single-thread resource loop, reads and admits the installed application, claims the
record's GPIO lines, prepares and starts the program, and serves the control link over the native
USB-Serial/JTAG port.

The shared loop, the container admission and the engine cycle live in `crates/trust-runtime-services`
and `crates/trust-runtime-core`; the two-region measured heap and the reset record live in
`crates/trust-platform-memory`; the C6 hardware adapter (peripherals, clock, USB link, wait, GPIO
claim and the chip's watchdog rule) lives in `crates/trust-platform-esp32`. This firmware keeps only
its board composition (design §1.1):

- `src/main.rs`: the boot sequence, the panic path and the runner composition.
- `src/application.rs`: the header read, the total-length copy into the heap, and `installation::admit`
  against the C6's `DeviceDescriptor`.
- `src/memory.rs`: the 72 KiB main heap, the boot loader's 64 KiB reclaimed region and the 16 KiB
  main-stack window.

## Boot sequence

The boot order is design §1.1:

1. **Hardware first.** `memory::initialize` gives the allocator its two regions and paints the stack
   window; the adapter takes the peripherals and brings the clock up, driving no output. The reset
   reason and the reset record kept in RTC fast memory are read once, and this boot is counted for
   the next record.
2. **The installed application.** The firmware reads the `TRSTA001` container's header from the
   `stbc` partition, copies only its declared total length into the heap (refusing a length above
   the heap's room with its numbers), and admits it with `installation::admit` against the C6's
   descriptor before any state exists.
3. **Outputs.** The deployment record's GPIO lines are claimed by the chip's pin names
   (`GPIO18`, refused if the chip lacks the pin or reserves it), and each output is driven to its
   initial value before it becomes an output (STATE-01).
4. **Prepare and start.** `PreparedModule::from_bytes` is given limits derived from the application
   region and the heap, the program is instantiated on the engine with the device's execution
   services, and the resource loop starts its task timing. The prepared module lives for the rest of
   the boot (boxed and leaked). Retain mode is `none` until the C6 checkpoint driver exists: a record
   that requests another mode is refused.
5. **Run.** `Runner::new(link, wait, cycle, platform, clock, event_capacity).run()` serves the
   control link and runs the scans; the event store keeps the record's `[runtime.events] capacity`,
   or the C6's default of 200 from `chips/esp32c6.toml` when the project sets none.

With no valid installed application, or one the device refuses, the firmware still runs the same
loop in the Stopped state and serves the link, so `status` says why and a deployment can be accepted;
`restart` is refused until a program is installed.

## Control link

The native USB-Serial/JTAG port is the control link (PORT-05, design §2). A small interrupt handler
moves bytes between the peripheral's 64-byte FIFOs and the shared line link; the runner polls it for
requests and sends one reply per request. The link carries replies only: the firmware never prints on
it. The boot loader still prints its own lines before the first reply.

The chip answers the same operations as `trust-runtime` (`status`, `health`, `tasks.stats`,
`events.tail`, `faults`, `pause`, `resume`, `restart`, `shutdown`); the endpoint is the board's
`/dev/serial/by-id` path (specification 11 §6.9). Requests and replies are bounded by the link's
frame; a request or a reply above it is refused with the clear error, never cut.

## Build

Build on the authorized builder from the repository root with
`cargo xtask portability build-firmware esp32c6 <build-report.json>`. The command reads this
firmware's crate, binary name and target from the chip's file, `chips/esp32c6.toml`, and checks the
required link flags. A direct Cargo invocation is not the qualified composition. The chip's file
also owns lockfile resolution, review, the release link, map and section inspection
(`cargo xtask portability inspect esp32c6-devkitc-1 <firmware.elf> <report.json>`), application
packing, installation and the USB bench checks.

## Install

Install only while the board is stopped. Firmware never writes or erases flash during RUN. Name the
board's USB port explicitly on every command. Its `/dev/serial/by-id` path contains the MAC address.

```
espflash flash --port <devkit-serial-by-id> --partition-table partitions.csv \
  target/riscv32imac-unknown-none-elf/release/trust-esp32c6-reference
espflash write-bin --port <devkit-serial-by-id> 0x200000 application.bin
trust-runtime ctl --endpoint usb://<mac> status
```

`application.bin` is the `TRSTA001` container the development computer generates and packs
(`cargo xtask portability pack`), written into the `stbc` partition. The partition is 6112 KiB, the
flash the chip allows (owner decision O7). The second-stage bootloader is the one bundled with
espflash 4.6.0 (ESP-IDF v6.1-beta1); M5 qualification requires a released bootloader instead.

## Flash layout

| Region | Address / size | Purpose |
|---|---|---|
| Bootloader | `0x0` | ESP-IDF second-stage bootloader from espflash 4.6.0 |
| Partition table | `0x8000` | `partitions.csv` |
| `factory` | `0x10000`, 1984 KiB | this firmware image |
| `stbc` | `0x200000`, 6112 KiB | separately installed `TRSTA001` application container, read once per boot |
| `ckpt_a`, `ckpt_b` | `0x7F8000` and `0x7FC000`, 16 KiB each | reserved; no persistence implementation |

## RAM

| Region | Size | Purpose |
|---|---|---|
| Main heap (`HEAP_MEMORY`) | 72 KiB static | the first measured-allocator region |
| Reclaimed region (`RECLAIMED`) | 64 KiB | the boot loader's RAM, `#[esp_hal::ram(reclaimed)]` in `.dram2_uninit`, the second heap region |
| Main-stack window | 16 KiB below the fixed stack top `0x4086E610` | measured, 2 KiB minimum headroom |
| Scan-time code (`.rwtext`) | about 300 KB | the interpreter and the engine's scan path, executed from RAM |

Scan-time code runs from HP SRAM. From flash, through the 32 KB cache, one task activation took
about 1.77 M cycles for 188,838 instructions (11.07 ms, over the 10 ms period); from RAM it takes
about 264,000 cycles (1.65 ms). `rwtext_hook.x` lists the selected modules and functions; `build.rs`
puts it on the linker path and `.cargo/config.toml` enables esp-hal's hook. Only the scan's hot path
is moved: load-time code (preparation, decoding, validation, construction, the engine's initializers
and its restart and retain paths) and the control path (the runner, its requests, its services and
JSON, and the platform glue) stay in flash. The USB-Serial/JTAG interrupt handler and the FIFO
servicing it calls are in RAM through esp-hal's `#[ram]` attribute, because they run while the flash
cache may be off. `xtask portability inspect` fails if the VM dispatch loop is not in HP SRAM.

The esp-hal stack region spans the HP SRAM left after static data and RAM code. The firmware measures
and budgets only the top 16 KiB, below `0x4086E610`, the `_stack_start` of esp-hal 1.2.2's C6 memory
map; the window is a constant, as on the STM32F407, and `xtask portability inspect` fails if the
linked `_stack_start` differs. Exceeding that window fails the run, although the region has room.
Interrupts use the same stack on RISC-V, so the window includes them.

esp-riscv-rt does not paint RAM the way cortex-m-rt's `paint-stack` does. `memory::initialize`
therefore paints the window below the live stack once, before the first measurement. The figures are
high-water observations, not a worst-case recursion or interrupt proof.

## Reset and supervision

MWDT0 on TIMG0 resets the system after the configured cycle watchdog timeout. The panic handler is
the terminal fault path: it masks interrupts, drives every configured output to its safe value
through the adapter's emergency path, writes the cause and location into the reset record, and resets
the chip. It never writes to the control link. What a chip does after such a reset is owner question
O10.

The reset reason and the record kept in RTC fast memory are read once at boot, before any
application state exists. The record is the same one trust-runtime reports as the last fault. The CPU
runs at `CpuClock::max()`, 160 MHz; SYSTIMER (16 MHz) gives the runner its monotonic microseconds.
`misa` must read RV32 with I, M, A and C, and without F or D, so REAL and LREAL use software floating
point.

## Limits

- Radios stay disabled: the firmware initializes no Wi-Fi or BLE.
- The `ckpt_*` partitions are reserved only. There is no persistence, no UTC clock and no durable
  RETAIN, so the deployment record's retain mode must be `none`.
- The C6 carries out the `gpio` I/O driver only until slice T1; a record that asks for another driver
  is refused with the driver's name.
- The application region is 6112 KiB, so a container larger than that (or larger than the heap's
  room) cannot be installed, while `trust-runtime` has no such limit.

Dependency API sources: [esp-hal 1.2.2](https://docs.rs/esp-hal/1.2.2/esp_hal/) and
[embedded-alloc 0.7.0](https://docs.rs/embedded-alloc/0.7.0/embedded_alloc/struct.LlffHeap.html).
