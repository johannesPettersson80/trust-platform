# Two I/O Drivers: Modbus/TCP and MQTT

Run a conveyor drive from two places at once: the drive itself over Modbus/TCP, and the line's
supervisory system over MQTT. You learn how `io.drivers` puts two I/O drivers in one runtime, why
each must fill only its own inputs, and what both get when the runtime stops.

## What you learn

- `io.drivers` runs several I/O drivers in one runtime. Before each cycle every driver reads its
  inputs into the one input image, in the order of the list, and after the cycle every driver gets
  the whole output image.
- Two drivers that fill the same inputs overwrite each other, so each one here fills only its own:
  the Modbus/TCP driver through `input_points` and `output_points`, which name a bit of the image
  and a Modbus address, and the MQTT driver through `mappings`, which name a variable of the
  program and a topic.
- `[[io.safe_state]]` reaches every driver at a stop, and a stop that a driver cannot confirm ends
  faulted.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, and three terminals.
- A Modbus/TCP server on `127.0.0.1:1502` that stands in for the drive: its discrete input 0,
  "drive ready", holds TRUE, and it shows coil 0, the run command that the runtime writes. This
  script, for Python 3 with the package pyModbusTCP (`pip install pyModbusTCP`, in a virtual
  environment where your system asks for one), is such a server. Save it as `drive_server.py`:

    ```python
    from time import sleep
    from pyModbusTCP.server import ModbusServer

    server = ModbusServer("127.0.0.1", 1502, no_block=True)
    server.start()
    server.data_bank.set_discrete_inputs(0, [True])
    while True:
        print("coil 0:", server.data_bank.get_coils(0)[0])
        sleep(1)
    ```

- An MQTT broker on `127.0.0.1:1883` that stands in for the supervisory system, with its
  command-line clients: Mosquitto (`mosquitto`, `mosquitto_pub` and `mosquitto_sub`) from your
  system's packages, for example. Start the broker with `mosquitto` if your system did not start
  it as a service when you installed it.
- About 20 minutes.

## Step by step

1. Open a terminal in `examples/communication/multi_driver` and build the project:

    ```text
    trust-runtime build --project . --sources src
    ```

    It writes the program's image, here on an x86-64 Linux computer, and then names the image's
    memory figure, the key that signed it and its two sources, `src/config.st` and `src/main.st`:

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 2148 bytes: code 713, read-only data 0, starting memory 3
    ```

    The program `Main` runs the drive when the drive is ready and the line asks for it,
    `Run := DriveReady AND RemoteStart;`, and `src/config.st` puts `DriveReady` at `%IX0.0`,
    `RemoteStart` at `%IX1.0` and `Run` at `%QX0.0`.

2. Open `io.toml`. It lists the two drivers, each in a `[[io.drivers]]` table of its own, and the
   safe state of the output:

    ```toml
    # The drive: a Modbus/TCP device whose discrete input 0 says the drive is ready and whose coil 0
    # runs it. Its points fill %IX0.0 and write %QX0.0, and nothing else of the image.
    [[io.drivers]]
    name = "modbus-tcp"

    [io.drivers.params]
    address = "127.0.0.1:1502"
    unit_id = 1
    timeout_ms = 500
    on_error = "fault"

    [[io.drivers.params.input_points]]
    image_offset = 0
    image_bit = 0
    address = 0
    function = "read_discrete_inputs"
    data_type = "bool"

    [[io.drivers.params.output_points]]
    image_offset = 0
    image_bit = 0
    address = 0
    function = "write_single_coil"
    data_type = "bool"

    # The supervisory system: an MQTT broker. The mappings name the program's variables: the remote
    # start comes from one topic, and the run state goes to another. They fill RemoteStart's bit,
    # %IX1.0, and nothing else of the image.
    [[io.drivers]]
    name = "mqtt"

    [io.drivers.params]
    broker = "127.0.0.1:1883"
    reconnect_ms = 500
    keep_alive_s = 5
    on_error = "warn"
    mappings = [
      { tag = "P1.RemoteStart", topic = "trust/examples/multi/start", direction = "read" },
      { tag = "P1.Run", topic = "trust/examples/multi/running", direction = "write" },
    ]

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    ```

    The Modbus/TCP driver reads discrete input 0 into `%IX0.0` and writes `%QX0.0` to coil 0. The
    MQTT driver sets `P1.RemoteStart` from the topic `trust/examples/multi/start` and publishes
    `P1.Run` to `trust/examples/multi/running`. Neither driver fills an input of the other.

3. Check the configuration before you start anything:

    ```text
    trust-runtime validate --project .
    ```

    It answers `Project ok`.

4. Start the drive's stand-in in the second terminal with `python3 drive_server.py`; it prints
   `coil 0: False` each second. Then start the runtime in the first terminal:

    ```text
    trust-runtime run --project .
    ```

5. In the third terminal, read the process image:

    ```text
    trust-runtime ctl --project . io-read
    ```

    The drive is ready,
    `{"address":"%IX0.0","forced":false,"name":"P1.DriveReady","value":"Bool(true)","valueType":"BOOL"}`,
    but the line has not asked for it yet: `%IX1.0` (`P1.RemoteStart`) and `%QX0.0` (`P1.Run`) read
    `Bool(false)`. Take one message of the run state from the broker:

    ```text
    mosquitto_sub -h 127.0.0.1 -t trust/examples/multi/running -C 1
    ```

    It prints `false`.

6. Ask for the drive as the supervisory system would:

    ```text
    mosquitto_pub -h 127.0.0.1 -t trust/examples/multi/start -m true
    ```

    Read the image again with `trust-runtime ctl --project . io-read`: `P1.RemoteStart` and
    `P1.Run` now read `Bool(true)`, and `P1.DriveReady` still reads `Bool(true)`. The drive's
    terminal prints `coil 0: True`, and the `mosquitto_sub` command of step 5, which ends after one
    message, now prints `true` when you run it again.

7. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`. The drive's terminal
   prints `coil 0: False` again, and the last message on `trust/examples/multi/running` is `false`:
   the runtime wrote the safe state of `%QX0.0` through both drivers. Stop the drive's stand-in
   with Ctrl+C.

## What happened

**One image, two drivers.** Each 100 ms, before the program runs, every driver of `io.drivers`
reads its inputs into the one input image, in the order of the list. After the program, every
driver gets the whole output image and sends what it maps:

| Variable | Address | Driver | Outside the runtime |
|---|---|---|---|
| `DriveReady` | `%IX0.0` | `modbus-tcp` | discrete input 0 (function 2) |
| `RemoteStart` | `%IX1.0` | `mqtt` | topic `trust/examples/multi/start`, `true` or `false` |
| `Run` | `%QX0.0` | both | coil 0 (function 5) and topic `trust/examples/multi/running` |

**Each driver fills only its own inputs.** A driver in its raw form fills the input image from its
first byte: a Modbus/TCP driver with `input_start` from that input register on, and an MQTT driver
with `topic_in` with its newest message, and zero after its end, at every cycle. Two drivers in
raw form overwrite each other: the one later in the list wins at every cycle, and the inputs of the
other never reach the program. Points and mappings fill only their own bits, so the drive's input
and the line's request cannot cover each other. Give each driver its own inputs in every project
with more than one: when two drivers fill the same bytes, the runtime warns at every start.

**Levels, not changes.** The runtime publishes `Run` at every cycle, ten messages a second here,
not only when it changes, and `RemoteStart` keeps the value of the last message until a new one
comes. A subscriber that comes late has the run state within one cycle; a runtime that starts late
has no remote start until the next message.

**Errors per driver.** Each driver has its own `on_error`. The drive's is `fault`: when the drive
cannot be reached, the resource faults and the program stops, so it never runs on a stale "drive
ready". The broker's is `warn`: the runtime keeps running and `RemoteStart` keeps its last value,
so a lost broker stops neither the line nor the drive. Decide for each driver what a lost link must
do.

**The stop.** At a deliberate stop the runtime writes the safe values of `[[io.safe_state]]` to the
output image and hands them to every driver, so coil 0 went back to FALSE and the run state's last
message was `false`. A safe state that a driver cannot confirm ends the stop in `Faulted`, not
`Stopped`.

## Try it yourself

- Put the single-driver form beside the list: add `[io]` with `driver = "simulated"` at the top of
  `io.toml` and check the configuration again. `trust-runtime validate --project .` refuses it with
  `Error: invalid config 'io.toml: use either io.driver/io.params or io.drivers, not both'`: a file
  takes either `io.driver` with `io.params` or the list `io.drivers`. Take the two lines out again.
- Turn the mappings back into raw topics: in `io.toml`, replace the `mappings = [...]` list with
  `topic_in = "trust/examples/multi/start"` and `topic_out = "trust/examples/multi/running"`, and
  start the runtime again. Its first line is the warning `"event":"io_inputs_overlap"` at
  `"level":"warn"`, with `"earlier":"modbus_tcp:0"`, `"later":"mqtt:1"`, `"first_byte":0` and
  `"last_byte":0`: the raw topic fills the image from byte 0, the drive's point included, so what
  the broker sends overwrites the drive's ready signal. The runtime still runs it. Put the
  mappings back.
- Publish `false` instead of `true` in step 6; `0` and `off` mean the same, as `1` and `on` mean
  `true`. `P1.RemoteStart` and `P1.Run` read `Bool(false)` again, and the drive's terminal prints
  `coil 0: False`.
- Stop the runtime and start it again. `P1.RemoteStart` reads `Bool(false)`: a broker hands a
  message only to the clients that have subscribed when it comes. Publish the request with `-r`,
  `mosquitto_pub -h 127.0.0.1 -t trust/examples/multi/start -m true -r`, and start the runtime
  again: the broker keeps a retained message for clients that subscribe later, so `P1.RemoteStart`
  and `P1.Run` read `Bool(true)` right after the start. Clear it with
  `mosquitto_pub -h 127.0.0.1 -t trust/examples/multi/start -n -r`.
- Stop the broker while the drive runs. `P1.RemoteStart` and `P1.Run` keep `Bool(true)`,
  `trust-runtime ctl --project . status` still answers `state=running fault=none`, and the drive's
  terminal keeps printing `coil 0: True`. Stop the runtime: the drive's terminal prints
  `coil 0: False`, but the MQTT driver cannot confirm the safe state, so the runtime ends with
  `runtime resource faulted: i/o driver error 'safe-state output unconfirmed for driver 'mqtt': ...'`.
  Start the broker again.
- Stop the drive's stand-in with Ctrl+C while the runtime runs. The next exchange fails and the
  resource faults: `trust-runtime ctl --project . status` answers `state=faulted fault=i/o transport
  error 'modbus tcp 127.0.0.1:1502: ...'`, and the program no longer runs, whatever the broker
  sends.

## Reference

- Specification 11, composed I/O drivers and the safe state at a stop; specification 32, MQTT
  mappings.
- The Modbus/TCP and MQTT pages and `io.toml` in the reference of the documentation, and the
  `trust-runtime ctl` commands `io-read`, `status` and `shutdown`.
