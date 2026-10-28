# Comm Library Guide

The Comm library gives the program and the HMI the state of every communication link the runtime
runs: an `io.toml` driver (Modbus TCP, MQTT, EtherCAT, GPIO, simulated, loopback), an ADS client
connection or an OPC UA client connection. The program declares a `Link` block naming its link;
the runtime fills the block's outputs at the start of every scan. The program uses them to stop
equipment safely when a link fails, and the operator opens the link faceplate to see which link
failed, since when and why. The contract is specification 42,
`docs/specs/42-communication-links.md`.

## Package Setup

```toml
[project]
include_paths = ["src"]

[dependencies]
Alarm = { path = "../../libraries/alarm", version = "0.1.0" }
Comm = { path = "../../libraries/comm", version = "0.1.0" }
```

The block's alarm comes from the Alarm library, so a project adds both. The program writes
`USING Comm;`.

## Design Rules

- One `Link` per link, called once per scan with the link's `Name`.
- The runtime writes the outputs at the start of each scan, after the inputs are read and before
  the program runs. The program reads them and never writes them.
- A block first called with its `Name` in one scan is filled from the next scan; until then it
  reads `Unknown`.
- The program decides what a failed link means for the process (interlocks, safe state, holding
  the last value); the block's alarm tells the operator.

## Link Names

A link's name is the identity Devices & Connections uses:

| Link | Name |
| --- | --- |
| an `io.toml` driver | `<protocol>:<position>`, its position in `io.toml` from 0: `modbus_tcp:0`, `mqtt:1`, `ethercat:0`, `gpio:0`, `simulated:0`, `loopback:0` |
| an ADS client connection | `ads:<connection>`, for example `ads:line` |
| an OPC UA client connection | `opcua_client:<connection>`, for example `opcua_client:plant` |

## Blocks

| Block | Use | Faceplate | Symbol |
| --- | --- | --- | --- |
| `Link` | the state of one link, filled by the runtime, with its alarm | link | network-link |

## Common Types

`LinkState`:

| State | Meaning |
| --- | --- |
| `Unknown` | the runtime runs no link of that name, or has not read it yet |
| `Connecting` | the link is being established |
| `Connected` | data is exchanged |
| `Degraded` | data is exchanged with problems, for example stale or bad values |
| `Failed` | no data is exchanged |
| `Disabled` | the link is configured but switched off |

A driver is `Connected` while healthy, `Degraded` while degraded and `Failed` while faulted, with
the driver's error text. An ADS or OPC UA client connection is `Degraded` while any of its points
is degraded, and `Failed` while it reconnects or has faulted.

## Interface Summary

```st
Link   Name, AlarmDelay (10 s), Enable (TRUE), Test, Reset
       -> State, Connected, Disabled, Known, StateTime, LastGoodAge, LastError,
          Exchanges, Failures, ResponseTime, TestResult, Detail; alarm LinkFailed
```

- `State` is the link's state, a `LinkState`.
- `Connected` is TRUE while data is exchanged: `Connected` or `Degraded`.
- `Disabled` is TRUE while the link is switched off on purpose, by `Enable` or in its
  configuration.
- `Known` is FALSE when the runtime runs no link of that name; `LastError` then says so.
- `StateTime` is how long the link has been in its state; `LastGoodAge` how long ago data was last
  exchanged, 0 while it is.
- `LastError` keeps the latest error after the link recovers.
- `LinkFailed` is a Medium alarm, "`<Name>` failed", raised when no data has been exchanged for
  `AlarmDelay`: the link failed, its name is unknown to the runtime, or it never finished
  connecting. A link switched off on purpose (`Disabled`) raises none; `LastError` tells the
  causes apart.
- `Enable` FALSE switches an `io.toml` driver's link off from the next scan: the runtime neither
  reads nor writes the driver, so the program keeps the last input values and the device the last
  outputs. The link reads `Disabled` and raises no alarm. One block with `Enable` FALSE is
  enough. ADS and OPC UA client connections are switched in their configuration; `Enable` does
  not reach them.
- A rising edge of `Test` reads the driver once into a spare image, which the program does not
  see, and writes the outcome to `TestResult`: "answered in 12 ms", or the error. A client
  connection is not tested this way, and `TestResult` says so.
- `Exchanges` counts the scans in which data was exchanged, `Failures` the scans in which the link
  was `Failed`. A rising edge of `Reset` sets both to 0 for every block of the link.
- `ResponseTime` is how long an `io.toml` driver's last read and write took; 0 for a client
  connection, which exchanges in the background.
- `Detail` is the protocol's own state in its words: a Modbus TCP server and unit and its last
  exception reply, an MQTT broker and session, what EtherCAT discovery found, a GPIO chip and its
  lines; an ADS client connection's summary or an OPC UA client connection's session detail.
  Empty when the protocol reports none.
- An input the program's call passes is the program's each scan; the faceplate's switches work on
  the inputs the call leaves out.

## Operating From The HMI

Place the `network-link` symbol, bind its `connected` to the block's `Connected` and its
`out-of-service` to `Disabled`, and give it `[object.click] faceplate = "link"`. A link switched
off on purpose then reads Off, out of service, not lost. The link faceplate shows the link's name, its state and how
long it has been in it, whether data is exchanged and when it last was, its latest error, its
exchanges, failures and response time, its protocol's own state, and its alarm. For engineers it has **Link on**
(`Enable`), **Test link** (`Test`, with `TestResult` under it) and **Reset counters** (`Reset`);
only an engineer may use them. Everything else on it is the runtime's.

The runtime writes them only within its write policy (`[[runtime.web.hmi.write]]` in
`runtime.toml`), for example:

```toml
[[runtime.web.hmi.write]]
path = "Main.RemoteIo.Enable"
role = "engineer"

[[runtime.web.hmi.write]]
path = "Main.RemoteIo.Test"
role = "engineer"

[[runtime.web.hmi.write]]
path = "Main.RemoteIo.Reset"
role = "engineer"
```

## Examples

A transfer pump stops when the link to its remote I/O fails:

```iecst
USING Comm;

PROGRAM Transfer
VAR
    RemoteIo : Link;
    StartRequest : BOOL;
    PumpRun : BOOL;
END_VAR
    RemoteIo(Name := 'modbus_tcp:0', AlarmDelay := T#5s);
    PumpRun := StartRequest AND RemoteIo.Connected;
END_PROGRAM
```

A line that keeps the last good value of a remote measurement while its OPC UA link is down, and
says how old it is:

```iecst
USING Comm;

PROGRAM Line
VAR
    Plant : Link;
    RemoteFlow : LREAL;
    Flow : LREAL;
    FlowAge : TIME;
END_VAR
    Plant(Name := 'opcua_client:plant');
    IF Plant.State = LinkState#Connected THEN
        Flow := RemoteFlow;
    END_IF;
    FlowAge := Plant.LastGoodAge;
END_PROGRAM
```

A remote I/O station is taken off its link while it is serviced, with no alarm, and tested before
it goes back:

```iecst
USING Comm;

PROGRAM Station
VAR
    RemoteIo : Link;
    Servicing : BOOL;
    TestRequest : BOOL;
    Answer : STRING[120];
END_VAR
    RemoteIo(Name := 'modbus_tcp:0', Enable := NOT Servicing, Test := TestRequest);
    Answer := RemoteIo.TestResult;
END_PROGRAM
```

## Validation

```bash
cargo test -p trust-runtime --test comm_links
cargo test -p trust-runtime --test comm_link_actions
cargo test -p trust-runtime --test comm_faceplates
cargo test -p trust-runtime --test comm_guide
cargo test -p trust-runtime --lib link_subsystem
```

`comm_links` drives a link through its states and checks an unknown name, a disabled driver and
the alarm; `comm_link_actions` switches a driver's link off and on, tests it and checks its
counters, their reset and its response time against a simulated device; `comm_faceplates` checks the pair and the faceplate against the library; `comm_guide`
compiles and runs every example of this guide; the unit tests map every state of ADS and OPC UA
client connections.
