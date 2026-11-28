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
listen = "127.0.0.1:5211"
connect = ["127.0.0.1:5212"]
tls = false
zenohd_version = "1.7.2"
publish = ["P1.LampOut"]

[runtime.mesh.subscribe]
"PlcB:P1.LampOut" = "P1.RemoteLamp"
```

This runtime publishes its program's `P1.LampOut`, connects to the peer at
`127.0.0.1:5212` and writes that peer's `P1.LampOut` into its own `P1.RemoteLamp`:
a subscription names the publisher by its `[resource] name` (here `PlcB`) and a
path it publishes. An endpoint is `host:port` or a Zenoh locator such as
`tcp/host:port`.

A runtime's mesh keys are under its site (`[runtime.cloud] site`, see
[Runtime Cloud Profiles](../../reference/config/runtime-cloud.md#sites-and-runtime-ids)). A
subscription names a publisher of its own site by its resource name, as above, and a publisher of
another site by `<site>/<resource>`, for example `"site-b/PlcB:P1.LampOut"`.

A mesh links only to the peers its `connect` entries name and to the runtimes
that connect to its `listen` endpoint: it does not look for other meshes on the
network, and it does not link to the peers of its peers. A mesh that listens on
a network address accepts every runtime that connects to it, so without
`auth_token` any of them can join and exchange data.

When `auth_token` is configured, peers authenticate with that shared mesh token. Peers with
missing or different tokens cannot join the authenticated link. `tls = true` uses the
certificate, key and CA configured under `[runtime.tls]` and negotiates TLS on mesh sockets;
it does not fall back to plaintext. Certificates must cover the hostnames used to connect.

A runtime logs every runtime that joins its mesh (`mesh_peer_joined`) or leaves it (the warning
`mesh_peer_left`), and 5 s after its start it warns with `mesh_peer_missing` about each runtime that
a subscription names and that is not in the mesh, with the subscriptions that wait for it. A runtime
that refuses a peer with another token does not log it, so when a value does not arrive, look for
`mesh_peer_missing` on the subscribing side, then check the peer's name, its address and the tokens.

## How to think about it

- discovery lists the runtimes on the network; it links no mesh
- pairing establishes trust/access workflow
- mesh moves selected runtime values between the peers that `connect` links

For same-host deterministic transport, use [Realtime T0](realtime-t0.md).
Mesh is not the HardRT path.

## Worked tutorial

--8<-- "examples/tutorials/15_multi_plc_discovery_mesh/README.md:3"

Published names may also select a nested program or equipment member, such as
`Main.Tank.Level`, or an array element such as `Main.Levels[2]`. Only the named values are
included; missing members and out-of-bounds elements are omitted. Identifiers match without
case sensitivity. This does not by itself enable HMI remote writes.
