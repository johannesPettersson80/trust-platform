# Install And Update truST On A Chip

The truST firmware is the runtime for the chip. The PLC program is separate: it is built with
`trust-runtime build --target` and installed with `trust-runtime ctl install`. Installing or
updating the firmware keeps the chip's installed program, its retained values and its
[device settings](device-toml.md).

| Task | Command | Link |
| --- | --- | --- |
| First install on a chip without truST | `trust-runtime firmware install --chip esp32c6 [--port <port>] <folder>` | USB, through the chip's own boot loader |
| Update a chip that runs truST | `trust-runtime ctl --endpoint <endpoint> firmware <image> [--wait <seconds>]` | the control connection: USB or the network |

The ESP32-C6 is the only chip that these commands install and update today
([Supported Chips](supported-chips.md)).

## The Firmware Folder

The firmware of one truST version for the ESP32-C6 is a folder of three files. The first install
writes each where the chip's flash layout places it:

| File | Written at | First bytes |
| --- | --- | --- |
| `bootloader.bin` | `0x0` | `0xE9`, an ESP image |
| `partitions.bin` | `0x8000` | `0xAA 0x50`, a partition table |
| `firmware.bin` | `0x10000`, the first firmware slot | `0xE9`, an ESP image |

Use the folder of the same truST version as your `trust-runtime`: the chip refuses a program built
by another release's toolchain and asks for the project to be rebuilt (see Versions below).

## First Install

```bash
trust-runtime firmware install --chip esp32c6 ./trust-firmware-esp32c6
```

| Option | Meaning |
| --- | --- |
| `--chip esp32c6` | the chip; `esp32c6` is the only value the command takes |
| `--port <port>` | the chip's USB serial port, such as `/dev/ttyACM0`, a stable name under `/dev/serial/by-id/`, or `COM5`; without it, the command uses the one USB serial port with the chip's boot loader USB ID, `303A:1001` |
| `<folder>` | the firmware folder |

The command writes through the chip's ROM download mode over its USB-Serial/JTAG port, with the
espflash library that `trust-runtime` carries. No other tool is needed. In order, it:

1. reads the three files and refuses a file that does not begin with its magic, before anything
   is written: `<path> is not a <file> file of the chip`;
2. finds the port; with no such port it stops with
   `no chip on USB with the boot loader 303a:1001; give --port`, and with two it names both;
3. erases the boot loader's firmware selection (`otadata`, `0xB000`, 8 KiB), so that the chip
   boots the first firmware slot;
4. writes the boot loader, the partition table and the firmware;
5. resets the chip, which starts truST.

```text
installing truST on esp32c6 at /dev/ttyACM0
...
installed truST on esp32c6 at /dev/ttyACM0; it starts the firmware now
```

The program store, the checkpoint slots and the device settings are not written, so a chip keeps
its program, its retained values and its settings across a new first install.

## Two Firmware Slots

The ESP32-C6 keeps two firmware slots and a boot loader that rolls back, built from ESP-IDF v5.5.1's
boot loader with application rollback:

| Region | Offset | Size |
| --- | --- | --- |
| Firmware slot a (`ota_0`) | `0x10000` | 1,984 KiB |
| Firmware slot b (`ota_1`) | `0x200000` | 1,984 KiB |
| Firmware selection (`otadata`) | `0xB000` | 8 KiB |

An update is written into the slot the chip does not run from. The running firmware stays
selectable until the new one has finished a boot.

## Update Over The Control Connection

Stop the program, then update:

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 shutdown
trust-runtime ctl --endpoint serial:///dev/ttyACM0 firmware ./trust-firmware-esp32c6/firmware.bin
```

The image is `firmware.bin` of the firmware folder. A chip with a control token needs `--token` or
`TRUST_CTL_TOKEN`, and a chip on the network takes the same command with its `tcp://` endpoint.

What happens:

1. `ctl` asks the chip for its `capabilities` and `status`. Before anything is uploaded it refuses
   a runtime without firmware slots
   (`this runtime keeps no firmware slots: truST on a computer is updated by installing truST on that computer`)
   and a program that is not stopped
   (`the runtime's program is <state>: stop it first (trust-runtime ctl shutdown), then update its firmware`).
2. `ctl` uploads the image. The chip writes it into the slot it does not run from as the parts
   arrive, and keeps the image's first 60 bytes in memory, so the slot holds no bootable image yet.
3. The chip checks the image where it lies: the ESP image magic, the chip ID of the ESP32-C6 (13),
   every segment inside the upload, the checksum, the appended SHA-256, no byte after the image, and
   truST's application descriptor, `trust-esp32c6`, whose version it reports.
4. The chip writes the kept bytes, reads them back, and selects the new slot for the next boot, on
   trial. `ctl` prints:

    ```text
    installed version=<new> slot=<slot>; the runtime restarts into it (it ran <old> from slot <slot>)
    ```

5. 100 ms after its reply the chip leaves its Wi-Fi network, if it has one, and resets into the new
   firmware.
6. `ctl` connects again every second, for at most `--wait` seconds (60 by default), until the chip
   answers. When the chip runs the new version from the new slot, confirmed, `ctl` prints:

    ```text
    running version=<new> slot=<slot> confirmed
    ```

The program, its retained values and the device settings stay as they are. The new firmware starts
the installed program as at any reset.

## Trial Boot And Rollback

- The boot loader boots the new firmware once on trial.
- The new firmware confirms itself when its boot is done: its control loop built, and the installed
  program loaded and started, or refused with its reason. From then on the boot loader keeps
  booting it.
- A firmware that resets before it has confirmed itself, a panic for example, is not booted again:
  the boot loader marks it aborted and boots the firmware before it.
- The chip that rolled back reports the version it rolled back, and `ctl firmware` fails with both
  versions:

```text
the runtime rolled the update back: truST <new> did not finish its boot, and the runtime runs truST <old> from slot <slot> again
```

Other failures of the wait:

| `ctl` reports | Meaning |
| --- | --- |
| `the runtime did not answer within <n> seconds of its restart into the new firmware` | no answer within `--wait` seconds |
| `the runtime runs truST <v> from slot <s> but has not confirmed it` | the new firmware answers but has not finished its boot |
| `the runtime runs truST <v> from slot <s>, not the installed <v> from slot <s>` | the chip runs another firmware than the one installed |

## What The Chip Checks

A refused image is not selected: the chip keeps running its firmware, and the slot the upload
went into holds no bootable image. The chip's refusals of an image include:

| Refusal | Cause |
| --- | --- |
| `the firmware image does not begin with the ESP image magic 0xE9` | not an ESP image |
| `the firmware image is for chip id <n>, and this chip is an ESP32-C6 (chip id 13)` | an image for another chip |
| `the firmware image's checksum is wrong` | a damaged image |
| `the firmware image's SHA-256 is wrong` | a damaged image |
| `the firmware image is the application '<name>', not truST's 'trust-esp32c6'` | not truST's firmware |

The update needs the program stopped because the chip erases no flash while a program runs: an
upload begun while the program runs is refused with `resource_not_running`.

## What The Chip Reports

The chip reports its firmware in the `firmware` member of its `capabilities` reply, which
`ctl firmware` reads and `trust-runtime ctl capabilities` prints:

| Field | Value |
| --- | --- |
| `slot` | the slot it runs from: `a` or `b` |
| `version` | the truST version of the running firmware |
| `confirmed` | `false` while the firmware runs on trial after an update, `true` otherwise |
| `rolled_back` | the version of a newer firmware that the boot loader stopped booting, or `null` |

A computer reports `firmware` as `null`: truST on a computer is updated by installing truST on it.

## Versions

A runtime reports its truST version in `status`. The tools and the runtimes of one truST release
belong together. When they differ, `trust-runtime ctl` writes which side to update to its error
output and then carries out the request:

```text
warning: the runtime runs truST 0.30.9 and these tools are truST 0.31.0: update the runtime to truST 0.31.0
```

The older side is the one to update: the chip with `ctl firmware`, or the tools. Versions compare as
release versions, `major.minor.patch`, with a pre-release such as `-rc.1` before its release.

A program image carries the kernel interface version and the program library of the release that
built it. A chip refuses an image of another release and asks for a rebuild:
`kernel interface <n> required, this runtime has <m>: rebuild the project`, or a refusal that names
both program libraries and asks to rebuild with this release's toolchain.

## STM32U5A9

`trust-runtime` does not install or update truST on the STM32U5A9 yet. Its first install through
the ROM's DFU boot loader and its updates with the chip's two flash banks wait for truST's first
run on the chip.

## Related

- [ESP32-C6 Getting Started](esp32c6-getting-started.md)
- [Supported Chips](supported-chips.md)
- [`device.toml`](device-toml.md)
- [`trust-runtime` CLI](../reference/cli/trust-runtime.md)
- [Specification 34, Runtime Portability](../reference/specifications/34-runtime-portability.md): TARGET-04, TARGET-05, DEPLOY-01, Appendix D.2
- [Specification 11, Runtime Engine](../reference/specifications/11-runtime-engine.md): the firmware update contract
