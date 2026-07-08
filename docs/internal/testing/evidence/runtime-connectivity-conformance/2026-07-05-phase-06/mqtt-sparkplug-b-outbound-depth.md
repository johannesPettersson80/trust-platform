# Phase 6 MQTT Sparkplug B Outbound Depth Evidence

Date: 2026-07-05

Scope: bounded MQTT Sparkplug B outbound node profile for typed output metrics.
This evidence proves `RTCONN-P6-004` for namespace, version, broker publish, and
Tahu protobuf compatibility in the implemented scope. It does not claim
Sparkplug command subscriptions, device-level DBIRTH/DDATA, metric aliases,
templates, or store-and-forward.

## External Source Check

- Eclipse Tahu `sparkplug_b.proto` was checked for the scalar protobuf payload
  fields and datatype values:
  <https://github.com/eclipse-tahu/tahu/blob/master/sparkplug_b/sparkplug_b.proto>
- The implemented profile pins the MQTT namespace to `spBv1.0` and the
  supported Sparkplug spec version string to `3.0.0`.

## Implementation

- `crates/trust-runtime/src/io/mqtt/sparkplug.rs`
  - Adds Sparkplug config validation for `namespace = "spBv1.0"`,
    `spec_version = "3.0.0"`, required `group_id`, and required
    `edge_node_id`.
  - Builds NBIRTH, NDATA, and NDEATH topics as
    `spBv1.0/<group>/<message-type>/<edge-node>`.
  - Encodes a minimal Tahu-compatible proto2 payload without introducing a new
    protobuf dependency.
  - Maps scalar MQTT point types to Tahu datatype ids for `Int16`, `Int32`,
    `UInt16`, `UInt32`, `Float`, and `Boolean`.
  - Includes `bdSeq` in NBIRTH and NDEATH.

- `crates/trust-runtime/src/io/mqtt/session.rs`
  - Configures MQTT clean session false for Sparkplug sessions.
  - Configures NDEATH as the MQTT last will before the broker connection is
    opened.

- `crates/trust-runtime/src/io/mqtt/driver.rs`
  - Publishes NBIRTH once after a Sparkplug session becomes connected.
  - Publishes NDATA from typed output points during output writes.
  - Resets the birth state on disconnect/publish failure so reconnects republish
    NBIRTH.

- `crates/trust-runtime/src/io/mqtt/point_map.rs`
  - Adds optional `metric_name` for Sparkplug metrics, defaulting to the typed
    output point `topic` for backward-compatible configs.

- `crates/trust-runtime/src/io/mqtt/tests.rs`
  - Proves Sparkplug topics and NDEATH last-will configuration.
  - Proves deterministic scalar protobuf wire shape for timestamp, datatype,
    value, and sequence fields.
  - Proves driver publish order: NBIRTH then NDATA.
  - Proves unsupported shapes fail config validation.

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
- MQTT unit module passed with 18 passed and 3 ignored.

## Remote Builder Proof

Remote copy: `trust-builder:~/projects/trust-platform-rtconn-validation`

Target dir:
`trust-builder:~/.cache/codex-targets/trust-platform-rtconn-validation`

Disk preflight before remote Sparkplug gates:

```text
/home/johannes: 95G free
/tmp: 6.7G free
```

Focused MQTT/Sparkplug unit module:

```sh
ssh trust-builder 'cd "$HOME/projects/trust-platform-rtconn-validation" &&
  CARGO_TARGET_DIR="$HOME/.cache/codex-targets/trust-platform-rtconn-validation" \
  cargo test -p trust-runtime io::mqtt::tests --lib'
```

Result: passed with 18 tests, 3 ignored.

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
  with 838 passed and 16 ignored.

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

Result: passed in 19.34s.

Disk after gates:

```text
/home/johannes: 95G free
/tmp: 6.7G free
```
