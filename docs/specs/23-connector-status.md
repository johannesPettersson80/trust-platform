# Connector Status Contract

This document defines the canonical connector-status vocabulary shared by the
runtime API, CLI and reports, browser HMI, and VS Code surfaces.

## Canonical Vocabulary

Connector lifecycle state is exactly one of `disabled`, `configured`,
`starting`, `ready`, `degraded`, `reconnecting`, `stale`, `not_ready`, or
`faulted`. Connector health is exactly one of `ok`, `degraded`, `faulted`, or
`unknown`.

Discovery confidence is exactly one of `confirmed`, `likely`,
`port_reachable`, or `unavailable`. Point quality is exactly one of `good`,
`stale`, `bad`, `unsupported`, `unavailable`, `write_pending`, or
`write_failed`.

## Projection Rules

Missing freshness evidence is not itself a `stale` state. A stale state needs
positive evidence that previously usable data exceeded its freshness bound.
`degraded` and `faulted` projections preserve available error detail and point
quality rather than replacing them with a generic label. `disabled`,
`reconnecting`, `stale`, `degraded`, and `faulted` remain distinct.

Wire identifiers are stable lowercase values. Human-facing surfaces may add
presentation text, but HMI, CLI, reports, hover, and VS Code must preserve the
same state and health meaning. An unknown wire identifier fails visibly at a
closed mapping boundary; it is not silently promoted to a healthy state.
