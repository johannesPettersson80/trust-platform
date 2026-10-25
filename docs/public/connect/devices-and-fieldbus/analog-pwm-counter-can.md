# Analog, PWM, Counters And CAN

The `analog`, `pwm`, `counter` and `can` drivers read and drive the rest of a
board's I/O with the same `io.toml` keys on every runtime. A `line` names the
runtime's own hardware: on a chip a pin or a peripheral (`GPIO2`, `TWAI0`), on
Linux a kernel device and channel. macOS and Windows have no such hardware and
refuse these drivers when the runtime starts, with the reason.

## What to verify first

- every `line` names hardware this runtime has: a pin of the chip, or a Linux
  device that exists (`/sys/bus/iio/devices`, `/sys/class/pwm`,
  `/sys/bus/counter/devices`, `ip link` for CAN)
- every address is a flat address of the size the driver uses, and the entries
  of one list do not share bytes
- every output that drives something has a `[[io.safe_state]]` entry
- on Linux, the runtime may write the device's sysfs files and open the CAN
  interface

## The drivers

| Driver | Entry | Address | Value |
| --- | --- | --- | --- |
| `analog` | `inputs` | `%IW` word | millivolts at the line, as an `INT` |
| `analog` | `outputs` | `%QW` word | millivolts to drive, as an `INT`, from `initial` |
| `pwm` | `outputs` | `%QW` word | duty in hundredths of a percent, 0 to 10,000, at `frequency_hz` |
| `counter` | `inputs` | `%ID` double word | edges as a `UDINT`, or encoder steps as a `DINT` |
| `can` | `receive` | `%IB` byte | the frame's data bytes, `length` of them |
| `can` | `transmit` | `%QB` byte | the bytes sent every `period_ms` |

A misspelt key, a second use of a line, or two entries of one list that share a
byte are refused before the runtime starts, with the key, for example
`pwm: outputs[1].frequency_hz: must be an integer from 1 to 1000000, not 0`.

## On an ESP32-C6

```toml
[[io.drivers]]
name = "analog"
params = { inputs = [{ address = "%IW0", line = "GPIO2" }] }

[[io.drivers]]
name = "pwm"
params = { outputs = [{ address = "%QW0", line = "GPIO4", frequency_hz = 1000 }] }

[[io.drivers]]
name = "counter"
params = { inputs = [
  { address = "%ID4", line = "GPIO5", edge = "rising" },
  { address = "%ID8", line = "GPIO6", line_b = "GPIO7", mode = "quadrature" },
] }

[[io.drivers]]
name = "can"
[io.drivers.params]
line = "TWAI0"
tx = "GPIO10"
rx = "GPIO11"
bitrate = 500000
receive = [{ id = 0x181, address = "%IB16", length = 8, timeout_ms = 100 }]
transmit = [{ id = 0x201, address = "%QB16", length = 2, period_ms = 50 }]
```

- `analog` inputs use ADC1, whose channels are `GPIO0` to `GPIO6`, with the
  chip's calibration; the C6 has no converter for analog outputs.
- `pwm` has six outputs and four frequencies; outputs of one frequency share a
  timer.
- `counter` has four counters; a quadrature counter takes the encoder's B on
  `line_b`.
- `can` runs on `TWAI0` and `TWAI1`, with `tx` and `rx` on any free pins, wired
  to a CAN transceiver.

## On Linux

```toml
[[io.drivers]]
name = "analog"
params = { inputs = [{ address = "%IW0", line = "iio:device0/in_voltage1" }] }

[[io.drivers]]
name = "pwm"
params = { outputs = [{ address = "%QW0", line = "pwmchip0/pwm1", frequency_hz = 1000 }] }

[[io.drivers]]
name = "counter"
params = { inputs = [{ address = "%ID4", line = "counter0/count0", mode = "quadrature" }] }

[[io.drivers]]
name = "can"
params = { line = "can0", bitrate = 500000, receive = [{ id = 0x181, address = "%IB16", length = 8 }] }
```

- `analog` reads the IIO channel's `_raw`, `_offset` and `_scale` files.
- `pwm` exports the channel and sets its `period`, `duty_cycle` and `enable`.
- `counter` sets the count's `function`; a quadrature count's own signals are
  the encoder's A and B, so `line_b` is refused.
- `can` uses the SocketCAN interface at the rate `ip link` gave it; set
  `restart-ms` so that it rejoins after bus-off:
  `ip link set can0 type can bitrate 500000 restart-ms 100`.

## Errors

`on_error` (`fault`, the default, `warn` or `ignore`) decides what a CAN
receive that times out or a bus-off does, as for Modbus. A failed reading or
write of the other drivers faults the driver until the next good one, as for
GPIO. `trust-runtime ctl status` shows each driver's health.

## Related

- [Driver Matrix](driver-matrix.md)
- [GPIO](gpio.md)
- [I/O Binding](io-binding.md)
- Specification 11 §6.5, "Analog, PWM, counter and CAN drivers"
