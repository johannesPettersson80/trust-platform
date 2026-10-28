# Supported Chips

A chip runs truST as a firmware, built once for each chip from the same runtime code as
`trust-runtime` on Linux, macOS and Windows. Any board with a supported chip runs that chip's
firmware. The board's pins are named in `io.toml`, and the chip's own settings are kept in
[`device.toml`](device-toml.md) on the chip.

| Chip | Status | Build target | First board |
| --- | --- | --- | --- |
| ESP32-C6 | runs on hardware: install, programs, Wi-Fi and firmware updates | `esp32c6` | ESP32-C6-DevKitC-1 |
| STM32U5A9 | its firmware builds; it has not run on hardware yet | `stm32u5a9` | Riverdi RVT50HQSNWC00-B |

## ESP32-C6

### The Chip

| Item | Value |
| --- | --- |
| Processor | the RISC-V HP core, RV32IMAC, at 160 MHz; the LP core is not used |
| Floating point | none in hardware: `REAL` and `LREAL` are computed in software |
| Crystal | 40 MHz |
| RAM | HP SRAM 512 KiB at `0x40800000`, LP SRAM 16 KiB at `0x50000000` |
| Flash | on the SPI flash interface, in the module; truST's layout fills 8 MB |
| USB | the USB-Serial/JTAG controller, `303A:1001`: the first install, the control connection and resets, on one cable |
| Network | Wi-Fi; the chip has no Ethernet |

The chip file also lists the chip's UARTs, CAN (TWAI), SPI, I2C, ADC and PWM, and Bluetooth LE. No
`io.toml` driver uses them yet.

### Memory

| Memory | Size and rule |
| --- | --- |
| Heap | 180,160 bytes in three regions: the main heap of 98,368 bytes, the 64 KiB the boot loader leaves, and 16,256 bytes of LP SRAM |
| Program region | 112 KiB of RAM for the installed program's code, which is copied there when the program starts. A program whose code does not fit runs from flash, slower, and is not refused. |
| Memory for programs | measured at every boot, once the firmware's own blocks are taken: the heap's free bytes, less 8,192 bytes of working memory (which also holds the program's Live Values) and 16,300 bytes for the checkpoint buffer |
| Wi-Fi | a chip whose `device.toml` names a Wi-Fi network takes 56 KiB for the radio driver and 38,588 bytes for the network's sockets, link and tasks, 95,932 bytes in all, at boot and before it measures the memory for programs. A chip without Wi-Fi settings starts no radio and keeps the whole heap. |
| Event store | 200 events, the default and the most a project may set in `[runtime.events] capacity` |
| Retained values | `[runtime.retain] mode = "file"` saves them into two checkpoint slots of 16 KiB in flash, at most 16,300 bytes of payload; `path` is not used on a chip |
| Program slots | two of 2,060 KiB; a program and its record must fit one |

`trust-runtime build` prints a program's memory figure. The chip reports its memory for programs
as `program_memory_bytes` in its `capabilities` reply, and it refuses a program whose memory figure
is larger, at the install and at the load, with both numbers. Because Wi-Fi takes memory at boot,
the same program can fit a chip without Wi-Fi settings and be refused by one with them.

### Flash Layout

| Region | Offset | Size |
| --- | --- | --- |
| Boot loader (ESP-IDF v5.5.1 with application rollback) | `0x0` | 32 KiB |
| Partition table | `0x8000` | 4 KiB |
| Boot counter | `0x9000`, `0xA000` | 4 KiB each |
| Firmware selection (`otadata`) | `0xB000` | 8 KiB |
| Device settings (`device.toml`) | `0xD000`, `0xE000` | 4 KiB each |
| Firmware slot a (`ota_0`) | `0x10000` | 1,984 KiB |
| Firmware slot b (`ota_1`) | `0x200000` | 1,984 KiB |
| Program store, slots a and b | `0x3F0000`, `0x5F3000` | 2,060 KiB each |
| Pairings | `0x7F6000`, `0x7F7000` | 4 KiB each |
| Checkpoint slots | `0x7F8000`, `0x7FC000` | 16 KiB each |

[Install And Update truST On A Chip](chip-install-and-update.md) explains the firmware slots.

### Pins

- A line names a pin as `GPIO` and a decimal number without leading zeros: `GPIO18`.
- The chip has `GPIO0` to `GPIO30`, as its largest package has them. A board names only the pins
  it has.
- The firmware keeps `GPIO12` and `GPIO13` for USB, and `GPIO24` to `GPIO30` for the SPI flash
  the firmware and the program run from.
- A line that names a kept pin, a pin the chip lacks, or a pin another line holds is refused:
  `GPIO12 is reserved by the firmware`, `GPIO31 is not a pin of this chip`,
  `GPIO18 is already claimed by another line`.
- An output takes its `initial` level before it becomes an output.

On the ESP32-C6-DevKitC-1:

- `GPIO9` is the BOOT button, low while it is pressed. It is a strapping pin, read by the chip at
  reset, so a program reads it after the boot.
- `GPIO8` drives the board's RGB LED, a WS2812, not a plain LED.

```toml
# io.toml on an ESP32-C6
[io]
driver = "gpio"

[io.params]
inputs = [{ address = "%IX0.0", line = "GPIO9", invert = true }]
outputs = [{ address = "%QX0.0", line = "GPIO18", initial = false }]
```

### What The Firmware Holds

Its chip file lists three runtime components:

| Component | On the ESP32-C6 |
| --- | --- |
| `engine` | compiled programs, tasks and cycles, the process image |
| `control` | the control connection over USB and over Wi-Fi: status, start, stop and restart, resource states and faults, events and log |
| `live-values` | read, write, force and release, at the pins too |

The firmware also carries out:

- installing a program over the control connection into two program slots (`ctl install`), so
  that a failed transfer or a loss of power leaves the previous program selected;
- retained values in flash;
- its device settings, [`device.toml`](device-toml.md);
- firmware updates with a trial boot and a rollback;
- `config.get` and `config.set`;
- the control connection over Wi-Fi with a control token, answers to ping, and discovery over mDNS;
- the time from an SNTP server, and the time kept across every reset but a loss of power;
- the fault and watchdog policies and the safe outputs of `runtime.toml` and `io.toml`;
- online changes (`bytecode.reload`), which no `trust-runtime ctl` command sends yet.

Not available on the ESP32-C6 yet:

- `io.toml` drivers other than `gpio`. A program that uses one does not start, and the chip reports
  `the installed application uses the <driver> driver, which the C6 does not carry out yet`.
- The web pages and the HMI.
- OPC UA, Beckhoff ADS, the mesh and the runtime cloud, the historian and the Prometheus metrics,
  OpenOT telemetry, and `simulation.toml`.
- Bluetooth: `[bluetooth] enabled = true` is accepted and does nothing yet.
- Several runtimes on one chip.
- VS Code: the extension and the debugger do not open a chip's USB link yet. These pages use
  `trust-runtime` on the command line.

## STM32U5A9

The STM32U5A9's firmware exists and builds from its chip file. It has not run on hardware yet: no
value of its chip file has been read on a U5A9. Its first board is the Riverdi RVT50HQSNWC00-B, a
5-inch STM32 Embedded Display; truST does not use the display.

| Item | Value |
| --- | --- |
| Processor | Cortex-M33 (ARMv8-M Mainline) at 160 MHz |
| Floating point | `REAL` in hardware (single precision); `LREAL` in software |
| Flash | 4 MiB in two banks of 8 KiB pages |
| RAM | 2,512 KiB: SRAM1 768 KiB, SRAM2 64 KiB, SRAM3 832 KiB, SRAM5 832 KiB and SRAM4 16 KiB; and 2 KiB of backup SRAM |
| USB | OTG_HS with the on-chip PHY; the ROM boot loader's DFU device is `0483:DF11` |
| Network | none: no Ethernet, no Wi-Fi, no Bluetooth |
| Event store | 1,000 events |
| Components | `engine` |

Pins are named as `P`, a port letter and a number: `PD12`. Ports A to I have pins 0 to 15, and
port J has `PJ0` to `PJ11`. The firmware keeps `PA11` and `PA12` (USB), `PA13` and `PA14` (debug
probe) and `PH0` (oscillator).

Not available on the STM32U5A9 yet:

- installing truST from `trust-runtime`: `firmware install --chip` takes `esp32c6` only. The first
  install through the chip's ROM DFU boot loader and updates with its two flash banks wait for
  truST's first run on the chip;
- installing a program over the control connection: its firmware has no program store yet;
- device settings (`device.toml`);
- retained values.

## Building For A Chip

`trust-runtime build --target <chip>` compiles the project into a program image for the chip's CPU:

| Chip | `--target` | Rust target | Floating point |
| --- | --- | --- | --- |
| ESP32-C6 | `esp32c6` | `riscv32imac-unknown-none-elf` | software `REAL` and `LREAL` |
| STM32U5A9 | `stm32u5a9` | `thumbv8m.main-none-eabihf` | hardware `REAL`, software `LREAL` |

A program computes bit-identical results on every device for the same inputs and logical times.

## No Read-out Protection

truST enables no read-out protection, flash encryption or secure boot on any chip. A debug probe
can read the firmware and the installed program. Signed programs
([`[signing] trusted_keys`](device-toml.md)) prove who built a program and that it was not changed;
they hide nothing.

## Related

- [ESP32-C6 Getting Started](esp32c6-getting-started.md)
- [Install And Update truST On A Chip](chip-install-and-update.md)
- [`device.toml`](device-toml.md)
- [Specification 34, Runtime Portability](../reference/specifications/34-runtime-portability.md): TARGET-01 to TARGET-10, Appendix D.2 and D.5, Appendix H
- [Specification 44, Native Execution](../reference/specifications/44-native-execution.md): §4.4, §10
