# Communication Example: OPC UA Runtime Wire

This example shows how to expose runtime variables over OPC UA.

## OPC UA comes with every release

The released `trust-runtime` serves OPC UA: its build feature `opcua-wire` is on by default. Only a
build from source with `--no-default-features` leaves it out, and then a `runtime.toml` with
`[runtime.opcua] enabled = true` refuses to start with a feature-disabled error.

## What you learn

- how `[runtime.opcua]` controls server endpoint and exposure
- why security defaults should be explicit at commissioning time

## Files in this folder

- `src/main.st`: sample logic (`PumpRunning := TankLevel > 50.0`)
- `src/config.st`: global symbols exposed by OPC UA
- `runtime.toml`: includes `[runtime.opcua]` section
- `io.toml`: simulated I/O for protocol-focused bring-up
- `trust-lsp.toml`: project settings

## Step 1: Build the project

Why: prove the ST compile path before the server's.

```bash
cd examples/communication/opcua
trust-runtime build --project . --sources src
```

## Step 2: Enable OPC UA server in `runtime.toml`

Why: wire server starts only when explicitly enabled, and this example keeps first boot simple.

This example already ships with these values in `runtime.toml`:

```toml
[runtime.opcua]
enabled = true
listen = "127.0.0.1:4840"
endpoint_path = "/"
namespace_uri = "urn:trust:runtime"
publish_interval_ms = 250
max_nodes = 128
expose = ["TankLevel", "PumpRunning"]
security_policy = "none"
security_mode = "none"
allow_anonymous = true
```

For production, tighten security by setting:

- `security_policy = "basic256sha256"`
- `security_mode = "sign_and_encrypt"`
- `allow_anonymous = false` with explicit `username` + `password`

## Step 3: Validate runtime config

Why: catches schema and exposure-pattern issues before launch.

```bash
trust-runtime validate --project .
```

## Step 4: Run runtime and verify endpoint

Why: confirms server boot + published node visibility.

```bash
trust-runtime run --project .
```

Use your OPC UA client to connect to `opc.tcp://127.0.0.1:4840/` and browse exposed nodes.

## Common mistakes

- building trust-runtime from source without its default features, which leaves the OPC UA server out
- using broad `expose = ["*"]` too early in commissioning
- allowing anonymous access in production networks
- skipping startup probe/read validation from a real client
