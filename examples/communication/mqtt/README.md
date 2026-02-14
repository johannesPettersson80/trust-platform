# Communication Example: MQTT

This example shows how to use `io.driver = "mqtt"` for broker-based I/O exchange.

## What you learn

- topic design for PLC input/output channels
- reconnect and keep-alive tuning basics
- why secure broker boundary decisions must be explicit

## Files in this folder

- `src/main.st`: minimal `%IX -> %QX` logic (`DO0 := DI0`)
- `src/config.st`: `%IX0.0` / `%QX0.0` global mapping and task binding
- `io.toml`: MQTT backend profile
- `runtime.toml`: runtime defaults
- `trust-lsp.toml`: project settings

## Step 1: Build first

Why: separate compiler issues from transport issues.

```bash
cd examples/communication/mqtt
trust-runtime build --project . --sources src
```

## Step 2: Review `io.toml`

Why: topic and reconnect policy directly shape reliability and security.

```toml
[io]
driver = "mqtt"

[io.params]
broker = "127.0.0.1:1883"
topic_in = "trust/examples/mqtt/in"
topic_out = "trust/examples/mqtt/out"
reconnect_ms = 500
keep_alive_s = 5
allow_insecure_remote = false
```

Field intent:

- `broker`: MQTT endpoint.
- `topic_in`: messages consumed into `%I` image.
- `topic_out`: messages published from `%Q` image.
- `reconnect_ms`: backoff cadence for broken sessions.
- `keep_alive_s`: session liveness interval.
- `allow_insecure_remote`: blocks unsafe remote configuration.

## Step 3: Validate config

Why: detect missing mandatory fields and invalid values early.

```bash
trust-runtime validate --project .
```

## Step 4: Run against a broker

Why: topic contract validation must happen with a real broker path.

1. Start broker on `127.0.0.1:1883`.
2. Run runtime:

```bash
trust-runtime run --project .
```

3. In another terminal, inspect I/O:

```bash
trust-runtime ctl --project . io-read
```

## Step 5: Production hardening checklist

Why: MQTT deployments often fail at boundary assumptions, not syntax.

- enforce broker auth/TLS according to site policy
- keep input/output topics separate and explicit
- define ACLs for publish/subscribe directions
- confirm reconnect behavior under broker restart tests

## Common mistakes

- reusing same topic for both `topic_in` and `topic_out`
- leaving broker ACLs open during rollout
- treating successful `validate` as proof of runtime connectivity
- dropping `safe_state` definitions for output-critical projects
