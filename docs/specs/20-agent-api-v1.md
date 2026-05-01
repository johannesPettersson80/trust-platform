# Agent API v1

This specification owns the JSON-RPC contract exposed by
`trust-dev agent serve`.

## 1. Scope

The agent API is the machine-facing orchestration contract for workspace, LSP,
runtime, harness, and agent operations. It complements the standard LSP and DAP
surfaces; it does not replace them.

## 2. Canonical Contract

--8<-- "docs/guides/AGENT_CONTRACT_V1.md"
