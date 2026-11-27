# MQTT Traffic-Light Tag Mapping

Publish a traffic light's three lamps to an MQTT broker by naming the program's variables, with no
I/O address in the project. You learn how `mappings` connect topics to variables, which way each
one goes, and when the runtime checks them.

## What you learn

- A mapping in the MQTT driver's `io.toml` names a variable of a program instance, such as
  `MainInstance.Red`, a topic and a direction: `write` publishes the variable's value, and `read`
  sets the variable from the topic's messages.
- A mapped variable needs no address: the runtime finds it in the program when it starts, and
  publishes its value as text, `true` or `false` for a BOOL, at every cycle.
- The runtime checks the mappings when it starts, and a mapped output gets a safe state through an
  address.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, and two terminals.
- An MQTT broker on `127.0.0.1:1883` with its command-line clients: Mosquitto (`mosquitto`,
  `mosquitto_pub` and `mosquitto_sub`) from your system's packages, for example. Start the broker
  with `mosquitto` if your system did not start it as a service when you installed it.
- About 15 minutes.

## Step by step

1. Open a terminal in `examples/communication/mqtt_traffic_light` and build the project:

    ```text
    trust-runtime build --project . --sources src
    ```

    It writes the program's image, here on an x86-64 Linux computer, and then names the image's
    memory figure, the key that signed it and its two sources, `src/config.st` and `src/main.st`:

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 5412 bytes: code 2470, read-only data 32, starting memory 80
    ```

    The program `TrafficLight` steps through four phases: red, red and yellow, green, yellow, and
    red again. A timer of 500 ms ends each phase. `src/config.st` runs the program every 100 ms as
    the instance `MainInstance`, and locates none of its variables.

2. Open `io.toml`. It selects the driver and its broker, and maps each lamp to a topic:

    ```toml
    [io]
    driver = "mqtt"

    [io.params]
    broker = "127.0.0.1:1883"
    reconnect_ms = 500
    keep_alive_s = 5
    allow_insecure_remote = false

    [[io.params.mappings]]
    tag = "MainInstance.Green"
    topic = "traffic/north/green"
    direction = "write"

    [[io.params.mappings]]
    tag = "MainInstance.Yellow"
    topic = "traffic/north/yellow"
    direction = "write"

    [[io.params.mappings]]
    tag = "MainInstance.Red"
    topic = "traffic/north/red"
    direction = "write"
    ```

    Each mapping names a variable of `MainInstance`, a topic, and the direction `write`: the
    runtime publishes the lamp to the topic.

3. Check the configuration before you start anything:

    ```text
    trust-runtime validate --project .
    ```

    It answers `Project ok`.

4. In the second terminal, subscribe to the three topics:

    ```text
    mosquitto_sub -h 127.0.0.1 -v -t 'traffic/north/#'
    ```

    Then start the runtime in the first terminal:

    ```text
    trust-runtime run --project .
    ```

5. The subscriber prints three lines at every cycle, one a lamp, in the order of the mappings. The
   light starts red:

    ```text
    traffic/north/green false
    traffic/north/yellow false
    traffic/north/red true
    ```

    After 7 or 8 cycles, about 0.75 seconds, yellow joins red, then green shows alone, then yellow
    alone, and then red again.

6. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`, and the subscriber with
   Ctrl+C.

## What happened

**The mapping.** When the runtime starts, it looks each tag up among the variables of the program
instances: `MainInstance.Red` is the variable `Red` of the instance `MainInstance` that
`src/config.st` declares. Then, at every cycle, it publishes the value of each `write` mapping to
its topic as text. A tag that names no variable stops the start.

**No addresses.** The lamps are not located, so `trust-runtime ctl --project . io-read` lists no
inputs and no outputs: a mapping reaches the variable itself, wherever the runtime keeps it.

**The direction.** The direction is the PLC's: `write` sends the PLC's value to the broker, and
`read` subscribes to the topic and sets the variable at each new message. With only `write`
mappings the runtime subscribes to nothing, not even `trust/io/in`, the input topic of the raw
form that `../mqtt` uses.

**The timing.** A phase lasts the timer's 500 ms and the two cycles in which the program turns the
timer off and on again, so 7 cycles of 100 ms, or 8 when the timer reaches 500 ms one cycle late.

**The stop.** The project has no `[[io.safe_state]]`, so the stop publishes nothing more, and each
topic's last message is the value of the last cycle. A safe state names an address, which a
mapped lamp does not have until you give it one (Try it yourself).

## Try it yourself

- Publish the phase number too: add this mapping at the end of `io.toml` and start the runtime
  again; the runtime reads `io.toml` when it starts, so no build is needed.
  `mosquitto_sub -h 127.0.0.1 -v -t traffic/north/phase` prints `traffic/north/phase 0`, then 1,
  2, 3 and 0 again, about seven times each: an INT is published as its decimal text.

    ```toml
    [[io.params.mappings]]
    tag = "MainInstance.Phase"
    topic = "traffic/north/phase"
    direction = "write"
    ```

- Name a variable that the program does not declare: change `MainInstance.Red` to
  `MainInstance.Blue`. `trust-runtime validate --project .` still answers `Project ok`, but
  `trust-runtime run --project .` refuses to start with
  `Error: invalid config 'io.params.mappings[2].tag: 'MainInstance.Blue' names no variable of a program instance'`.
  Change it back.
- Give the red lamp a safe state: locate it with the lines below after `END_RESOURCE` in
  `src/config.st`, add `[[io.safe_state]]` with `address = "%QX0.0"` and `value = "FALSE"` at the
  end of `io.toml`, build again and start the runtime. Stop it while the light is red: the last
  message on `traffic/north/red` is `false`, because the mapping uses the variable's address once
  it has one, and the stop writes the safe state there.

    ```st
    VAR_CONFIG
        MainInstance.Red AT %QX0.0 : BOOL;
    END_VAR
    ```

- Make each phase 2 seconds long: in `src/main.st`, set `PhasePeriod : TIME := T#2s;`, build again
  and start the runtime. Each phase now lasts 22 or 23 cycles.

## Reference

- Specification 32, the MQTT driver's mappings (§4 and §5) and its scalar types (§6); specification
  11, the safe state at a stop.
- The MQTT page and `io.toml` in the reference of the documentation, and the `trust-runtime ctl`
  commands `io-read` and `shutdown`.
