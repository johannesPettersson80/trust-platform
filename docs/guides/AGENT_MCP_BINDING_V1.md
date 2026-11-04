# Agent API MCP Binding v1

`trust-dev agent serve-mcp` exposes the Agent API v1 contract as a Model
Context Protocol (MCP) server. The Agent API keeps exactly one owner of
behaviour: the JSON-RPC contract documented in `AGENT_CONTRACT_V1.md`. This
binding is a thin transport adapter that forwards every tool call to the same
dispatch table that serves `trust-dev agent serve`; it adds no methods, no
validation rules, and no second copy of the contract.

Implementation basis: the official Rust MCP SDK, the `rmcp` crate, pinned to
`3.8.0` (which implements the MCP protocol revisions up to and including
`2026-07-28`).

## Transport and Protocol Revision

- Transport: MCP stdio (JSON-RPC 2.0 messages, one per line, over stdin/stdout)
- Pinned protocol revision: `2025-06-18`
- The server advertises exactly `2025-06-18` as its supported protocol
  revision. A client that requests `2025-06-18` receives it back; a client
  that requests any other revision is answered with `2025-06-18` during the
  `initialize` handshake, because it is the only revision this server speaks.

The Agent API contract itself remains `stdio` + `jsonl` (see
`AGENT_CONTRACT_V1.md`). `serve-mcp` reuses the same framing discipline on the
MCP stdio transport.

## Server Identity and Capabilities

`initialize` returns:

- `protocolVersion`: `2025-06-18`
- `capabilities`: `tools` only. Resources, prompts, logging, sampling,
  elicitation, completion, and tasks are intentionally not advertised.
- `serverInfo`: `name = "trust-dev"`, `version` = the truST workspace version
  the binary was built from.

## Tool Mapping

The MCP server exposes one tool per Agent API method: a 1:1 projection of the
`agent.describe.methods` list. No method is omitted, and no tool exists that
does not correspond to an Agent API method.

MCP tool names MUST match `^[a-zA-Z0-9_-]{1,64}$`; the `.` used by the Agent
API method names is not a legal tool-name character. Every dot is therefore
replaced with an underscore. The mangled-to-canonical mapping is stored in the
server's tool table, not re-derived by string manipulation at dispatch time
(a reverse mapping cannot distinguish `workspace.project_info` from a
hypothetical `workspace.project.info`). An unknown tool name is rejected with
a JSON-RPC protocol error `-32601`.

| Agent API method | MCP tool |
| --- | --- |
| `agent.describe` | `agent_describe` |
| `workspace.read` | `workspace_read` |
| `workspace.write` | `workspace_write` |
| `workspace.project_info` | `workspace_project_info` |
| `lsp.diagnostics` | `lsp_diagnostics` |
| `lsp.ast_canonicalize` | `lsp_ast_canonicalize` |
| `lsp.ast_similarity` | `lsp_ast_similarity` |
| `lsp.format` | `lsp_format` |
| `runtime.build` | `runtime_build` |
| `runtime.compile_reload` | `runtime_compile_reload` |
| `runtime.validate` | `runtime_validate` |
| `runtime.test` | `runtime_test` |
| `runtime.reload` | `runtime_reload` |
| `harness.load` | `harness_load` |
| `harness.reload` | `harness_reload` |
| `harness.execute` | `harness_execute` |
| `harness.cycle` | `harness_cycle` |
| `harness.set_input` | `harness_set_input` |
| `harness.get_output` | `harness_get_output` |
| `harness.advance_time` | `harness_advance_time` |
| `harness.run_until` | `harness_run_until` |

Every tool carries a non-empty one-line description and an `inputSchema` that
is a JSON Schema object.

### `agent_describe`

`agent.describe` remains a tool. Its payload is byte-identical to the
JSON-RPC method's payload: it describes the Agent API v1 contract — the
`stdio` transport and `jsonl` framing of `trust-dev agent serve` — not the
MCP transport over which the tool call arrived. This is intended: the method
reports the Agent API contract it belongs to, and this binding adds no method
whose payload could report the MCP surface.

## Input Schemas Are Advisory

Each tool's `inputSchema` is generated from the Agent API parameter structs.
JSON Schema cannot express every rule the contract enforces. The clearest
example is `harness.load`, whose `inline_sources`, `files`, and `project`
selectors are mutually exclusive and fail-closed; that rule is not expressible
as JSON Schema, so the generated schema is more permissive than the contract.

This is intended behaviour, not a defect: param enforcement stays entirely in
the Agent API dispatch layer. A call whose arguments violate a contract rule
that the schema fails to express is rejected with the contract's `-32602`
error, mapped as an error tool result below. Hosts and models MUST treat the
schema as a hint and the contract as the authority.

## Result Mapping

A successful Agent API result becomes an MCP tool result with:

- `structuredContent`: the Agent API JSON result object, verbatim;
- `content`: one text block containing the same result, pretty-printed as
  JSON, for clients that only render text;
- `isError`: `false`.

## Error Mapping

MCP distinguishes tool-level failures (an error tool result the caller can
read) from JSON-RPC protocol failures. The binding maps the Agent API's own
error taxonomy to error tool results so that contract-stable codes survive
the transport:

| Agent API error | MCP projection |
| --- | --- |
| `-32001` workspace path escape | error tool result |
| `-32002` I/O or runtime command failure | error tool result |
| `-32003` harness not loaded | error tool result |
| `-32004` `harness.run_until` budget exceeded | error tool result |
| `-32602` invalid params | error tool result |

An error tool result carries:

- `isError`: `true`;
- `content`: one text block whose text is the contract-stable JSON error
  object `{"code": <code>, "message": <message>, "data": <data-or-absent>}`;
- no `structuredContent`.

Errors that belong to the JSON-RPC envelope, not to an Agent API method —
`-32700` parse, `-32600` invalid request, `-32601` unknown method — remain
JSON-RPC protocol errors on the MCP session; in particular an unknown tool
name is rejected as a `-32601` protocol error and never reaches the tool
result.

## Large Projects and Response Size

- Whole-project results can be large. A project whose analysis produces many
  issues (for example when vendor stub libraries are not yet wired into the
  analysis path) yields a single multi-megabyte JSON response, and real MCP
  hosts enforce message-size limits and call timeouts on stdio traffic.
- `lsp_diagnostics` therefore supports the contract's opt-in paging:
  pass `page` and `page_size` together to bound each response and read
  `totalIssues` / `totalPages` to iterate. Omitting both returns the full
  list exactly as the JSON-RPC surface does.
- `runtime_compile_reload`, `runtime_build`, and `runtime_validate` accept
  the contract's opt-in `issues_limit`; an applied limit caps `issues[]` and
  adds `issuesTruncated` and `totalIssues` to the result.
- Clients driving large projects should set these parameters
  (for example `page_size: 500`, `issues_limit: 500`). Without them the
  tools return whole lists, which can exceed a host's message limit; the
  call itself still completes server-side.

## Concurrency and Cancellation

- Requests are served sequentially per server instance, in arrival order.
  There is no parallel tool execution.
- The Agent API harness methods share one stateful harness session per server
  instance (`harness.load` then `harness.cycle` observe the same session).
  Sequential serving is what makes that session model safe.
- Long-running calls (builds, tests) block later calls until they finish.
- `notifications/cancelled` is accepted but cannot preempt a running call; it
  is ignored in this version. A caller that no longer wants a result should
  simply not read it; the call itself runs to completion.

## Scope Boundary

This binding adds no capabilities beyond `tools`, no transport other than
stdio, and no configuration surface beyond the `--project`/`--bundle` flag
shared with `trust-dev agent serve`. Host configuration examples live in
`docs/public/reference/agent-api/mcp.md`.
