# Tutorial 15: Multi-PLC Discovery, Pairing, and Mesh Sharing

This tutorial runs two runtimes and walks through:

1. network discovery,
2. optional pairing,
3. mesh data sharing.

## Why this tutorial exists

Single-PLC workflows are only the start. Real plants often split logic across
multiple runtimes. You need a repeatable method to discover peers, trust peers,
and exchange selected data safely.

## What you will learn

- how to configure two runtimes so they can be discovered
- why service names and unique ports matter
- how to enable mesh publish/subscribe with explicit mappings

## Prerequisites

- complete Tutorial 13 first
- one machine is enough (we will run two runtimes with different ports)

## Step 1: Create PLC-A and PLC-B project copies

Why: separate project folders simulate real independent PLC nodes.

```bash
rm -rf /tmp/trust-plc-a /tmp/trust-plc-b
cp -R /tmp/trust-tutorial-13 /tmp/trust-plc-a
cp -R /tmp/trust-tutorial-13 /tmp/trust-plc-b
```

Expected result:
- two independent project trees

## Step 2: Configure unique runtime endpoints

Why: each runtime needs unique control/web/mesh endpoints on one host.

Set PLC-A runtime config:

```bash
cat > /tmp/trust-plc-a/runtime.toml <<'TOML'
[bundle]
version = 1

[resource]
name = "PlcA"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime-plc-a.sock"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:18101"
auth = "local"

[runtime.discovery]
enabled = true
service_name = "LineA"
advertise = true
interfaces = []

[runtime.mesh]
enabled = true
listen = "127.0.0.1:5211"
auth_token = ""
publish = []
subscribe = {}

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
TOML
```

Set PLC-B runtime config:

```bash
cat > /tmp/trust-plc-b/runtime.toml <<'TOML'
[bundle]
version = 1

[resource]
name = "PlcB"
cycle_interval_ms = 100

[runtime.control]
endpoint = "unix:///tmp/trust-runtime-plc-b.sock"
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:18102"
auth = "local"

[runtime.discovery]
enabled = true
service_name = "LineB"
advertise = true
interfaces = []

[runtime.mesh]
enabled = true
listen = "127.0.0.1:5212"
auth_token = ""
publish = ["Status.PLCState"]
subscribe = {}

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
TOML
```

Expected result:
- both runtimes are discoverable
- PLC-B publishes one mesh point (`Status.PLCState`)

## Step 3: Build both projects

Why: each node still needs valid bytecode before it can run.

```bash
trust-runtime build --project /tmp/trust-plc-a --sources /tmp/trust-plc-a/src
trust-runtime build --project /tmp/trust-plc-b --sources /tmp/trust-plc-b/src
```

Expected result:
- `program.stbc` exists in both folders

## Step 4: Start both runtimes

Why: discovery and mesh only work when both nodes are online.

Terminal A:

```bash
trust-runtime run --project /tmp/trust-plc-a
```

Terminal B:

```bash
trust-runtime run --project /tmp/trust-plc-b
```

Expected result:
- PLC-A Web UI on `http://127.0.0.1:18101`
- PLC-B Web UI on `http://127.0.0.1:18102`

## Step 5: Verify discovery from Web UI

Why: discovery is the first proof that nodes can see each other on the network.

1. Open PLC-A Web UI (`http://127.0.0.1:18101`).
2. Go to `Network -> Discovery`.
3. Confirm `LineB` appears.

Expected result:
- PLC-B is listed with reachable link

If discovery fails:
- verify both runtimes are running
- verify `runtime.discovery.enabled = true`
- use manual add with PLC-B URL (`127.0.0.1:18102`)

## Step 6: Pair nodes (optional but recommended)

Why: pairing is a safer trust workflow than sharing static credentials manually.

1. In PLC-B, open `Network -> Pairing` and generate a code.
2. In PLC-A, claim PLC-B using that code.

Expected result:
- PLC-A can access PLC-B without repeated token prompts

## Step 7: Configure mesh subscription on PLC-A

Why: publish/subscribe must be explicit so only intended data flows between
runtimes.

Edit `/tmp/trust-plc-a/runtime.toml` and replace `subscribe = {}` with:

```toml
[runtime.mesh.subscribe]
"LineB:Status.PLCState" = "Local.Status.RemoteState"
```

Restart PLC-A after edit.

Expected result:
- PLC-A subscribes to PLC-B status

## Step 8: Verify mesh connection

Why: operational confidence requires observing active mesh links, not just
configuration files.

1. Open PLC-A Web UI.
2. Go to `Network -> Mesh connections`.
3. Confirm connection to `LineB` is present.

Expected result:
- mesh link appears as connected/healthy

## Common mistakes

- same web or mesh ports for both PLCs
- service name mismatch (`LineB` in config vs actual runtime name)
- editing `runtime.toml` without restarting runtime

## Completion checklist

- [ ] PLC-A and PLC-B both running
- [ ] discovery shows peer runtime
- [ ] optional pairing completed
- [ ] mesh publish/subscription configured
- [ ] mesh connection observed in UI
