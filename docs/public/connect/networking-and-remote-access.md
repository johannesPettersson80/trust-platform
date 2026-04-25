# Networking And Remote Access

Decide which endpoints stay loopback-only, which endpoints become reachable
from another machine, and which firewall/TLS/token policy owns that exposure.

Every exposed endpoint needs an owner, transport, authentication expectation,
and rollback plan before it leaves local development.

That question usually appears before runtime-cloud, mesh, or remote HMI work.

## Guide

--8<-- "docs/guides/PLC_NETWORKING.md:3"

## Related

- [Runtime To Runtime / Security](runtime-to-runtime/security.md)
- [Run / Runtime UI And Control](../operate/runtime-ui-and-control.md)
