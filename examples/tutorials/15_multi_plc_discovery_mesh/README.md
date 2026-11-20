# Tutorial 15: Multi-PLC Mesh Sharing and Discovery

This tutorial runs two runtimes on one machine and shares a value between them:

1. PLC-B publishes its lamp output over the mesh,
2. PLC-A receives it into a variable of its own,
3. PLC-A finds PLC-B with discovery.

## Why this tutorial exists

Single-PLC workflows are only the start. Real plants often split logic across
multiple runtimes. You need a repeatable method to exchange selected data
between runtimes, explicitly and safely.

## What you will learn

- how a runtime publishes program variables over the mesh
- how another runtime subscribes to them, by the publisher's resource name
- why each runtime needs its own resource name and ports
- how discovery finds a peer, and how the mesh is secured across a network

## Prerequisites

- complete Tutorial 13 first
- one machine is enough: the two runtimes use different ports

## Step 1: Create PLC-A and PLC-B project copies

Why: separate project folders stand for independent PLC nodes.

```bash
rm -rf /tmp/trust-plc-a /tmp/trust-plc-b
cp -R /tmp/trust-tutorial-13 /tmp/trust-plc-a
cp -R /tmp/trust-tutorial-13 /tmp/trust-plc-b
```

## Step 2: Give PLC-A a variable for the shared value

Why: a subscription writes the value it receives into a variable of the
receiving program, so PLC-A needs one.

```bash
cat > /tmp/trust-plc-a/src/Main.st <<'ST'
PROGRAM FirstApp
VAR
    StartCmd : BOOL;
    LampOut : BOOL;
    RemoteLamp : BOOL;
END_VAR

LampOut := StartCmd;
END_PROGRAM
ST
```

## Step 3: Configure the two runtimes

Why: two runtimes on one machine need their own resource names and ports.
PLC-B publishes `P1.LampOut`, the lamp output of its program instance `P1`.
PLC-A connects to PLC-B's mesh port and subscribes to that variable under
PLC-B's resource name, `PlcB`, writing it into its own `P1.RemoteLamp`. Each
project keeps its own control endpoint by default, so neither file names one.

Set PLC-B's runtime config:

```bash
cat > /tmp/trust-plc-b/runtime.toml <<'TOML'
[bundle]
version = 1

[resource]
name = "PlcB"
cycle_interval_ms = 100

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:18102"
auth = "local"

[runtime.discovery]
enabled = true
service_name = "LineB"
advertise = false
interfaces = []

[runtime.mesh]
enabled = true
listen = "127.0.0.1:5212"
publish = ["P1.LampOut"]
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

Set PLC-A's runtime config:

```bash
cat > /tmp/trust-plc-a/runtime.toml <<'TOML'
[bundle]
version = 1

[resource]
name = "PlcA"
cycle_interval_ms = 100

[runtime.control]
mode = "production"
debug_enabled = false

[runtime.web]
enabled = true
listen = "127.0.0.1:18101"
auth = "local"

[runtime.discovery]
enabled = true
service_name = "LineA"
advertise = false
interfaces = []

[runtime.mesh]
enabled = true
listen = "127.0.0.1:5211"
connect = ["127.0.0.1:5212"]
publish = []

[runtime.mesh.subscribe]
"PlcB:P1.LampOut" = "P1.RemoteLamp"

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

What the mesh settings do:

- `publish` lists the variables a runtime shares: a global, or a program's
  variable by its full path, such as `P1.LampOut`.
- `[runtime.mesh.subscribe]` maps `"<publisher's resource name>:<path>"` to a
  variable of this runtime.
- `connect` names the peers to connect to. On one machine PLC-A connects to
  PLC-B's port directly; on a network, a peer is reached at its network address.
- The mesh here has no `auth_token`. It listens on this computer only, but a
  mesh also looks for other meshes on the network by multicast and connects to
  those it finds, so a mesh without a token exchanges data with any of them.
  Step 8 secures a mesh across a network.

## Step 4: Build and validate both projects

```bash
trust-runtime build --project /tmp/trust-plc-a --sources src
trust-runtime validate --project /tmp/trust-plc-a
trust-runtime build --project /tmp/trust-plc-b --sources src
trust-runtime validate --project /tmp/trust-plc-b
```

Expected result:
- `Project ok` for both

## Step 5: Start both runtimes

Terminal A:

```bash
trust-runtime run --project /tmp/trust-plc-b
```

Terminal B:

```bash
trust-runtime run --project /tmp/trust-plc-a
```

## Step 6: Share the lamp over the mesh

Why: configuration is not proof; watch the value cross from one runtime to the
other.

In a third terminal, switch PLC-B's input on, read its lamp, then read PLC-A's
copy:

```bash
trust-runtime ctl --project /tmp/trust-plc-b io-write %IX0.0 true
trust-runtime ctl --project /tmp/trust-plc-b read P1.LampOut
trust-runtime ctl --project /tmp/trust-plc-a read P1.RemoteLamp
```

Expected result:
- `P1.LampOut : BOOL = true` on PLC-B
- `P1.RemoteLamp : BOOL = true` on PLC-A, within a second

Switch the input off again with `io-write %IX0.0 false`: `P1.RemoteLamp` follows
to `false`.

## Step 7: Find PLC-B with discovery

Why: on a plant network you find runtimes instead of typing their addresses.

Ask PLC-A to search, through its control endpoint:

```bash
trust-runtime ctl --project /tmp/trust-plc-a endpoint
trust-runtime discover --only trust_runtime --endpoint <the endpoint printed above>
```

Expected result:
- the search finds the truST runtime `PlcB`, through the mesh

Discovery also browses the network with mDNS, on every interface but the
loopback (`interfaces = []`), and a runtime announces itself there only with
`advertise = true`. Both runtimes here keep `advertise = false` and listen on
this computer only, so PLC-A finds PLC-B through the mesh.

## Step 8: Secure the mesh across a network

Why: on a network, any computer could join an unauthenticated mesh.

When the runtimes run on different computers:

- set `listen` to an address on the plant network, and `connect` to the peer's
  address;
- give both meshes the same `auth_token` (`openssl rand -hex 24`): a peer
  without it, or with another, cannot join;
- set `tls = true`: the mesh then uses the certificate of `[runtime.tls]`
  (Tutorial 16, step 4) and accepts a peer only with a certificate signed by
  the authority of `ca_path` that covers the address it is reached at: give
  each PLC a certificate from your site's certificate authority, and that
  authority's certificate as `ca_path`;
- open the mesh port only between the PLCs.

Tutorial 16 shows how a device gets its own token by pairing.

## Common mistakes

- the same web or mesh port for both PLCs
- subscribing under the publisher's discovery `service_name` instead of its
  resource name
- publishing or subscribing a name that the program does not have: nothing
  flows, and nothing fails
- editing `runtime.toml` without restarting the runtime

## Completion checklist

- [ ] both runtimes run with their own resource names and ports
- [ ] PLC-A's `P1.RemoteLamp` follows PLC-B's `P1.LampOut`
- [ ] PLC-A's search finds `PlcB`
- [ ] you know which settings secure a mesh across a network
