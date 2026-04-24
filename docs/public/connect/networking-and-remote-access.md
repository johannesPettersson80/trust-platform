# Networking And Remote Access

Good questions for this page:

- which endpoints need to be reachable?
- which ones should stay loopback-only?
- what firewall/TLS/token policy should I enforce?

The guide below is the practical network boundary checklist. After reading it,
you should know which runtime surfaces can remain local, which remote path is
intentional, and which security checks belong in the site runbook before remote
access is enabled.

Success means every exposed endpoint has an owner, transport, authentication
expectation, and rollback plan before it leaves loopback-only development.

Use this page before runtime-cloud, mesh, or remote HMI work whenever the
question is "should another machine be able to reach this?"

## Guide

--8<-- "docs/guides/PLC_NETWORKING.md:3"

## Related

- [Runtime To Runtime -> Security](runtime-to-runtime/security.md)
- [Operate -> Runtime UI And Control](../operate/runtime-ui-and-control.md)
