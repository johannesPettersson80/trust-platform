# Mesh And Zenoh

truST uses a Zenoh-backed mesh surface for explicit multi-runtime sharing.

## Mesh Fits

- you want publish/subscribe-style data sharing between runtimes
- you need explicit control over what a runtime publishes or subscribes to
- discovery alone is not enough because you need live data exchange

## Core config surface

Mesh is configured in `[runtime.mesh]` in `runtime.toml`.

Typical keys:

- `enabled`
- `role`
- `listen`
- `connect`
- `tls`
- `auth_token`
- `zenohd_version`
- `publish`
- `[runtime.mesh.subscribe]`

Example:

```toml
[runtime.mesh]
enabled = true
role = "peer"
listen = "127.0.0.1:5212"
connect = []
tls = false
auth_token = ""
zenohd_version = "1.7.2"
publish = ["Status.PLCState"]

[runtime.mesh.subscribe]
"LineB:Status.PLCState" = "Local.Status.RemoteState"
```

When `auth_token` is configured, peers authenticate with that shared mesh token. Peers with
missing or different tokens cannot join the authenticated link. `tls = true` uses the
certificate, key and CA configured under `[runtime.tls]` and negotiates TLS on mesh sockets;
it does not fall back to plaintext. Certificates must cover the hostnames used to connect.

## How to think about it

- discovery finds peers
- pairing establishes trust/access workflow
- mesh moves selected runtime values between trusted peers

For same-host deterministic transport, use [Realtime T0](realtime-t0.md).
Mesh is not the HardRT path.

## Worked tutorial

--8<-- "examples/tutorials/15_multi_plc_discovery_mesh/README.md:3"

Published names may also select a nested program or equipment member, such as
`Main.Tank.Level`, or an array element such as `Main.Levels[2]`. Only the named values are
included; missing members and out-of-bounds elements are omitted. Identifiers match without
case sensitivity. This does not by itself enable HMI remote writes.
