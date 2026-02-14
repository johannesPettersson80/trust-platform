# Communication Example: OPC UA Runtime Wire

This example shows how to expose runtime variables over OPC UA.

## Critical feature gate

OPC UA wire support is optional in this build.

- required feature: `opcua-wire`
- if `runtime.opcua.enabled = true` without `opcua-wire`, runtime startup fails with a feature-disabled error

## What you learn

- how `[runtime.opcua]` controls server endpoint and exposure
- how to confirm feature-gated behavior intentionally
- why security defaults should be explicit at commissioning time

## Files in this folder

- `src/main.st`: sample logic (`PumpRunning := TankLevel > 50.0`)
- `src/config.st`: global symbols exposed by OPC UA
- `runtime.toml`: includes `[runtime.opcua]` section
- `io.toml`: simulated I/O for protocol-focused bring-up
- `trust-lsp.toml`: project settings

## Step 1: Build with OPC UA feature enabled

Why: this confirms runtime includes wire server code path.

```bash
cd examples/communication/opcua
cargo build -p trust-runtime --features opcua-wire
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

## Step 5: Confirm expected failure mode (optional but recommended)

Why: verifies your team recognizes feature-gated startup errors.

1. Build without `opcua-wire`.
2. Keep `[runtime.opcua].enabled = true`.
3. Start runtime and confirm it fails with the feature-disabled OPC UA message.

## Common mistakes

- enabling OPC UA in config without enabling `opcua-wire` in build
- using broad `expose = ["*"]` too early in commissioning
- allowing anonymous access in production networks
- skipping startup probe/read validation from a real client
