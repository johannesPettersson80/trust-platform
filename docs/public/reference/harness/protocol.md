# Harness Protocol

Use this page when you are implementing a harness client, debugging NDJSON
traffic, or checking whether a deterministic test run is reporting the right
events. It is the protocol reference; the concept page explains why virtual time
and deterministic execution matter.

Success means your client can load a project, step cycles, exchange inputs and
outputs, and distinguish protocol errors from program behavior.

Use [Deterministic Harness](../../concepts/deterministic-harness.md) first if
you need the mental model before the wire contract.

Use this page when implementing or testing a client.

Examples should link here when they depend on NDJSON behavior.

Keep client tests pinned to this contract.

## Protocol Reference

--8<-- "docs/guides/TRUST_HARNESS_PROTOCOL.md:3"

## Related

- [Agent API overview](../agent-api/overview.md)
- [Deterministic Harness](../../concepts/deterministic-harness.md)
- [trust-harness CLI](../cli/trust-harness.md)
