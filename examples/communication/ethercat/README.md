# EtherCAT

Map a PLC project onto an EtherCAT module chain with the mock adapter, which needs no hardware, and
watch one bit travel from the input terminal's image to the output terminal's. You learn how
`io.toml` describes the chain, and what changes when a real network adapter takes the mock's place.

## What you learn

- `io.driver = "ethercat"` takes the chain from `[[io.params.modules]]`, in the order of the bus:
  here the coupler `EK1100`, the 8-channel input terminal `EL1008` and the 8-channel output terminal
  `EL2008`, with fixed byte lengths and channel order.
- `adapter = "mock"` runs the same mapping without a bus: `mock_inputs` are the input bytes that the
  chain reports, so a run gives the same image every time, as CI needs.
- A real network adapter takes the mock's place with `adapter = "<adapter name>"`, and needs raw
  socket rights on a unix computer.

## What you need

- `trust-runtime`, from the truST extension for VS Code or a release archive, and two terminals.
- About 10 minutes. For the hardware part of Try it yourself: a unix computer with a network adapter
  for EtherCAT, and the terminals `EK1100`, `EL1008` and `EL2008`.

## Step by step

1. Open a terminal in `examples/communication/ethercat` and build the project:

    ```text
    trust-runtime build --project . --sources src
    ```

    It writes the program's image, here on an x86-64 Linux computer:

    ```text
    Wrote ./program.tpi
    ```

    The program `Main` copies one input to one output, `DO0 := DI0;`, and `src/config.st` puts
    `DI0` at `%IX0.0` and `DO0` at `%QX0.0`.

2. Open `io.toml`. It selects the mock adapter, the input bytes it reports, the module chain and the
   safe state of the output:

    ```toml
    [io]
    driver = "ethercat"

    [io.params]
    adapter = "mock"
    timeout_ms = 250
    cycle_warn_ms = 5
    on_error = "fault"
    mock_inputs = ["01", "00"]

    [[io.params.modules]]
    model = "EK1100"
    slot = 0

    [[io.params.modules]]
    model = "EL1008"
    slot = 1
    channels = 8

    [[io.params.modules]]
    model = "EL2008"
    slot = 2
    channels = 8

    [[io.safe_state]]
    address = "%QX0.0"
    value = "FALSE"
    ```

3. Check the configuration:

    ```text
    trust-runtime validate --project .
    ```

    It answers `Project ok`.

4. Start the runtime in this terminal:

    ```text
    trust-runtime run --project .
    ```

5. In the second terminal, read the process image:

    ```text
    trust-runtime ctl --project . io-read
    ```

    The mock's first input byte, `01`, sets `%IX0.0`, and the program copies it to `%QX0.0`; both
    read TRUE:
    `{"address":"%IX0.0","forced":false,"name":"P1.DI0","value":"Bool(true)","valueType":"BOOL"}`
    and `{"address":"%QX0.0","forced":false,"name":"P1.DO0","value":"Bool(true)","valueType":"BOOL"}`.
    `trust-runtime ctl --project . status` answers
    `state=running fault=none rt_profile=disabled rt_active=false`.

6. Stop the runtime with `trust-runtime ctl --project . shutdown --exit`. Before it ends, it writes
   the safe state, `%QX0.0` FALSE, to the chain, and its log says so with the event
   `runtime_safe_state_applied`.

## What happened

**The chain.** The driver lays out the image from the modules in the order of
`[[io.params.modules]]`, which must be the order of the terminals on the bus. The coupler carries no
process data, the `EL1008`'s channels fill the input image and the `EL2008`'s channels take the
output image. A chain on the bus whose process image differs in size from the declared chain's
always faults the resource, whatever `on_error` says, because running on would read the channels in
another layout.

**The policy.** `timeout_ms = 250` bounds an exchange with the bus, a cycle slower than
`cycle_warn_ms = 5` marks the driver degraded, and `on_error = "fault"` faults the resource when an
exchange fails. Keep it while you commission the bus.

**The stop.** At a deliberate stop the runtime writes the safe values of `[[io.safe_state]]` to the
output image and through the driver. A safe state that the driver cannot confirm ends the stop in
`Faulted`, not `Stopped`.

## Try it yourself

- Set `mock_inputs = ["00", "00"]` and start the runtime again: `%IX0.0` and `%QX0.0` read FALSE.
- On hardware, copy `io.hardware.toml` over `io.toml` and set `adapter` to the name of your
  EtherCAT network adapter, such as `eth0`. Wire the terminals in the declared order, give the
  runtime raw socket rights (`CAP_NET_RAW`, or run it as root), and run steps 3 to 5 again:
  `%IX0.0` follows the `EL1008`'s first channel and drives the `EL2008`'s first channel. A real
  adapter needs a `trust-runtime` built with the `ethercat-wire` feature, as every release is, on
  a unix computer.

## Reference

- Specification 11, the EtherCAT driver (backend v1) and the safe state at a stop.
- `io.toml`'s `ethercat` driver and the `trust-runtime ctl` commands `io-read`, `status` and
  `shutdown`, in the reference of the documentation.
