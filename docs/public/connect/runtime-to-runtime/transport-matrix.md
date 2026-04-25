# Runtime-To-Runtime Transport Matrix

| Surface | What it does | Best for | Not for | Docs |
| --- | --- | --- | --- | --- |
| `discovery` | advertises and finds runtimes on a LAN | first contact, browsing peers, pairing bootstrap | deterministic data exchange by itself | [Discovery And Pairing](discovery-and-pairing.md) |
| `zenoh` / mesh | shares explicit published values and subscriptions | multi-runtime data sharing on a plant network | same-host HardRT contracts | [Mesh And Zenoh](mesh-zenoh.md) |
| `realtime` | same-host bounded shared-memory transport | tightly coupled low-latency paths | WAN or generic IP mesh | [Realtime T0](realtime-t0.md) |
| `web` / runtime-cloud | browser/fleet control plane, preflight, dispatch, rollout | federation, topology, remote operations | replacing deterministic data transport | [Runtime Cloud Federation](runtime-cloud-federation.md) |

## Recommended decision order

1. Start with `discovery` if you are still finding peers or bootstrapping trust.
2. Use `zenoh` mesh when the goal is selective runtime-to-runtime data flow.
3. Use `realtime` only when you explicitly need same-host deterministic
   transport.
4. Use runtime-cloud/web when the problem is orchestration, rollout, or fleet
   visibility rather than raw signal sharing.

## Security boundary

`connect/` answers how you wire runtimes together. `operate/` answers how you
run them once they are connected.

- setup and exposure decisions: [Security](security.md)
- day-to-day federation/operator work: [Run / Runtime Cloud](../../operate/runtime-cloud.md)
