# ESP32-C6 Getting Started

From an ESP32-C6 board without truST to a program that runs on it, over USB and then over Wi-Fi.
A chip is a truST runtime: it runs the same project files as `trust-runtime` on a computer and is
controlled with the same `trust-runtime ctl` command. Only the pin names in `io.toml` are the
chip's.

## What You Need

- A board with an ESP32-C6, such as the ESP32-C6-DevKitC-1. truST's flash layout fills 8 MB of
  flash.
- A USB cable from the computer to the board's native USB port, the chip's USB-Serial/JTAG
  controller on `GPIO12` and `GPIO13`. The same cable carries the first install, the control
  connection and the chip's resets.
- `trust-runtime` on the computer, on Linux, macOS or Windows: [Install truST](../start/installation.md).
- The truST firmware folder for the ESP32-C6, of the same truST version as `trust-runtime`:
  `bootloader.bin`, `partitions.bin` and `firmware.bin`.
- Room for the compiler toolchain, about 630 MB once installed. The VS Code extension carries it
  and the first build installs it; `trust-runtime` on its own downloads it, about 220 MB, once.
- For Wi-Fi, a WPA2-Personal network or an open one. The ESP32-C6's radio driver has no WPA3
  station.

No other tool is needed: `trust-runtime` writes the firmware through the chip's ROM download mode.

## 1. Install truST On The Chip

Connect the board and run:

```bash
trust-runtime firmware install --chip esp32c6 ./trust-firmware-esp32c6
```

The command finds the board by the USB ID of its USB-Serial/JTAG controller, `303A:1001`. With no
such port, or two, it stops and asks for the port:

```bash
trust-runtime firmware install --chip esp32c6 --port /dev/ttyACM0 ./trust-firmware-esp32c6
```

It writes the boot loader, the partition table and the firmware, and resets the chip:

```text
installing truST on esp32c6 at /dev/ttyACM0
...
installed truST on esp32c6 at /dev/ttyACM0; it starts the firmware now
```

[Install And Update truST On A Chip](chip-install-and-update.md) has the details and the updates
that follow.

## 2. Connect Over USB

`trust-runtime ctl` reaches the chip over the same USB port with a `serial://` endpoint:

| Computer | Endpoint, with an example port name |
| --- | --- |
| Linux | `serial:///dev/ttyACM0` |
| macOS | `serial:///dev/cu.usbmodem1101` |
| Windows | `serial://COM5` |

On Linux the port also has a stable name under `/dev/serial/by-id/`, which contains the chip's MAC
address. `trust-runtime discover --only usb,serial` lists the board's USB device and the serial
port behind it without opening the port.

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 status
```

A chip without a program answers `state=stopped`. Opening the port does not reset the chip. Until
the chip's device settings set a control token, a request over USB acts with the Admin role.

## 3. Build A Project For The Chip

A project for the chip is the same as for `trust-runtime` on a computer: `runtime.toml`, `io.toml`
and the program. Start from a project such as `examples/communication/gpio`, whose program copies
`%IX0.0` to `%QX0.0` ([GPIO](../connect/devices-and-fieldbus/gpio.md)).

On the chip, the `gpio` driver's `line` names a pin of the chip as `GPIO<n>`. On the
ESP32-C6-DevKitC-1 this `io.toml` reads the BOOT button, `GPIO9`, which is low while it is pressed,
and drives `GPIO18`:

```toml
[io]
driver = "gpio"

[io.params]
inputs = [{ address = "%IX0.0", line = "GPIO9", invert = true }]
outputs = [{ address = "%QX0.0", line = "GPIO18", initial = false }]

[[io.safe_state]]
address = "%QX0.0"
value = "FALSE"
```

`gpio` is the only `io.toml` driver the chip carries out today. [Supported Chips](supported-chips.md)
lists the pins the firmware keeps.

Build the program image for the chip:

```bash
trust-runtime build --project ./my-c6-project --target esp32c6
```

The build compiles the project for the chip's CPU, `riscv32imac-unknown-none-elf`, and writes into
the project's `build` folder:

- `program.riscv32imac-unknown-none-elf.tpi`, the program image;
- `deployment-record.riscv32imac-unknown-none-elf.bin`, its deployment record;
- `install.riscv32imac-unknown-none-elf.bin`, the two packed into the one file `ctl install`
  takes;
- `build.json`, the build report.

It prints the image's size, its memory figure, and the public key that signed it
(`Signed by the key <64 hexadecimal digits>`). The first build installs the compiler toolchain;
`trust-runtime toolchain status` names the installed one.

- Build with the same truST release as the firmware on the chip. A chip refuses an image of
  another release and asks for the project to be rebuilt.
- The build writes a debug image by default. `[build] debug = false` in the project's
  `trust-lsp.toml` writes a release image, which is smaller and has no debug hooks.

## 4. Install The Program And Start It

`ctl install` uploads a program container to the stopped chip, installs it into the program slot
the chip does not run from, and with `--restart` starts it:

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 install \
  ./my-c6-project/build/install.riscv32imac-unknown-none-elf.bin --restart cold
```

```text
installed sequence=<n> program_sha256=<64 hexadecimal digits> bytes=<n>
restarted mode=cold state=running
```

- A program that runs is not replaced: stop it first with
  `trust-runtime ctl --endpoint serial:///dev/ttyACM0 shutdown`. The install is otherwise refused
  with `resource_not_running`.
- The chip refuses a program whose memory figure is larger than the memory it has for programs,
  and names both numbers. Nothing is written then.
- Without `--restart` the installed program does not start;
  `trust-runtime ctl --endpoint serial:///dev/ttyACM0 restart cold` starts it.
- At every boot the chip starts its installed program: warm when the project keeps retained values
  (`[runtime.retain] mode = "file"`), cold otherwise.

Check that it runs:

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 status
```

```text
state=running fault=none rt_profile=disabled rt_active=false
```

`ctl io-read`, `ctl events` and `ctl faults` print the process image, the events and the faults as
the chip's raw replies.

## 5. Join A Wi-Fi Network

The chip keeps its own settings, [`device.toml`](device-toml.md), on the chip and not in the project.
Write the file on the computer:

```toml
[device]
name = "line-3"

[network.wifi]
ssid = "plant-net"
password = "the-wifi-passphrase"
country = "SE"

[control]
token = "CHANGE-ME-16-to-128-chars"
```

Choose your own token: 16 to 128 printable ASCII characters without spaces. The chip serves its
control connection over the network only when the settings set a token, and every request must then
carry it, over USB too.

Store the settings while the program is stopped, then reset the chip:

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 shutdown
trust-runtime ctl --endpoint serial:///dev/ttyACM0 device-set device.toml
trust-runtime ctl --endpoint serial:///dev/ttyACM0 device-reset
```

```text
stored sequence=<n>; the settings apply at the device's next boot (trust-runtime ctl device-reset)
resetting
```

The chip boots with its settings: it starts the network before its program, joins the Wi-Fi
network without making the program wait, and starts its installed program. From now on every `ctl`
command needs the token, from `--token <token>` or from the environment variable
`TRUST_CTL_TOKEN`:

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 --token CHANGE-ME-16-to-128-chars status
```

!!! warning "Keep the token and the file"
    `device-get` never shows the Wi-Fi password or the token, and a new `firmware install` keeps
    the stored settings. truST has no command that changes the settings without the token.

Wi-Fi takes 95,932 bytes of the chip's heap at boot, before the chip measures its memory for
programs ([Supported Chips](supported-chips.md)). A program that fits a chip without Wi-Fi can be
refused at the first boot with Wi-Fi; the chip then stays Stopped with the reason, and a smaller
program, such as a release image, can be installed.

## 6. Find The Chip On The Network

```bash
trust-runtime discover --only trust_runtime
```

A chip that serves its control connection announces itself over mDNS as every truST runtime does:
the service `_trust._plc._tcp.local.` with the instance `truST-<device name>` and the host
`<device name>.local.`. The search prints one line for each device it finds,
`Found <address>: truST runtimes (...)`. With `--json`, each find carries the chip's
`control_endpoint`, `tcp://<address>:<port>`.

mDNS reaches only the computers on the chip's local network. The address is also in the DHCP
server's list of leases, under the device name as host name, or `trust-` and the last six
hexadecimal digits of the chip's MAC address without a name. A fixed address in `device.toml` makes
it known in advance. The chip answers ping on its address.

## 7. Control Over The Network

```bash
trust-runtime ctl --endpoint tcp://192.168.1.50:9901 --token CHANGE-ME-16-to-128-chars status
```

```text
state=running fault=none rt_profile=disabled rt_active=false
```

- The port is 9901 unless `[control] port` in `device.toml` sets another.
- Every `ctl` command works over this endpoint as over USB, `install` included.
- The chip serves one network client at a time. A second client is told `server busy`
  (`server_busy`); `ctl` connects again a few times within about a second and a half before it
  reports the busy chip.
- The chip reads its USB link and its network connection in turn, and answers each request on the
  link it came from.

## If Something Does Not Work

| Symptom | Check |
| --- | --- |
| `firmware install` finds no port | the cable is on the board's native USB port; give `--port` |
| `serial port <name> went away ...` | the chip reset, lost power or was unplugged; run the command again once its port is back |
| `resource_not_running` from `install`, `device-set` or `device-reset`, or `stop it first` from `firmware` | the program runs: `ctl shutdown` first |
| `missing auth token` | the chip has a control token: give `--token` or set `TRUST_CTL_TOKEN` |
| the chip is not found on the network | the SSID, the password (WPA2-Personal or open) and `country`; the computer is on the chip's local network; the settings set a control token, without which the chip serves no control connection over the network |
| `rebuild the project` | the program was built by another truST release than the chip's firmware |

## Related

- [Supported Chips](supported-chips.md)
- [Install And Update truST On A Chip](chip-install-and-update.md)
- [`device.toml`](device-toml.md)
- [`trust-runtime` CLI](../reference/cli/trust-runtime.md)
- [`io.toml`](../reference/config/io-toml.md)
- [Specification 34, Runtime Portability](../reference/specifications/34-runtime-portability.md): TARGET-01 to TARGET-06, PORT-05, PORT-06
