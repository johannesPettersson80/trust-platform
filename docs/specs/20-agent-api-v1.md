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

Lexical normalization is the first containment gate, not the complete security
boundary. Before a filesystem operation, the canonical workspace root and the
resolved target (or nearest existing ancestor for a new target) are compared as
specified by the canonical contract. Symbolic links, junctions, and other path
aliases MUST NOT permit a read, write, project selection, source-root
selection, or explicit harness-file load outside the workspace. Containment
failure uses `-32001` and occurs before filesystem mutation.

## 4. JSON-RPC envelope and dispatch authority

The request, response, stable-error-code, current-method, and method-note
sections included by the canonical contract are normative for JSON parsing,
envelope validation, request-ID correlation, parameter boundaries, method
discovery, dispatch, and error projection. A valid fixture or workflow result
is distinct from a JSON-RPC protocol failure; one must not be reclassified as
the other merely because the result reports unsuccessful product behavior.

## 5. Workspace and project filesystem authority

The canonical contract's workspace filesystem containment and path-bearing
method notes are normative for normalized result paths, UTF-8 byte accounting,
parent creation, project selection, source-root resolution, I/O error data,
and symbolic-link containment. Lexical acceptance never substitutes for the
canonical filesystem check, and containment is checked before mutation.

## 6. Harness request and result transaction authority

The canonical contract's `harness.load`, `harness.reload`,
`harness.execute`, individual harness-method, typed-value, and stable-error
sections are normative for source selection, validation timing, fresh versus
stateful sessions, ordered steps and assertions, accounting, failure ordering,
watch snapshots, restart modes, and run-until timeout data.

## 7. Agent workflow projection authority

The canonical contract's `workspace.project_info`, `lsp.diagnostics`,
`lsp.ast_canonicalize`, `lsp.ast_similarity`, and `lsp.format` notes are
normative for project orientation, source discovery, public diagnostic
coordinates, canonical token and five-gram results, similarity decisions, and
non-mutating formatting previews. Optional project and source-root parameters
retain their documented resolution bases; an inline override affects only the
request result and never changes the selected file. Paging on `lsp.diagnostics`
and the `issues_limit` cap on the runtime transaction methods are opt-in and
deterministic: parameters select windows of the same sorted lists, and omitted
parameters preserve the established whole-list responses.

## 8. MCP server binding

`trust-dev agent serve-mcp` exposes the Agent API as an MCP server. The
canonical contract remains the single authority on behaviour: the binding is a
transport adapter that forwards every tool call, with its raw JSON arguments,
to the same dispatch table that serves `trust-dev agent serve`, and projects
the same results and stable error codes onto MCP tool results. It adds no
method, no validation rule, and no configuration beyond the shared
`--project`/`--bundle` flag.

The binding's normative rules — the pinned `2025-06-18` protocol revision,
the 1:1 tool mapping with dot-to-underscore name mangling, the advisory
status of generated input schemas, the success and error result mappings, the
tools-only capability set, and the sequential, shared-harness,
non-preemptible concurrency model — are specified in
`docs/guides/AGENT_MCP_BINDING_V1.md`, which this section incorporates by
reference. A tool whose name does not map to an Agent API method is a JSON-RPC
protocol error (`-32601`), never a tool result.
