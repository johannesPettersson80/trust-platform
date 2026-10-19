# ESP32-C6-DevKitC-1 truST runtime

This is the ESP32-C6 truST runtime: a standalone Rust 1.99.0 / edition 2024 bare-metal firmware
workspace for the ESP32-C6-DevKitC-1 (`riscv32imac-unknown-none-elf`, 8 MiB flash, 160 MHz). A chip
is a truST runtime and works exactly like `trust-runtime` on a computer (specification 34). It boots
the shared single-thread resource loop, reads and admits the installed application, claims the
record's GPIO lines, prepares and starts the program, and serves the control link over the native
USB-Serial/JTAG port. A `restart` of the stopped program loads a newly installed program in place,
without a reset, so the USB connection stays up.

The shared loop, the container admission and the engine cycle live in `crates/trust-runtime-services`
and `crates/trust-runtime-core`; the two-region measured heap and the reset record live in
`crates/trust-platform-memory`; the C6 hardware adapter lives in `crates/trust-platform-esp32`: the
peripherals, clock, USB link, wait, GPIO claim and the chip's watchdog rule, and the three parts that
make a program replaceable, the program loader (`loader.rs`), the memory a program lives in
(`program_memory.rs`) and the boot counter (`boot_counter.rs`). This firmware keeps only its board
composition (design §1.1):

- `src/main.rs`: the boot sequence, the panic path and the runner composition.
- `src/application.rs`: the boot selection over the two program slots. It reads the header of each
  slot, takes the committed `TRSTA002` container with the highest install sequence, copies it once
  into the heap and admits it with `installation::admit_sequenced` against the C6's
  `DeviceDescriptor`. A refusal gives the reason and the install sequence of the newest committed
  container.
- `src/memory.rs`: the 72 KiB main heap, the boot loader's 64 KiB reclaimed region and the 24 KiB
  main-stack window.

## Boot sequence

The boot order is design §1.1:

1. **Hardware first.** `memory::initialize` gives the allocator its two regions and paints the stack
   window; the adapter takes the peripherals and brings the clock up, driving no output. The reset
   reason and the reset record kept in RTC fast memory are read once, and this boot is counted in the
   flash (the boot counter, below). The board is then split: the loop gets the clock, the wait and
   the watchdog, and the loader keeps the GPIO bank and the flash owner for the life of the boot, so
   that every program it loads claims the pins and the checkpoint flash that the program before it
   released.
2. **The installed application.** The firmware reads the header of each of the two program slots in
   the `stbc` partition and selects the committed `TRSTA002` container with the highest install
   sequence. It copies only that container's declared total length into the heap (refusing a length
   above the heap's room with its numbers) and admits it with `installation::admit_sequenced`
   against the C6's descriptor before any state exists. A slot that holds a `TRSTA001` container is
   not committed, and a damaged newest container refuses the boot: the other slot never starts in
   its place (specification 34 §5.1).
3. **The program.** The C6's loader (`Esp32Loader`) builds the program, at boot and at every later
   load in place, in this order. It prepares the module (`PreparedModule::from_bytes`, with limits
   derived from the application region and the heap) and frees the container's bytes at once; binds
   the program's Live Values; and sets the hardware watchdog's interval from the record's cycle
   watchdog. With `[runtime.retain] mode = "file"` it takes the chip's checkpoint slots as the
   program's retain store. It claims the record's GPIO lines by the chip's pin names (`GPIO18`,
   refused if the chip lacks the pin or reserves it), each output driven to its initial value before
   it becomes an output (STATE-01). It starts the program on the engine with the device's execution
   services and, for a warm start with a retain store, restores the newest committed checkpoint
   (specification 34 §10). The lines are claimed last, so a program that cannot be prepared leaves
   the outputs as the stop of the program before it left them. The loader feeds the hardware
   watchdog between these steps; the firmware starts the watchdog after the first program is built.
4. **Run.** `Runner::new(link, wait, cycle, platform, clock, event_capacity)`, or `Runner::stopped`
   when there is no program, is given the transfer session of the boot, the program store and the
   loader, and `run()` serves the control link and runs the scans. The event store keeps the
   record's `[runtime.events] capacity`, or the C6's default of 200 from `chips/esp32c6.toml` when
   the project sets none.

With no valid installed application, or one the device refuses, the firmware still runs the same
loop in the Stopped state and serves the link, so `status` says why and a program can be installed
over the link. `restart` is refused with `no installed program: <reason>` until the store holds a
container newer than the one the boot refused; it then loads that container in place.

## Loading a newer program in place

A `restart` of the Stopped resource compares the install sequence of the newest committed container
in the program store with the sequence of the container the program came from (or that the boot
refused). When the store's is newer, the loop loads it without a reset, and the control link stays
up (specification 34 STATE-08):

1. It drops the program that ended: the engine cycle and its state, the I/O lines (the pins go back
   to the GPIO bank), the handle on the checkpoint flash, and the prepared module and the execution
   services, which the cycle owns (`ProgramMemory`) and frees when it is dropped. Nothing of the new
   container has been read yet, so the heap holds one program at a time.
2. It reads the newest committed container within the room the heap has, checks its payloads and
   admits it, as the boot does.
3. The loader builds and starts it as the boot does (step 3 above) in the mode the request gave:
   `cold` gives every variable its declared initial value, and `warm` restores the retained values
   of the newest checkpoint.

The program's records start empty, as for a new process. The control link, the transfer session and
its ids, the program store, the audit sequence and the reset record stay, and the load writes no
flash. A `restart` with no newer container restarts the running program in place. A program that
cannot start (a damaged container, a refused record, a pin the chip lacks, a program that cannot be
prepared) leaves the resource Faulted with `no installed program: <reason>`, the old program being
gone, and the next `restart` tries again. The loop serves no request while it loads and feeds the
hardware watchdog between the steps. A client reads the first reply after such a `restart` with the
bound that specification 11 §6.8.1 gives for installing the same container.

## Control link

The native USB-Serial/JTAG port is the control link (PORT-05, design §2). A small interrupt handler
moves bytes between the peripheral's 64-byte FIFOs and the shared line link; the runner polls it for
requests and sends one reply per request. The link carries replies only: the firmware never prints on
it. The boot loader still prints its own lines before the first reply.

The chip answers the same operations as `trust-runtime` (`status`, `capabilities`, `health`,
`tasks.stats`, `events.tail`, `faults`, `pause`, `resume`, `restart`, `shutdown`, the transfer
operations `transfer.begin`, `transfer.part`, `transfer.status`, `transfer.get` and `transfer.abort`,
`program.install`, `program.active` and `bytecode.reload`); the endpoint is the board's
`/dev/serial/by-id` path (specification 11 §6.9). `program.active` reports the installed program and
the one before it, reading the header and the record of each container from the two program slots in
reads of at most 256 bytes, and never the program. `bytecode.reload` changes the running program
online, as it does on `trust-runtime`: it is applied at the program's next scan boundary, without a
reset and without writing flash, the program keeps its pins and their output levels, and its retained
values, function-block instances and references carry over when the new program declares the same
retained storage (specification 34 §11.3). The reply is the one reply in flight until the change is
applied or refused. Requests and replies are bounded by the link's frame; a request or a reply above
it is refused with the clear error, never cut.

## Build

Build on the authorized builder from the repository root with
`cargo xtask portability build-firmware esp32c6 <build-report.json>`. The command reads this
firmware's crate, binary name and target from the chip's file, `chips/esp32c6.toml`, and checks the
required link flags. A direct Cargo invocation is not the qualified composition. The chip's file
also owns lockfile resolution, review, the release link, map and section inspection
(`cargo xtask portability inspect esp32c6-devkitc-1 <firmware.elf> <report.json>`), application
packing, installation and the USB bench checks.

## Install

The firmware and the program are installed separately (DEPLOY-01). Install only while the board is
stopped: the firmware never writes or erases flash during RUN, and `program.install` writes only
while the program is Stopped (specification 34 §11.2). Name the board's USB port explicitly on every
command. Its `/dev/serial/by-id` path contains the MAC address.

The firmware, with the boot loader and the partition table:

```
espflash flash --port <devkit-serial-by-id> --partition-table partitions.csv \
  target/riscv32imac-unknown-none-elf/release/trust-esp32c6
```

The `stbc` partition is 6112 KiB, the flash the chip allows (owner decision O7), and holds the two
program slots. The partition table also declares the boot counter's two 4 KiB sectors as the data
partitions `bootcnt_a` and `bootcnt_b` (type `0x42`), so that no tool writes there. The second-stage
bootloader is the one bundled with espflash 4.6.0 (ESP-IDF v6.1-beta1); M5 qualification requires a
released bootloader instead.

### A first program, from the bench

`cargo xtask portability pack-project <program.stbc> <deployment-record.bin> application.bin`
writes the `TRSTA002` container of a first flash: the program and its record, with install
sequence 1 (specification 34 §5.1). Written into slot A at `0x200000`, it is a committed container,
and the boot starts it after the next reset:

```
espflash write-bin --port <devkit-serial-by-id> 0x200000 application.bin
espflash reset --port <devkit-serial-by-id>
trust-runtime ctl --endpoint usb://<mac> status
```

A chip that is running without a program starts the container by `restart` when it is newer than
the one its boot refused, if it refused one; `restart` loads it in place (see "Loading a newer
program in place").

A flash that stops part-way can leave a good header over missing bytes. The boot then refuses the
container as damaged, and `restart` names the failed check; flash the file again.

### A program, over USB

With the board stopped, the same `application.bin` is installed over the control link, as
`program.install` installs a program into any runtime (specification 34 §11.2). `trust-runtime ctl`
has no install action yet, so a client sends these requests, one JSON line each. The USB link acts as
Admin, because a chip has no control token yet:

1. `{"id":1,"type":"shutdown"}` stops the program.
2. `{"id":2,"type":"transfer.begin","params":{"bytes":<file size>,"sha256":"<64 hex digits of the
   file>"}}` replies with the transfer id, `part_bytes` 4096 and `next_offset` 0.
3. For each part of 4,096 bytes, in order (the last part is shorter):
   `{"id":3,"type":"transfer.part","params":{"transfer":"<id>","offset":<offset>,"data":"<base64
   of the part>","crc32":"<8 hex digits of the part's CRC-32>"}}`. The reply gives the next offset.
4. `{"id":4,"type":"program.install","params":{"transfer":"<id>"}}` checks the container, its
   record and its admission, the retained values against the newest checkpoint, and the program's
   preparation, and writes nothing until all of them pass. Then it erases and writes the slot that
   does not hold the newest committed container, one step in each pass of the loop. The reply,
   `{"status":"installed","sequence":2,"program_sha256":"...","bytes":...}`, comes after the whole
   commit, and the link answers no other request meanwhile. Read it for up to 4 s plus 0.4 s for
   each 4 KiB of the file plus 10 ms for each 256 bytes (specification 11 §6.8.1): the 750 ms of
   `ctl` is too short.
5. `{"id":5,"type":"restart","params":{"mode":"warm"}}` (or `"cold"`) starts the program just
   installed, in place. The reply `{"status":"restart queued"}` comes at once, and the chip drops the
   running program and loads the committed container with the highest install sequence, which is the
   one just installed ("Loading a newer program in place"). It serves no request while it loads, so
   read the next reply for up to the bound of step 4. No reset is needed and the link stays up. A
   reset (`espflash reset`) starts the same program, because the boot selects the committed
   container with the highest install sequence.

The device writes its own install sequence, one more than the newest committed container's, so the
sequence in the file is replaced. A refused install changes nothing, and an install that stops at
any step, even at a power loss, leaves the previous program selected (DEPLOY-03).

## Flash layout

| Region | Address / size | Purpose |
|---|---|---|
| Bootloader | `0x0` | ESP-IDF second-stage bootloader from espflash 4.6.0 |
| Partition table | `0x8000` | `partitions.csv` |
| `bootcnt_a`, `bootcnt_b` | `0x9000` and `0xA000`, 4 KiB each | the boot counter (specification 34 PORT-05): two logs of 512 records, written once per boot. The chip file `chips/esp32c6.toml` names them `boot_counter_a` and `boot_counter_b`, and the reader of the file refuses a layout that overlaps them. |
| `factory` | `0x10000`, 1984 KiB | this firmware image |
| `stbc` | `0x200000`, 6112 KiB | the two program slots of the program store: slot A at `0x200000` and slot B at `0x4FC000`, 3056 KiB each, each holding one `TRSTA002` container. The boot reads the header of each slot and the newest committed container once. |
| `ckpt_a`, `ckpt_b` | `0x7F8000` and `0x7FC000`, 16 KiB each | retain checkpoint log: the newest committed checkpoint is restored at boot, and the retained values are saved at the project's interval and at stop |

## RAM

| Region | Size | Purpose |
|---|---|---|
| Main heap (`HEAP_MEMORY`) | 72 KiB static | the first measured-allocator region |
| Reclaimed region (`RECLAIMED`) | 64 KiB | the boot loader's RAM, `#[esp_hal::ram(reclaimed)]` in `.dram2_uninit`, the second heap region |
| Main-stack window | 24 KiB below the fixed stack top `0x4086E610` | measured, 2 KiB minimum headroom; the linker refuses a stack region under 28 KiB |
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
and budgets only the top 24 KiB, below `0x4086E610`, the `_stack_start` of esp-hal 1.2.2's C6 memory
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

Retained values survive a reset. With `[runtime.retain] mode = "file"` the boot restores the newest
committed checkpoint from the two checkpoint slots. A damaged newest checkpoint fails that warm start,
so the program does not start and the control link reports why, as a damaged retain file does on a
computer. Between scans, a save programs one 256-byte page per step and erases one 4 KiB sector per
step; an erase runs with interrupts masked for up to about 0.4 s. At a stop the pending save completes
at once, before the resource reports Stopped. No save step runs inside a scan.

Each boot is counted in the flash, in the sectors `bootcnt_a` and `bootcnt_b`. A sector is a log of
512 records of 8 bytes, the count and its complement. The boot writes the next count after the last
record and reads it back; when a sector is full it erases the other one and continues there. So the
count survives a loss of power, and one erase is spent for 512 boots. The count is the boot number
that every transfer id carries (specification 34 PORT-05), so an id never repeats across boots, and
it stamps the reset record. The write, and the erase once in 512 boots, happen at the start of the
boot, before the watchdog runs.

The reset reason and the record kept in RTC fast memory are read once at boot, before any
application state exists. The record is the same one trust-runtime reports as the last fault. The CPU
runs at `CpuClock::max()`, 160 MHz; SYSTIMER (16 MHz) gives the runner its monotonic microseconds.
`misa` must read RV32 with I, M, A and C, and without F or D, so REAL and LREAL use software floating
point.

## Limits

- Radios stay disabled: the firmware initializes no Wi-Fi or BLE.
- Retained values are saved into the two 16 KiB checkpoint slots. An image larger than the 16,300
  payload bytes a slot takes cannot be saved, and a failed save fails the next scan before its outputs.
  A power cut loses at most the save in progress: the guarantee is the last completed save, not the
  last scan. There is no UTC clock.
- The C6 carries out the `gpio` I/O driver only until slice T1; a record that asks for another driver
  is refused with the driver's name.
- A program that cannot start after a `restart` leaves the resource Faulted, and a Faulted chip takes
  no `program.install`. When the newest container is the one that cannot start, reset the board: the
  boot refuses that container, serves the link Stopped and takes an install.
- Loading a program in place is one step of the control loop: the link is not served meanwhile, and
  the longest such step is still to be measured on the bench. An upload left open keeps its buffer
  in the heap during a load, which can make the load refuse the container for room;
  `transfer.abort` frees it.
- `bytecode.reload` (online change) prepares the new program while the running one stays installed,
  so the heap holds both programs and the upload at once. The new program is prepared within the
  heap's free bytes less a reserve: 8 KiB of working memory for every change, and, when the
  running program keeps retained values, the length of the newest checkpoint's payload. A program
  that needs more, by any limit of its preparation, is refused with `insufficient memory` and its
  numbers, while the running program goes on unchanged. The working memory comes from a
  measurement on the host and the bench replaces it, and the longest uninterrupted step of a change
  is still to be measured against the hardware watchdog (specification 34 §11.3 and Appendix D.2).
- The program slots are 3056 KiB each, so a container larger than a slot (or larger than the heap's
  room) cannot be installed, while `trust-runtime` has no such limit. An install holds the upload and
  the commit's own copy of the container in the heap, and a container the heap cannot hold is
  refused (`no room for a transfer of <bytes> bytes`, `no room to install a container of <bytes>
  bytes`). The largest installable program is still to be measured.

Dependency API sources: [esp-hal 1.2.2](https://docs.rs/esp-hal/1.2.2/esp_hal/) and
[embedded-alloc 0.7.0](https://docs.rs/embedded-alloc/0.7.0/embedded_alloc/struct.LlffHeap.html).
