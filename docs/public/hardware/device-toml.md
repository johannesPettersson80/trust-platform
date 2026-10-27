# `device.toml` Device Settings

A chip has no operating system, so it keeps the settings that the operating system holds on a
computer in `device.toml`: its name, its network, its control token, Bluetooth, its time server, the
keys whose signed programs it trusts, and the wiring values its firmware cannot detect. The file is
stored on the chip, not in the project, so no Wi-Fi password or token reaches the project.

| What | Files | Where they live |
| --- | --- | --- |
| What the PLC does | `runtime.toml`, `io.toml`, the program | the project |
| The device itself | `device.toml` | on the device |
| The chip | `chips/<chip>.toml` | inside truST |

Pins are not device settings: `io.toml` names them, as on Linux.

## Example

```toml
[device]
name = "line-3"

[network.wifi]
ssid = "plant-net"
password = "the-wifi-passphrase"
country = "SE"

[control]
token = "CHANGE-ME-16-to-128-chars"

[time]
server = "pool.ntp.org"
```

```bash
trust-runtime ctl --endpoint serial:///dev/ttyACM0 device-set device.toml
trust-runtime ctl --endpoint serial:///dev/ttyACM0 device-reset
```

## Rules For Every Key

- Every section may be left out, and so may every key but `ssid` in `[network.wifi]`.
- An unknown section or key, and a value of the wrong type, are refused.
- A key for hardware the chip lacks is refused with the reason, for example
  `network.ethernet: the ESP32-C6 has no Ethernet`.
- The file is stored whole: `device-set` replaces every stored setting with the file's. A key left
  out of the file is not kept from the settings before.
- The settings, as JSON text, must fit one flash sector: 4,076 bytes on the ESP32-C6. Longer
  settings are refused with `the settings do not fit the device's 4076 bytes`.

## `[device]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `name` | string | none | 1 to 32 letters, digits and hyphens, not starting or ending with a hyphen (a DNS label) |

The chip gives its name to the DHCP server as its host name and announces it in discovery. Without
a name it uses `trust-` and the last six hexadecimal digits of its MAC address.

## `[network.wifi]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `ssid` | string | required in this section | 1 to 32 bytes |
| `password` | string | empty: an open network | empty, a WPA2 passphrase of 8 to 63 printable ASCII characters, or a pre-shared key of 64 hexadecimal digits |
| `address` | string | `dhcp` | `dhcp`, or a fixed IPv4 address with its prefix length, such as `192.168.1.50/24` (prefix 1 to 32) |
| `gateway` | string | none | an IPv4 address; only with a fixed address |
| `dns` | array of strings | none | at most two IPv4 addresses; only with a fixed address |
| `country` | string | none | the ISO 3166-1 alpha-2 code of the country whose radio rules apply, in two capital letters: `SE`, `DE`, `US` |

- The chip joins the network as a station with WPA2-Personal, or without security when the
  password is empty. The ESP32-C6's radio driver has no WPA3 station.
- Without `country`, the chip keeps to the channels and transmit power allowed in every country.
- With DHCP, the gateway and the name servers come from the DHCP server; giving `gateway` or `dns`
  with DHCP is refused.
- The chip starts the network at boot, before its program, and the program never waits for it.
  When joining fails or the network is lost, the chip tries again after 1, 2, 4, 8 and 16 seconds
  and then every 30 seconds, and 90 seconds after an attempt that the access point did not answer.
  An attempt that takes longer than 20 seconds is given up.
- A chip without `[network.wifi]` starts no radio, and its memory for programs is not reduced (see
  [Supported Chips](supported-chips.md)).

## `[network.ethernet]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `address` | string | `dhcp` | as in `[network.wifi]` |
| `gateway` | string | none | as in `[network.wifi]` |
| `dns` | array of strings | none | as in `[network.wifi]` |

Neither supported chip has Ethernet. The ESP32-C6 refuses the section with
`network.ethernet: the ESP32-C6 has no Ethernet`.

## `[control]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `token` | string | none | 16 to 128 printable ASCII characters, without spaces |
| `port` | integer | `9901` | the TCP port of the control connection over the network, 1 to 65535 |

- With a token, the chip serves its control connection over its network on `port`, and every
  request on every link must carry the token, over USB too. The token grants the Admin role, as
  `runtime.control.auth_token` does on a computer.
- Without a token, the chip serves no control connection over its network. A request over USB
  then acts with the Admin role.
- `trust-runtime ctl` sends the token from `--token`, or from the environment variable
  `TRUST_CTL_TOKEN`. A request without it is refused with `missing auth token`, and one with
  another token with `invalid auth token`.

!!! warning "Keep the token"
    The chip asks for the token on every link, and a new `firmware install` keeps the stored
    settings. If the token is lost, install the firmware again with `--reset-settings` over USB:
    `trust-runtime firmware install --chip esp32c6 --reset-settings <folder>` erases the device
    settings and the pairings, so the chip boots without a token, without pairing tokens and
    without a network, and its USB link takes every request again. Then write new settings with
    `ctl device-set`.

## `[bluetooth]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `enabled` | boolean | `false` | `true` is refused on a chip without Bluetooth: `bluetooth: the <chip> has no Bluetooth` |

The ESP32-C6 accepts `enabled = true`. Its firmware does not use Bluetooth yet.

## `[time]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `server` | string | none | a host name or an IPv4 address |

- The chip asks the time server with SNTP version 4 once it has an address, then every hour, and a
  minute after a request that failed. A host name is looked up at the name servers of the address
  in use, from DHCP or from `[network.wifi]`.
- A chip has no battery-held clock. Until a time source sets its clock after a loss of power, the
  clock counts from `DT#1970-01-01-00:00:00` at power-up, and `CURRENT_DT` reads that. The program
  keeps running.
- The ESP32-C6 keeps its time across every reset but a loss of power.
- Without a time server, `trust-dev test --device` and `trust-runtime conformance --device` give
  the chip the computer's clock when they open its link.

## `[signing]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `trusted_keys` | array of strings | empty | at most 8 Ed25519 public keys, each 64 hexadecimal digits |

```toml
[signing]
trusted_keys = ["<the 64 hexadecimal digits that trust-runtime build prints>"]
```

- `trust-runtime build` signs every program image it writes and prints the public key it signed
  with: `Signed by the key <64 hexadecimal digits>`.
- With an empty list, the chip checks no signature and admits signed and unsigned programs alike.
  `status` reports `signature: none`.
- With keys, the chip refuses a program that is unsigned or signed by another key
  (`the program is not signed by a key this device trusts`) and one changed after signing
  (`the program's signature does not verify: it was changed after signing`). `status` reports
  `signature: trusted`. An online change is checked against the same list.
- The chip reads the list once at boot, so a changed list applies from its next boot.

## `[hardware]`

| Key | Type | Default | Rules |
| --- | --- | --- | --- |
| `crystal_hz` | integer | none | inside the crystal range of the chip file; the ESP32-C6 takes 40,000,000 only |
| `ethernet_phy` | string | none | only on a chip with Ethernet; the ESP32-C6 refuses it |

Name these values only when the firmware cannot detect them.

## Write, Read And Reset

| Command | Role | Program state | Prints |
| --- | --- | --- | --- |
| `trust-runtime ctl device-set <file>` | Admin | Stopped | `stored sequence=<n>; the settings apply at the device's next boot (trust-runtime ctl device-reset)` |
| `trust-runtime ctl device-get` | Engineer | any | `# device settings, sequence <n>`, then the stored settings as `device.toml` text, or `no device settings are stored` |
| `trust-runtime ctl device-reset` | Admin | Stopped | `resetting` |

- The commands take `--endpoint`, `--token` and `--project` like every `ctl` command, and work over
  USB (`serial://`) and over the network (`tcp://`). The first `device-set` goes over USB.
- Stop the program first with `trust-runtime ctl shutdown`: storing the settings erases a flash
  sector, which masks the chip's interrupts, so the chip stores them only while it is Stopped.
- `device-get` never shows the Wi-Fi password or the control token: in their place it shows
  `password_set` and `token_set`. Keep your `device.toml` file on the computer, because the
  password cannot be read back.
- `device-reset` resets the chip 100 ms after its reply. The chip leaves its Wi-Fi network first,
  then boots with its stored settings and starts its installed program, as at any reset.

`device-set` refuses, in this order: a request without settings (`missing params`); settings that
are not an object (`invalid params: settings must be an object`); settings that do not read as
`device.toml`, such as an unknown section or key or a value of the wrong type
(`invalid params: <reason>`); a hung program; a program that is not Stopped
(`resource_not_running`); a setting the chip cannot take, with the key and the reason; and a flash
failure. A file that is not TOML is refused by `ctl` before anything is sent:
`<file> is not a TOML file`.

## When A Change Applies

At the chip's next boot. `device-reset` starts it, and so does any other reset: a power cycle, the
reset button, or the restart after a firmware update. The boot reads the settings before the network
and the program start. Until then `device-get` shows the stored settings while the chip still runs
with those it booted with.

## How A Chip Stores The File

- The ESP32-C6 keeps the settings in two 4 KiB flash sectors, at `0xD000` and `0xE000`.
- Each sector holds one record: the magic `TRSTDS01`, a sequence, the length and CRC-32 of the text,
  and the settings as JSON.
- The newer of two whole records holds the settings. A write goes into the sector that does not
  hold the newest record, with the next sequence, and is read back. A write that a loss of power
  ends leaves the settings before it.
- `firmware install` and firmware updates leave both sectors as they are, so a chip keeps its
  settings across them.

## On A Computer

`trust-runtime` on Linux, macOS and Windows reads one key of `device.toml`,
`[signing] trusted_keys`, from truST's configuration folder of the user that runs it:

| Computer | File |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/trust/device.toml`, else `~/.config/trust/device.toml` |
| macOS | `~/Library/Application Support/trust/device.toml` |
| Windows | `%APPDATA%\trust\device.toml` |

The operating system holds the computer's other settings. A computer answers `device-get`,
`device-set` and `device-reset` with `unsupported request`. A computer whose `device.toml` cannot be
read admits no program and names the file's error.

## STM32U5A9

The STM32U5A9's firmware keeps no device settings yet: it answers the three commands with
`unsupported request`.

## Related

- [ESP32-C6 Getting Started](esp32c6-getting-started.md)
- [Supported Chips](supported-chips.md)
- [`trust-runtime` CLI](../reference/cli/trust-runtime.md)
- [`runtime.toml`](../reference/config/runtime-toml.md)
- [Specification 34, Runtime Portability](../reference/specifications/34-runtime-portability.md): TARGET-02, TARGET-06, PORT-06
- [Specification 11, Runtime Engine](../reference/specifications/11-runtime-engine.md): the device settings contract
- [Specification 44, Native Execution](../reference/specifications/44-native-execution.md): §5.11 signatures
