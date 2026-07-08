# Phase 6 Modbus Function-Code and Point-Map Depth Evidence

Date: 2026-07-05

Scope: Modbus TCP function-code and per-point mapping depth. This evidence
proves `RTCONN-P6-001` for Modbus FC01, FC02, FC03, FC05, FC06, FC15, and
explicit FC16 coverage, and `RTCONN-P6-002` for typed per-point mapping,
scaling, byte order, and word order. It does not close the remaining Phase 6
MQTT, Sparkplug B, or EtherCAT DC/CoE rows.

## Implementation

- `crates/trust-runtime/src/io/modbus.rs`
  - Preserves the existing default profile: FC04 `read_input_registers` for
    `%I` and FC16 `write_multiple_registers` for `%Q`.
  - Adds optional `io.params.input_function`:
    - `read_coils` / `fc01`
    - `read_discrete_inputs` / `fc02`
    - `read_holding_registers` / `fc03`
    - `read_input_registers` / `fc04`
  - Adds optional `io.params.output_function`:
    - `write_single_coil` / `fc05`
    - `write_single_register` / `fc06`
    - `write_multiple_coils` / `fc15`
    - `write_multiple_registers` / `fc16`
  - Keeps these as Modbus driver options; no `IoDriver` trait redesign, ADS/OPC
    UA execution-loop change, or connector-status module placement change was
    introduced.
  - Uses optional `io.params.input_points` and `io.params.output_points` only
    when configured. Existing bulk input/output behavior remains the default
    when point maps are absent.

- `crates/trust-runtime/src/io/modbus/point_map.rs`
  - Isolates point-map config parsing and value conversion from the Modbus TCP
    transport.
  - Supports `bool`, `u16`, `i16`, `u32`, `i32`, and `f32` point types.
  - Applies input scaling as `engineering = raw * scale + offset`.
  - Applies output scaling as `raw = (engineering - offset) / scale`.
  - Treats `byte_order` and `word_order` as Modbus wire/register layout only.
    Numeric process-image values are stored and read little-endian.
  - Rejects coil/register type mismatches and zero scaling with named config
    diagnostics before any Modbus socket is opened.

- `crates/trust-runtime/tests/modbus_driver.rs`
  - Replaced the register-only fixture with a Modbus TCP fixture that records
    function codes and supports coils, discrete inputs, holding registers, input
    registers, single writes, and multiple writes.
  - Proves default FC04/FC16 behavior remains unchanged.
  - Proves explicit FC01, FC02, FC03, FC05, FC06, FC15, and FC16 behavior.
  - Proves invalid function config fails with a named `io.params.input_function`
    diagnostic.
  - Proves point-map reads for scaled registers plus coils and point-map writes
    for coils, multiple registers, and single registers.
  - Proves invalid point-map type/function and scaling config fail before
    runtime polling.

## Docs Updated

- `CHANGELOG.md`
- `docs/guides/PLC_IO_BINDING_GUIDE.md`
- `docs/specs/11-runtime-engine.md`
- `docs/public/connect/external-systems/modbus-tcp.md`
- `docs/public/connect/protocol-matrix.md`

Local internal note: `docs/internal/runtime/trust-runtime-cli-specification.md`
was also updated in this checkout, but that path is ignored by `.gitignore`
and is not used as the tracked proof for this slice.

## Local Checks

```sh
cargo fmt --all -- --check
git diff --check
```

Result: both passed locally after the enum-variant warning cleanup.

Local checks were rerun after adding point maps and the clippy warning cleanup:

```sh
cargo fmt --all -- --check
git diff --check
```

Result: both passed.

## Remote Builder Proof

Remote copy: `trust-builder:~/projects/trust-platform-rtconn-validation`

Target dir:
`trust-builder:~/.cache/codex-targets/trust-platform-rtconn-validation`

Disk preflight before the initial function-code gates:

```text
/home/johannes: 111G free
/tmp: 6.7G free
```

Disk preflight before the point-map rerun:

```text
/home/johannes: 97G free
/tmp: 6.7G free
```

Focused Modbus test:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  cargo test -p trust-runtime --test modbus_driver'
```

Initial function-code result: passed with 5 tests, 2 ignored.

Point-map rerun result: passed with 8 tests, 2 ignored.

Focused process-image composition test:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  cargo test -p trust-runtime --test io_multidriver_live'
```

Result: passed with 2 tests.

Common staged gate:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  just fmt && just clippy && just test'
```

Initial function-code result:

- `just fmt`: passed.
- `just clippy`: passed without warnings after renaming the new enum variants.
- `just test`: passed through the fallback `cargo test -p trust-runtime --lib`
  with 831 passed and 16 ignored.

Point-map rerun result:

- `just fmt`: passed.
- `just clippy`: passed without warnings after replacing manual even-length
  checks with `is_multiple_of`.
- `just test`: passed through the fallback `cargo test -p trust-runtime --lib`
  with 831 passed and 16 ignored.

Networking gates:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8'
```

Result: passed all 8 mesh TLS publish regression iterations after the
function-code slice and again after the point-map slice.

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets'
```

Initial function-code result: passed in 2m39s.

Point-map rerun result: passed in 20.19s.

Disk after gates:

```text
/home/johannes: 97G free
/tmp: 6.7G free
```
