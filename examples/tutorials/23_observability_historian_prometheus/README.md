# Tutorial 23: Observability (Historian + Prometheus Metrics)

This tutorial enables runtime observability and verifies both persisted historian
samples and Prometheus metrics export.

## Why this tutorial exists

Many teams enable runtime logic and I/O first, then postpone observability until
late commissioning. That increases startup risk because trend/alert/metrics
paths are never validated under real runtime behavior.

## What you will learn

- how to enable `[runtime.observability]` safely
- how to verify historian file output (`history/historian.jsonl`)
- how to verify Prometheus endpoint export (`/metrics`)
- how to scope recorded variables with allowlist mode

## Prerequisites

- complete Tutorial 13 first
- one shell for runtime, one shell for verification commands

## Step 1: Prepare isolated project copy

Why: observability tuning should not alter your baseline template project.

```bash
rm -rf /tmp/trust-observability
cp -R /tmp/trust-tutorial-13 /tmp/trust-observability
cd /tmp/trust-observability
```

## Step 2: Enable web + observability in `runtime.toml`

Why: Prometheus export is served via web route and historian needs explicit
recording policy.

Set/update these sections:

```toml
[runtime.web]
enabled = true
listen = "127.0.0.1:18084"
auth = "local"
tls = false

[runtime.observability]
enabled = true
sample_interval_ms = 1000
mode = "allowlist"
include = ["StartCmd", "RunLamp"]
history_path = "history/historian.jsonl"
max_entries = 20000
prometheus_enabled = true
prometheus_path = "/metrics"
```

Why these defaults:

- `allowlist` avoids recording every symbol by accident.
- explicit `include` makes retained telemetry intentional.
- local bind (`127.0.0.1`) keeps first-run exposure minimal.

## Step 3: Build and validate

Why: `mode = "allowlist"` requires a non-empty `include`, and validation catches
that class of mistakes before launch.

```bash
trust-runtime build --project . --sources src
trust-runtime validate --project .
```

## Step 4: Start runtime

Why: runtime startup confirms historian path setup and web binding.

```bash
trust-runtime run --project .
```

Leave this terminal running.

## Step 5: Generate runtime activity

Why: historian and metrics should reflect real signal changes, not idle state.

Use runtime panel/Web UI and toggle mapped inputs (for example `%IX0.0`) for at
least a few cycles so `StartCmd`/`RunLamp` values change.

## Step 6: Verify historian file output

Why: persistent telemetry is the basis for post-event diagnostics.

In another terminal:

```bash
ls -l history/historian.jsonl
tail -n 10 history/historian.jsonl
```

Expected result:

- file exists and grows over time,
- lines are JSON objects with timestamped samples,
- recorded variables match your allowlist scope.

## Step 7: Verify Prometheus endpoint

Why: this confirms metrics scraping contract before CI/monitoring integration.

```bash
curl -s http://127.0.0.1:18084/metrics | head -n 40
```

Expected result:

- endpoint responds with text exposition format,
- runtime metrics are present,
- historian counters are present when observability is enabled.

## Step 8: Harden for production

Why: observability paths are part of security and storage posture.

- change web listen/auth/TLS policy to production requirements,
- set retention limits appropriate for device storage,
- keep allowlist focused on operationally relevant symbols,
- define archiving/rotation policy for historian file handling.

## Common mistakes

- enabling `mode = "allowlist"` with empty `include`
- exposing `/metrics` broadly before network policy is ready
- recording too many variables and exhausting storage budget
- assuming observability works without testing non-idle signal changes

## Completion checklist

- [ ] observability enabled with explicit recording scope
- [ ] historian file verified with live sample updates
- [ ] `/metrics` endpoint verified locally
- [ ] production storage/network hardening decisions captured
