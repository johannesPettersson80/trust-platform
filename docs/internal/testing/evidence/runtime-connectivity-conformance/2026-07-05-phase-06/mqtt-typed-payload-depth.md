# Phase 6 MQTT Typed Payload Depth Evidence

Date: 2026-07-05

Scope: MQTT typed scalar payload mapping only. This evidence proves
`RTCONN-P6-003`. It does not close Sparkplug B compatibility (`RTCONN-P6-004`)
or EtherCAT DC/CoE roadmap decisions.

## Implementation

- `crates/trust-runtime/src/io/mqtt/point_map.rs`
  - Adds optional typed point maps for MQTT `input_points` and `output_points`.
  - Supports `bool`, `u16`, `i16`, `u32`, `i32`, and `f32` values.
  - Supports `text`, `json`, `binary_le`, and `binary_be` payload formats.
  - Applies input scaling as `engineering = raw * scale + offset`.
  - Applies output scaling as `raw = (engineering - offset) / scale`.
  - Keeps numeric process-image bytes little-endian; binary payload endianness
    applies only to MQTT payload bytes.

- `crates/trust-runtime/src/io/mqtt/config.rs`
  - Parses optional `input_points` and `output_points`.
  - Preserves the existing raw `topic_in`/`topic_out` byte bridge when point
    maps are absent.
  - Subscribes to the legacy `topic_in` in raw mode, or the unique mapped input
    point topics in typed mode.

- `crates/trust-runtime/src/io/mqtt/session.rs`
  - Replaces the single raw latest payload with a bounded queue of
    topic-bearing MQTT inbound payloads.
  - Keeps the queue bounded at 256 messages so broker bursts cannot grow memory
    without limit.

- `crates/trust-runtime/src/io/mqtt/driver.rs`
  - Keeps the raw byte read/write path unchanged when no typed points are
    configured.
  - Applies typed input messages by topic to the process image.
  - Publishes typed output points to their configured topics.

- `crates/trust-runtime/src/io/mqtt/tests.rs`
  - Proves raw payload read/write compatibility.
  - Proves typed point-map reads for JSON bool, text numeric with scaling, and
    big-endian binary numeric payloads.
  - Proves typed point-map writes for text bool, JSON numeric with scaling, and
    big-endian binary numeric payloads.
  - Proves invalid point maps fail during config validation.

## Docs Updated

- `CHANGELOG.md`
- `docs/guides/PLC_IO_BINDING_GUIDE.md`
- `docs/specs/11-runtime-engine.md`
- `docs/public/connect/external-systems/mqtt.md`
- `docs/public/connect/protocol-matrix.md`
- `examples/communication/mqtt/README.md`

## Local Checks

```sh
cargo fmt --all -- --check
git diff --check
cargo test -p trust-runtime io::mqtt::tests --lib
```

Result:

- formatting and whitespace checks passed.
- MQTT unit module passed with 14 passed and 3 ignored.

## Remote Builder Proof

Remote copy: `trust-builder:~/projects/trust-platform-rtconn-validation`

Target dir:
`trust-builder:~/.cache/codex-targets/trust-platform-rtconn-validation`

Disk preflight before remote MQTT gates:

```text
/home/johannes: 96G free
/tmp: 6.7G free
```

Focused MQTT unit module:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  cargo test -p trust-runtime io::mqtt::tests --lib'
```

Result: passed with 14 tests, 3 ignored.

Focused process-image composition test:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  cargo test -p trust-runtime --test io_multidriver_live'
```

Result: passed with 2 tests.

Communication-specific conformance gate:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  ./scripts/runtime_comms_conformance_gate.sh'
```

Result: passed.

Common staged gate:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  just fmt'

ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  just clippy'

ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  just test'
```

Result:

- `just fmt`: passed.
- `just clippy`: passed without warnings.
- `just test`: passed through the fallback `cargo test -p trust-runtime --lib`
  with 834 passed and 16 ignored.

Networking gates:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  ./scripts/runtime_mesh_tls_stability_gate.sh --iterations 8'
```

Result: passed all 8 mesh TLS publish regression iterations.

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  RUSTFLAGS=-Dwarnings cargo check -p trust-runtime --all-targets'
```

Result: passed in 18.88s.

Disk after gates:

```text
/home/johannes: 95G free
/tmp: 6.7G free
```
