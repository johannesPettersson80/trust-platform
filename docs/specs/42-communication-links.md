# Communication links

> **Plant integration, 7 October 2026:** FB-backed HMI authoring on a different machine deploys through the selected existing runtime connection. Plant components use ordinary PLC I/O; their HMI commands and settings ([trust-plant-library.md §12](trust-plant-library.md)) do not replace communications, drivers or the simulator.

Status: decided with the user on 3 October 2026: a link's status reaches the program as variables,
and the HMI shows it through them, as it shows equipment. This step delivers the state of each
link, its counters and response time, and switching and testing it (§4); each protocol's own
state and stale values follow in later steps (§7).

## 1. Purpose

A PLC program must know when a link to a device fails, to stop equipment safely, and an operator
must see which link failed, since when, and why. The runtime knows each link's state; the program
reads it through a `Comm.Link` block, and the HMI's link faceplate binds to that block like any
equipment (specification 39, §8.7).

## 2. Links and their names

A link is one connection the runtime runs to another system. Its name is the identity
Devices & Connections uses:

| Link | Name |
|---|---|
| a driver of `io.toml` (Modbus TCP, MQTT, EtherCAT, GPIO, simulated, loopback) | `<protocol>:<position>`, its position in `io.toml` from 0, for example `modbus_tcp:0`, `mqtt:1` |
| an ADS client connection | `ads:<connection>`, for example `ads:line` |
| an OPC UA client connection | `opcua_client:<connection>` |

The protocols of `io.toml` drivers are `modbus_tcp`, `mqtt`, `ethercat`, `gpio`, `simulated` and
`loopback` (driver aliases such as `sim` take their protocol's name).

## 3. The state of a link

| State | Value | Meaning |
|---|---|---|
| `Unknown` | 0 | the runtime runs no link of that name, or has not read it yet |
| `Connecting` | 1 | the link is being established |
| `Connected` | 2 | data is exchanged |
| `Degraded` | 3 | data is exchanged with problems, for example stale or bad values |
| `Failed` | 4 | no data is exchanged |
| `Disabled` | 5 | the link is configured but switched off |

1. **I/O drivers:** a healthy driver is `Connected`, a degraded one `Degraded` and a faulted one
   `Failed`, each with the driver's error text; a driver configured with `enabled = false` is
   `Disabled`.
2. **ADS client connections**, by the state their status report gives: connected is `Connected`,
   or `Degraded` while any of its points is degraded; stale is `Degraded`; not ready is
   `Connecting`; reconnecting and faulted are `Failed`; disabled is `Disabled`; unknown is
   `Unknown`.
3. **OPC UA client connections:** connected is `Connected`, or `Degraded` while any point is
   degraded; configured and connecting are `Connecting`; stale is `Degraded`; reconnecting and
   faulted are `Failed`; disabled is `Disabled`.
4. **Since when:** the runtime notes when each link's state changes and when it was last
   `Connected` or `Degraded`, that is, when data was last exchanged.

## 4. The `Comm.Link` block

The Comm library (`libraries/comm`) holds the block:

| Member | Kind | Meaning |
|---|---|---|
| `Name : STRING[80]` | input | the link's name (§2) |
| `AlarmDelay : TIME := T#10s` | input | how long the link may be failed before its alarm |
| `State : LinkState` | output | the link's state (§3) |
| `Connected : BOOL` | output | data is exchanged: `Connected` or `Degraded` |
| `Disabled : BOOL` | output | the link is switched off on purpose: `Disabled` |
| `Known : BOOL` | output | the runtime runs a link of that name |
| `StateTime : TIME` | output | how long the link has been in its state |
| `LastGoodAge : TIME` | output | how long ago data was last exchanged; 0 while it is |
| `LastError : STRING[120]` | output | the latest error the link reported, kept after it recovers |
| `Enable : BOOL := TRUE` | input | FALSE switches an `io.toml` driver's link off (item 6) |
| `Test : BOOL` | input | on its rising edge the runtime tests the driver once (item 7) |
| `Reset : BOOL` | input | on its rising edge the link's counters start again from 0 |
| `Exchanges : UDINT` | output | the scans in which data was exchanged, since the runtime started or the counters were reset |
| `Failures : UDINT` | output | the scans in which the link was `Failed`, counted alike |
| `ResponseTime : TIME` | output | for an `io.toml` driver, how long its last read and write took; 0 for a client connection, which exchanges in the background |
| `TestResult : STRING[120]` | output | the outcome of the last test: "answered in 12 ms", or the error |
| `Detail : STRING[120]` | output | the protocol's own state in words (item 9); empty when it reports none |
| `LinkFailed : Alarm.DigitalAlarm` | member | Medium alarm "`<Name>` failed" while no data is exchanged for `AlarmDelay`: `Failed`, `Unknown` or `Connecting`, never `Disabled` |

1. **Filled by the runtime:** at the start of every scan, after the inputs are read and before
   the program runs, the runtime writes the outputs of every `Comm.Link` instance at any depth of
   the program, from the link its `Name` names. The program does not write them.
2. **Calling it:** the program calls the block once per scan with its `Name`, which evaluates its
   alarm. A block first called with its `Name` in one scan is filled from the next scan; until
   then it reads `Unknown`.
3. **An unknown name** reads `Unknown` with `Known` FALSE and `LastError` "no link `<Name>` in
   this runtime"; its `StateTime` and `LastGoodAge` count from the scan the runtime first looked
   for it.
4. **The alarm** (decided on 3 October 2026 with the user's leave to choose): a misnamed link,
   or one that never finishes connecting, stops the process as a failed one does, so each raises
   the alarm once its delay has passed; the delay covers start-up, and the latest error tells the
   causes apart. A link switched off on purpose raises none.
5. **Cost:** a program without the Comm library is not searched; the instances are found by a
   walk of the program's instances, again once a second and at once when one is gone (an online
   change).
6. **Switching a link off** (decided on 3 October 2026 while the user was away, approved by the user on 3 October 2026): while
   any block naming an `io.toml` driver's link has `Enable` FALSE, the runtime neither reads nor
   writes that driver, from the next scan on: the program keeps the last input values, the device
   the last outputs. The link reads `Disabled` and raises no alarm. ADS and OPC UA client
   connections are switched in their configuration; `Enable` does not reach them.
7. **Testing a link:** on a rising edge of `Test`, the runtime reads the driver once into a spare
   image, which the process image does not take, and writes the outcome to `TestResult`. A
   client connection is not tested this way; its `TestResult` says so.
8. **Counting:** each scan the link is `Connected` or `Degraded` counts one exchange, each scan it
   is `Failed` one failure; a rising edge of `Reset` sets both to 0 for every block of the link.
9. **The protocol's own state** (decided on 3 October 2026, when the user asked to finish the
   checklist): beside its health, each `io.toml` driver may describe its protocol's state in
   words, which the block gives as `Detail`, cut to 120 characters:
   - Modbus TCP: the server and unit it reads, and, after an exception reply, its code and name
     from the Modbus application protocol (for example "10.0.0.5:502 unit 1, exception 02
     illegal data address");
   - MQTT: the broker, and whether the session is connected or why it is not;
   - EtherCAT: what discovery found on the bus (the slaves), or why it failed;
   - GPIO: the chip and the number of lines it serves;
   - the simulated and loopback drivers report none.
   An ADS client connection gives its status summary and an OPC UA client connection its detail
   (its session and certificate state), as their status reports carry them.

## 5. The HMI

1. **Faceplate `link`** (the Comm library's `hmi/panels/link.toml`, paired with `Comm.Link`):
   the link's state with its word and since when, whether data is exchanged and when it was last,
   its latest error, its exchanges, failures and response time, its protocol's own state
   (`Detail`), and its alarm; for engineers
   (`role = "engineer"`) a switch on `Enable`, a pulse button on `Test` with `TestResult` under
   it and a pulse button on `Reset`. Nothing else on it is written.
2. **Symbol:** the `network-link` symbol shows the link, `connected` bound to the block's
   `Connected` and `out-of-service` to `Disabled` (the pair's `[bind]`), so a link switched off
   on purpose reads Off, out of service, never lost; a click opens the faceplate.
3. The program decides what a failed link means for the process (interlocks, safe state); the
   link's alarm tells the operator.
4. **Values of a link without data** (decided on 3 October 2026, when the user asked to finish
   the checklist): each `io.toml` driver declares the bytes of the input image it fills, from its
   own configuration (Modbus TCP its block or points, MQTT its input mappings, EtherCAT its input
   image, GPIO its input lines). A page value whose variable is located at such an input address
   (an `AT %I…` declaration or a `VAR_CONFIG` entry) is stale while that driver's link exchanges
   no data (`Failed` or `Disabled`), with the link's time since its last data as its age, and is
   shown as any stale value (§5.3 of specification 39). A value the program computes from inputs
   is not marked: the program decides what a failed link means, through its `Link` blocks. A
   driver that declares no bytes marks nothing. The runtime looks at each call of `Hmi.Display`
   (specification 39, §18 phase 5); a link it has never seen exchange data counts from when it
   first saw it. A Modbus TCP block and a raw MQTT input topic fill the image from its start and
   so declare every byte.

## 6. Tests

The runtime tests drive a link through its states with a test driver and assert the block's
outputs, an unknown name, a disabled driver and the alarm; the mappings of §3 are tested for every
state of ADS and OPC UA client connections; the library and its faceplate are checked like the
Plant library's (specification `trust-plant-library`).

## 7. Later steps

Planned, each specified before it is built:
- the EtherCAT slaves' states and working counter, and the ADS remote PLC's state, in `Detail`:
  needs the bus and the ADS client to report them;
- timeouts counted apart from other failures, per request rather than per scan;
- the OPC UA and ADS servers' clients, and the runtime-to-runtime link.
