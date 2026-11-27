# MQTT

Connect a PLC project's inputs and outputs to an MQTT broker and watch one bit travel through it.
You learn how the MQTT driver's raw form carries the process image in messages, how long an input
keeps a message's value, and what the runtime publishes when it stops.

## What you learn

- `io.driver = "mqtt"` with `topic_in` and `topic_out` is a raw byte bridge: the newest message on
  `topic_in` fills the input image from its first byte, and the runtime publishes its output image
  to `topic_out` at every cycle.
- While the broker is connected, an input keeps the value of the last message until a new one
  comes, unless `stale_after_ms` limits its age.
- `[[io.safe_state]]` names the values the runtime publishes when it stops.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, and two terminals.
- An MQTT broker on `127.0.0.1:1883` with its command-line clients: Mosquitto (`mosquitto`,
  `mosquitto_pub` and `mosquitto_sub`) from your system's packages, for example. Start the broker
  with `mosquitto` if your system did not start it as a service when you installed it.
- About 15 minutes.

## Step by step

1. Open a terminal in `examples/communication/mqtt` and build the project:

    ```text
    trust-runtime build --project . --sources src
    ```

    It writes the program's image, here on an x86-64 Linux computer, and then names the image's
    memory figure, the key that signed it and its two sources, `src/config.st` and `src/main.st`:

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 1972 bytes: code 681, read-only data 0, starting memory 2
    ```

    The program `Main` copies one input to one output, `DO0 := DI0;`, and `src/config.st` puts
    `DI0` at `%IX0.0` and `DO0` at `%QX0.0`.

2. Open `io.toml`. It selects the driver, its broker and its two topics, and the safe state of the
   output:

    ```toml
    [io]
    driver = "mqtt"

    [io.params]
    broker = "127.0.0.1:1883"
    topic_in = "trust/examples/mqtt/in"
    topic_out = "trust/examples/mqtt/out"
    reconnect_ms = 500
    keep_alive_s = 5
    allow_insecure_remote = false
    on_error = "warn"

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    ```

3. Check the configuration before you start anything:

    ```text
    trust-runtime validate --project .
    ```

    It answers `Project ok`.

4. Make sure the broker runs, then start the runtime in the first terminal:

    ```text
    trust-runtime run --project .
    ```

5. In the second terminal, take one message of the output image from the broker, as two hex
   digits a byte:

    ```text
    mosquitto_sub -h 127.0.0.1 -t trust/examples/mqtt/out -F '%x' -C 1
    ```

    It prints `00`: the output image is one byte, and `%QX0.0` is its bit 0.

6. Publish one input image byte, 1, to `topic_in`:

    ```text
    printf '\001' | mosquitto_pub -h 127.0.0.1 -t trust/examples/mqtt/in -s
    ```

    Then read the process image:

    ```text
    trust-runtime ctl --project . io-read
    ```

    The input reads TRUE,
    `{"address":"%IX0.0","forced":false,"name":"P1.DI0","value":"Bool(true)","valueType":"BOOL"}`,
    and so does the output `%QX0.0` (`P1.DO0`) that the program copied from it. The `mosquitto_sub`
    command of step 5, which ends after one message, now prints `01` when you run it again, and a
    few seconds later `io-read` still reads TRUE: no new message came.

7. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`. The last message on
   `trust/examples/mqtt/out` is `00`: the runtime published the safe state of `%QX0.0`.

## What happened

**The bridge.** Each 100 ms, before the program runs, the driver takes the newest message that came
on `topic_in` since the last cycle into the input image, from its first byte, and zero after the
message's end. After the program it publishes the whole output image to `topic_out`, at every
cycle, ten messages a second here, not only when a value changes. Bit 0 of the first byte is
`%IX0.0`, bit 1 is `%IX0.1`, and so on:

| Image | Message | Here |
|---|---|---|
| `%IX0.0` to `%IX0.7` | byte 0 on `topic_in` | 1 sets `DI0` |
| `%IX1.0` to `%IX1.7` | byte 1 on `topic_in` | not used |
| `%QX0.0` to `%QX0.7` | byte 0 on `topic_out` | `01` while `DO0` is TRUE |

**The last message counts.** A cycle that finds no new message keeps the input image of the last
one, under every `on_error`, because a publisher commonly sends a value when it changes, not at
every cycle. `stale_after_ms` makes an input stale after that long without a message, which suits a
publisher that sends cyclically.

**The errors.** `on_error = "warn"` keeps the runtime running while the broker cannot be reached,
with the inputs of the last message. The default, `fault`, faults the resource instead, so that the
program stops rather than run on inputs it no longer receives.

**The stop.** At a deliberate stop the runtime writes the safe values of `[[io.safe_state]]` to the
output image and publishes them, so the last message on `topic_out` was `00`. A safe state that
the driver cannot confirm ends the stop in `Faulted`, not `Stopped`.

The raw form carries whole bytes of the image. For values that a broker carries one to a topic, as
text or JSON, use typed `input_points` and `output_points`, or `mappings` that name program
variables, as `../multi_driver` and `../mqtt_traffic_light` do.

## Try it yourself

- Publish the byte 2, `printf '\002' | mosquitto_pub -h 127.0.0.1 -t trust/examples/mqtt/in -s`.
  `%IX0.0` and `%QX0.0` read `Bool(false)`, and `topic_out` carries `00`: 2 is bit 1, `%IX0.1`,
  which the program does not read.
- Set `on_error = "fault"` and add `stale_after_ms = 2000` under `[io.params]`, start the runtime
  again and publish the byte 1 once. Two seconds later the input is stale:
  `trust-runtime ctl --project . status` answers
  `state=faulted fault=i/o freshness error 'mqtt input not fresh'`. Put the two lines back.
- Stop the broker while the runtime runs, after a message set `%IX0.0`. `%IX0.0` and `%QX0.0` keep
  `Bool(true)` and `trust-runtime ctl --project . status` still answers `state=running fault=none`,
  as `warn` says. Stop the runtime: it cannot publish the safe state, so it ends with
  `runtime resource faulted: i/o driver error 'safe-state output unconfirmed for driver 'mqtt': ...'`.
  Start the broker again.

## Reference

- Specification 32, the MQTT driver: its raw form, typed points and mappings, sessions and
  freshness; specification 11, the safe state at a stop.
- The MQTT page and `io.toml` in the reference of the documentation, with typed points, mappings
  and Sparkplug B, and the `trust-runtime ctl` commands `io-read`, `status` and `shutdown`.
