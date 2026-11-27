# Tutorial 17: I/O Backends and Composed Drivers

Run one project on the two I/O drivers that need no hardware, loopback and simulated, and check its
`io.toml` for GPIO, Modbus/TCP, MQTT and two drivers composed in one runtime. You learn what each
driver does to the process image, how to check an `io.toml` before you start the runtime, and why a
file takes one driver or a list. It starts from tutorial 13's project.

## What you learn

- `loopback` copies the output image into the input image at every cycle, and `simulated` keeps each
  input as you set it: two drivers that need no hardware.
- `gpio`, `modbus-tcp` and `mqtt` connect the image to pins, to a Modbus/TCP device and to an MQTT
  broker; `trust-runtime validate` checks their files without reaching the device.
- `io.drivers` composes several drivers, each filling only its own inputs, and a file takes either
  `io.driver` with `io.params` or the list `io.drivers`.

## What you need

- Tutorial 13's project in `/tmp/trust-tutorial-13`, as its README builds it, and
  `trust-runtime`, from the truST extension for VS Code or a release archive.
- Linux or macOS, whose shell runs the commands below, and two terminals.
- About 20 minutes. No device is needed: the GPIO, Modbus/TCP and MQTT files are checked here, and
  the communication examples run them against a device.

## Step by step

1. Copy tutorial 13's project and build it once:

    ```bash
    rm -rf /tmp/trust-io-backends
    cp -R /tmp/trust-tutorial-13 /tmp/trust-io-backends
    cd /tmp/trust-io-backends
    trust-runtime build --project . --sources src
    ```

    The program copies `StartCmd` at `%IX0.0` to `LampOut` at `%QX0.0`, and the build writes its
    image, here on an x86-64 Linux computer:

    ```text
    Wrote ./program.tpi
    Wrote ./build/install.x86_64-unknown-linux-gnu.bin for trust-runtime ctl install
    Debug image 2012 bytes: code 681, read-only data 0, starting memory 2
    ```

2. Run it on `loopback`, which copies the output image into the input image at every cycle. Write
   `io.toml` and check it:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "loopback"
    params = {}

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML

    trust-runtime validate --project .
    ```

    It answers `Project ok`. Start the runtime in the second terminal with
    `trust-runtime run --project .`, then, in the first, force the output on and read the image:

    ```bash
    trust-runtime ctl --project . io-force %QX0.0 true
    trust-runtime ctl --project . io-read
    ```

    In the answer, `%QX0.0` (`P1.LampOut`) has `"forced":true` and `"value":"Bool(true)"`, and
    `%IX0.0` (`P1.StartCmd`) reads `"value":"Bool(true)"` too: the loopback copied the output into
    the input. Release the force with `trust-runtime ctl --project . io-unforce %QX0.0`, and both
    still read `Bool(true)`: the program copies `%IX0.0` to `%QX0.0` and the loopback copies
    `%QX0.0` back to `%IX0.0`, so the lamp holds itself on. Writing the input breaks the loop:
    after `trust-runtime ctl --project . io-write %IX0.0 false`, the program puts the lamp out, the
    loopback copies the `FALSE` back, and both read `Bool(false)`. Stop the runtime with
    `trust-runtime ctl --project . shutdown --exit`.

3. Run it on `simulated`, which leaves each input as you set it:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "simulated"
    params = {}

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML

    trust-runtime validate --project .
    ```

    Start the runtime again and force the output on as in step 2: `%QX0.0` reads `Bool(true)` and
    `%IX0.0` stays `Bool(false)`, as nothing copies the output back. Release the force, and write the
    input instead: after `trust-runtime ctl --project . io-write %IX0.0 true`, `%IX0.0` and `%QX0.0`
    read `Bool(true)`, and they stay so until you write `false`. Stop the runtime.

4. Check a GPIO file, which binds `%IX0.0` to line 17 and `%QX0.0` to line 27:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "gpio"

    [io.params]
    backend = "sysfs"
    inputs = [
      { address = "%IX0.0", line = 17, debounce_ms = 5 }
    ]
    outputs = [
      { address = "%QX0.0", line = 27, initial = false }
    ]

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML

    trust-runtime validate --project .
    ```

    It answers `Project ok`. Running it needs those lines and the permission to use them, as
    `examples/communication/gpio` describes.

5. Check a Modbus/TCP file for a device at `192.168.0.10:502`:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "modbus-tcp"

    [io.params]
    address = "192.168.0.10:502"
    unit_id = 1
    input_start = 0
    output_start = 0
    timeout_ms = 500
    on_error = "fault"

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML

    trust-runtime validate --project .
    ```

    It answers `Project ok` without reaching the device. Started without one, the runtime faults:
    `trust-runtime ctl --project . status` answers
    `state=faulted fault=i/o transport error 'modbus input refresh pending'`, since
    `on_error = "fault"` stops the program when an exchange does not complete in time.
    `examples/communication/modbus_tcp` runs it against a server.

6. Check an MQTT file for a broker at `127.0.0.1:1883`:

    ```bash
    cat > io.toml <<'TOML'
    [io]
    driver = "mqtt"

    [io.params]
    broker = "127.0.0.1:1883"
    topic_in = "line/in"
    topic_out = "line/out"
    reconnect_ms = 500
    keep_alive_s = 5
    allow_insecure_remote = false

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML

    trust-runtime validate --project .
    ```

    It answers `Project ok`. `examples/communication/mqtt` runs it against a broker.

7. Compose two drivers: the Modbus/TCP device takes `%IX0.0` from its discrete input 0 and gets
   `%QX0.0` on its coil 0, and the MQTT broker gets the lamp by name:

    ```bash
    cat > io.toml <<'TOML'
    [[io.drivers]]
    name = "modbus-tcp"

    [io.drivers.params]
    address = "192.168.0.10:502"
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

    [[io.drivers]]
    name = "mqtt"

    [io.drivers.params]
    broker = "127.0.0.1:1883"
    reconnect_ms = 500
    keep_alive_s = 5
    mappings = [
      { tag = "P1.LampOut", topic = "line/lamp", direction = "write" },
    ]

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    TOML

    trust-runtime validate --project .
    ```

    It answers `Project ok`, having found `P1.LampOut` in the built program.
    `examples/communication/multi_driver` runs such a composition against a device and a broker.

## What happened

**Two drivers without hardware.** `loopback` makes the output image the next cycle's input image, so
an output reaches the input of the same address: the forced lamp reached `%IX0.0`, and the program
then kept it on. It shows a program reacting to its own outputs. `simulated` writes no input, so each
input keeps the value that `io-write` gave it, whatever the outputs do: the driver tutorial 13 uses
to try a program by hand.

**Checking before starting.** `validate` checks each driver's parameters without opening a device, a
socket or a broker, so it passes the Modbus/TCP and MQTT files on a computer that reaches neither. It
does read the built program, to find the variables that MQTT mappings name. Only a run shows whether
the device answers.

**The errors.** Each driver has its own `on_error`. `fault` stops the program as soon as an exchange
does not complete in time, so that the program never runs on inputs it did not receive. `warn` keeps
it running on the inputs it last had, as the last Try-it shows, which suits only inputs whose loss is
safe.

**Composed drivers.** Each driver of `io.drivers` reads its inputs into the one input image before the
cycle, in the order of the list, and gets the whole output image after it. A driver in its raw form, a
Modbus/TCP block from `input_start` or an MQTT `topic_in`, fills the image from its first byte, so
two such drivers would overwrite each other's inputs; points and mappings fill only their own, as
here. At a stop the safe state reaches every driver.

## Try it yourself

- Put both forms in one file: add `[io]` with `driver = "simulated"` at the top of step 7's
  `io.toml`. `trust-runtime validate --project .` refuses it with
  `Error: invalid config 'io.toml: use either io.driver/io.params or io.drivers, not both'`.
- On `loopback`, change the program to `LampOut := NOT StartCmd;`, build and start it. The output
  feeds its own inverse back, so `%IX0.0` and `%QX0.0` swap at every cycle: successive `io-read`s
  show `%IX0.0` `Bool(false)` with `%QX0.0` `Bool(true)` at one scan and the other way round at the
  next, as their `scan` numbers tell. Put `LampOut := StartCmd;` back and build again.
- Set `on_error = "warn"` in step 5's file and start the runtime without the device: it keeps running,
  `state=running fault=none`, with `%IX0.0` at `Bool(false)`, as no input came. Its stop then ends with
  `safe-state output unconfirmed for driver 'modbus-tcp'`, since nothing confirms the safe state.

## Reference

- Specification 11, I/O drivers and their composition (§6.5), the Modbus/TCP deadlines and the safe
  state at a stop; specification 32, MQTT mappings.
- The I/O binding and multi-driver pages of the documentation, and the communication examples
  `modbus_tcp`, `mqtt`, `gpio` and `multi_driver`.
