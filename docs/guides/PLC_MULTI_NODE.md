# Multi‑PLC (Multiple Runtimes) Guide

This guide explains how to find and connect multiple PLC runtimes on one network.
Tutorial 15 walks through it on one machine, and Tutorial 16 secures it.

## 1) Start each PLC

On each device:

```bash
trust-runtime run --project /path/to/project
```

Give each runtime its own `[resource] name`: other runtimes address its shared
data by that name. A project created by the runtime's setup or the browser IDE
listens on every network interface and announces itself with discovery. A
project that keeps to its own computer, as the tutorials' projects do, opens up
with:

```toml
[runtime.web]
listen = "0.0.0.0:8080"

[runtime.discovery]
enabled = true
advertise = true
```

## 2) Find PLCs

From any computer on the network:

```bash
trust-runtime discover --only trust_runtime
```

Expected outcome:

- the runtimes that advertise themselves with mDNS are listed, with their names

The browser IDE's Online panel lists the runtimes that its runtime discovered.
A runtime also finds its mesh peers: `trust-runtime discover --only
trust_runtime --endpoint <a runtime's control endpoint>` searches from inside
that runtime (Tutorial 15, step 7).

## 3) Pair with another PLC (optional)

Pairing gives a device its own token and role on a runtime: start a pairing on
the runtime, claim it from the device with the six-digit code. It goes through
the runtime's web API (`/api/pair/start`, `/api/pair/claim`, `/api/pairings`);
Tutorial 16, step 9, shows each request. On the PLC, the terminal UI
(`trust-runtime ui`) does the same with `/access start`, `/access claim`,
`/access list` and `/access revoke`.

## 4) Share data between PLCs

Data moves only through explicit mesh mappings in `runtime.toml`: one runtime
publishes variables, another subscribes to them under the publisher's resource
name and writes them into its own variables:

```toml
[runtime.mesh]
enabled = true
listen = "0.0.0.0:5200"
connect = ["192.168.1.20:5200"]
auth_token = "the same token on every PLC of this mesh"
publish = ["P1.LampOut"]

[runtime.mesh.subscribe]
"PlcB:P1.LampOut" = "P1.RemoteLamp"
```

A mesh also finds the other meshes on its network by multicast and connects to
them. Give every PLC of one mesh the same `auth_token`, so that no other device
can join it. Restart the runtime after editing `runtime.toml`.

## Troubleshooting

If PLCs don't appear:

- confirm that they are on the same LAN or VLAN, and that mDNS is allowed;
- confirm that each listens on a network address and advertises itself;
- `[runtime.discovery] interfaces` names the interfaces discovery uses, such as
  `["eth0"]`; an empty list means every interface but the loopback;
- connect the mesh by address (`connect`) where discovery is blocked.

If shared values don't arrive:

- the subscription must name the publisher's resource name and a published path;
- both meshes need the same `auth_token`, or none.
