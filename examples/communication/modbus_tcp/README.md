# Modbus/TCP

Connect a PLC project's inputs and outputs to a Modbus/TCP server and watch one bit travel through
it. You learn how `io.toml` maps the process image onto the server's registers, and what the
runtime writes when it stops.

## What you learn

- `io.driver = "modbus-tcp"` reads the input image from the server's input registers (function 4)
  and writes the output image to its holding registers (function 16), from the register offsets
  `input_start` and `output_start`.
- Registers carry the image high byte first, so `%IX0.0` is bit 0 of the high byte of input
  register 0: the register value 256 (`0x0100`).
- `[[io.safe_state]]` names the values the runtime writes to its outputs when it stops.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, and two terminals.
- A Modbus/TCP server on `127.0.0.1:1502` whose input register 0 holds 256, for example the
  simulator diagslave from modbusdriver.com (`diagslave -m tcp -p 1502`), with the register set in
  its tool or by a client of your choice.
- About 15 minutes.

## Step by step

1. Open a terminal in `examples/communication/modbus_tcp` and build the project:

    ```text
    trust-runtime build --project . --sources src
    ```

    It writes the program's image, here on an x86-64 Linux computer:

    ```text
    Wrote ./program.tpi
    ```

    The program `Main` copies one input to one output, `DO0 := DI0;`, and `src/config.st` puts
    `DI0` at `%IX0.0` and `DO0` at `%QX0.0`.

2. Open `io.toml`. It selects the driver and its server, and the safe state of the output:

    ```toml
    [io]
    driver = "modbus-tcp"

    [io.params]
    address = "127.0.0.1:1502"
    unit_id = 1
    input_start = 0
    output_start = 0
    timeout_ms = 500
    on_error = "fault"

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    ```

3. Check the configuration before you start anything:

    ```text
    trust-runtime validate --project .
    ```

    It answers `Project ok`.

4. Start your Modbus/TCP server with input register 0 at 256, then start the runtime in the first
   terminal:

    ```text
    trust-runtime run --project .
    ```

5. In the second terminal, read the process image:

    ```text
    trust-runtime ctl --project . io-read
    ```

    Its answer holds each address with its variable and value. The input reads TRUE,
    `{"address":"%IX0.0","forced":false,"name":"P1.DI0","value":"Bool(true)","valueType":"BOOL"}`,
    and so does the output `%QX0.0` (`P1.DO0`) that the program copied from it. The server sees
    holding register 0 written as 256.

6. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`. The server sees holding
   register 0 written as 0: the safe state of `%QX0.0`.

## What happened

**The mapping.** Each 100 ms the runtime reads the server's input registers from offset
`input_start` into the input image, runs the program, and writes the output image to the holding
registers from offset `output_start`. A register holds two bytes of the image, high byte first, so:

| Image | Register | Bit |
|---|---|---|
| `%IX0.0` to `%IX0.7` | input register 0, high byte | 256 for `%IX0.0` (`0x0100`) |
| `%IX1.0` to `%IX1.7` | input register 0, low byte | 1 for `%IX1.0` |
| `%QX0.0` | holding register 0, high byte | 256 when TRUE |

**The errors.** `timeout_ms = 500` bounds each exchange, and `on_error = "fault"` faults the
resource when an exchange fails, so that a lost server stops the program instead of leaving stale
inputs in the image. Keep it while you commission the connection.

**The stop.** At a deliberate stop the runtime writes the safe values of `[[io.safe_state]]` to the
output image and sends them through the driver, so the server's holding register 0 went back to 0.
A safe state that the driver cannot confirm ends the stop in `Faulted`, not `Stopped`.

## Try it yourself

- Set input register 0 to 1 instead of 256. `%IX0.0` reads FALSE, since 1 is bit 0 of the low byte:
  it is `%IX1.0`, which the program does not use.
- Set `input_start = 1` in `io.toml` and start the runtime again; the runtime reads `io.toml` when
  it starts, so no build is needed. It now reads from input register 1, and `%IX0.0` follows that
  register instead.
- Stop your server while the runtime runs. The next exchange fails and the resource faults:
  `trust-runtime ctl --project . status` answers `state=faulted fault=i/o transport error 'modbus
  tcp 127.0.0.1:1502: the server closed the connection'`.

## Reference

- Specification 11, the Modbus/TCP driver and the safe state at a stop.
- `io.toml` and the `trust-runtime ctl` commands `io-read` and `shutdown`, in the reference of the
  documentation.
