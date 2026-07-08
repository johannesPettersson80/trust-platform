# Modbus TCP

## Good fit vs bad fit

| Good fit | Bad fit |
| --- | --- |
| PLC-to-device register exchange | pub/sub event distribution |
| gateways, drives, legacy equipment | highly dynamic topic-style systems |
| explicit polling and deterministic offsets | loosely structured payloads |

## First things to decide

- what server endpoint will the runtime talk to?
- what `unit_id` is correct for the device or gateway?
- where do inputs and outputs start in coil/register space?
- which Modbus functions should be used for the process image?
- should communication faults halt, warn, or degrade gracefully?

Success means the device endpoint, `unit_id`, function-code profile, coil or
register map, byte order, and fault behavior are written down before runtime
validation starts.

The default profile is conservative and backward compatible: FC04
`read_input_registers` for `%I` and FC16 `write_multiple_registers` for `%Q`.
When a device map requires a different shape, set `input_function` explicitly to
`read_coils` (FC01), `read_discrete_inputs` (FC02),
`read_holding_registers` (FC03), or `read_input_registers` (FC04), and set
`output_function` explicitly to `write_single_coil` (FC05),
`write_single_register` (FC06), `write_multiple_coils` (FC15), or
`write_multiple_registers` (FC16). Coil payloads use Modbus bit packing; register
payloads remain big-endian bytes.

Use `input_points` and `output_points` when the process image needs explicit
typed points instead of one contiguous raw block. Each point names the
process-image byte offset, optional bit, Modbus address, function, data type
(`bool`, `u16`, `i16`, `u32`, `i32`, or `f32`), optional `scale`/`offset`, and
optional Modbus `byte_order`/`word_order`. Scaling converts inputs as
`engineering = raw * scale + offset`; output writes invert that formula before
writing Modbus. Numeric point-map values are stored in the runtime process image
as little-endian bytes after scaling.

At runtime, Modbus TCP exchange is worker-backed. Scan-cycle reads copy the
latest worker snapshot or return the configured `on_error` policy result, and
scan-cycle writes hand off the latest desired output without waiting for a TCP
round trip. The Modbus worker owns connect/read/write latency, reconnect
backoff, and stale/degraded health reporting through the normal driver status
surface.

## Example and commissioning guide

--8<-- "examples/communication/modbus_tcp/README.md:3"

## Common Modbus gotchas

- wrong `unit_id` behind a gateway
- off-by-one mental model around coil/register blocks
- selecting a coil function for a register map, or a register function for a
  coil map
- applying Modbus byte/word order to the runtime process image instead of the
  wire/register layout
- byte/word order mismatches on non-trivial payloads
- accepting a “validate passed” result as proof of runtime connectivity

## Related

- [I/O binding](../devices-and-fieldbus/io-binding.md)
- [Protocol Matrix](../protocol-matrix.md)
