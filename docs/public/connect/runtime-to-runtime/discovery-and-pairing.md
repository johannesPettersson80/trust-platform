# Discovery And Pairing

Use this guide when:

- peers are not fixed ahead of time
- you want discovery before manual endpoint wiring
- you need pairing/trust behavior explained

After reading it, you should be able to explain the split between discovery and
trust: seeing a peer is not the same as allowing data to move. Pairing and
explicit mappings are the consent boundary.

Success means a peer can be discovered, paired deliberately, and documented as
allowed before any runtime-to-runtime data path is trusted.

Use [Transport Matrix](transport-matrix.md) first if you have not yet decided
which runtime-to-runtime path fits the topology.

Keep discovery logs and pairing decisions together in the site runbook during
commissioning.

That record becomes the audit trail for why a peer was allowed.

## Guide

--8<-- "docs/guides/PLC_MULTI_NODE.md:3"
