# Agent API v1

This specification owns the JSON-RPC contract exposed by
`trust-dev agent serve`.

## 1. Scope

The agent API is the machine-facing orchestration contract for workspace, LSP,
runtime, harness, and agent operations. It complements the standard LSP and DAP
surfaces; it does not replace them.

## 2. Canonical Contract

--8<-- "docs/guides/AGENT_CONTRACT_V1.md"

## 3. Lexical workspace-relative paths

Workspace path parameters are normalized lexically before they are joined to
the configured workspace root. A current-directory component (`.`) is removed,
and an in-scope parent component (`..`) removes the preceding ordinary path
component. For example, `./src/../src/main.st` normalizes to
`src/main.st`.

A parent component that would move above the lexical workspace-relative root
is rejected with Agent API error code `-32001`. For example,
`../../secret.st` is rejected rather than normalized to an escaping path.

This section specifies lexical normalization only. It does not claim
filesystem containment across symbolic links, junctions, mount points, or
later canonical path resolution.
