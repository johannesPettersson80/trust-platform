# Agent Contract v1

`trust-dev agent serve` is the first stable automation surface outside VS Code.

It exists to let agents, shell automation, and future hosted workers drive truST
without depending on editor internals.

## Transport and Security

- Transport: `JSON-RPC 2.0` over `stdio`
- Framing: one JSON request per input line, one JSON response per output line
- Network listeners: not supported in v1
- Security rule for any future non-stdio mode:
  - default to loopback only
  - refuse non-loopback unless explicit auth is configured
  - unauthenticated remote execution is unsupported

## Request Envelope

```json
{"jsonrpc":"2.0","id":1,"method":"agent.describe","params":{}}
```

## Response Envelope

Success:

```json
{"jsonrpc":"2.0","id":1,"result":{"transport":"stdio"}}
```

Failure:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method 'missing' is not available."}}
```

## Stable Error Codes

| Code | Meaning |
| --- | --- |
| `-32700` | Parse error |
| `-32600` | Invalid JSON-RPC envelope |
| `-32601` | Unknown method |
| `-32602` | Invalid params |
| `-32001` | Workspace path escapes the workspace root |
| `-32002` | I/O or runtime command failure |
| `-32003` | Harness not loaded |
| `-32004` | `harness.run_until` exceeded the requested cycle budget |

## Current Methods

| Method | Purpose |
| --- | --- |
| `agent.describe` | Return transport info, workspace root, and supported method list |
| `workspace.read` | Read a file inside the workspace root |
| `workspace.write` | Write a file inside the workspace root |
| `workspace.project_info` | Return project-root, source-root, dependency, config, and runtime-orientation metadata |
| `lsp.diagnostics` | Return machine-readable diagnostics for one ST file or a whole project source tree |
| `lsp.format` | Return a formatting preview for one ST file without mutating disk |
| `runtime.build` | Reuse the `trust-runtime build --ci` payload path |
| `runtime.compile_reload` | Run the full diagnose -> build -> reload loop and return a single structured result |
| `runtime.validate` | Reuse the `trust-runtime validate --ci` payload path |
| `runtime.test` | Reuse the `trust-dev test --output json` payload path |
| `runtime.reload` | Rebuild `program.stbc` and issue `bytecode.reload` to a running runtime control endpoint |
| `harness.load` | Load inline/project sources into a deterministic in-process harness |
| `harness.reload` | Reload harness sources while preserving retain semantics when supported |
| `harness.cycle` | Advance cycles, optionally advancing virtual time first |
| `harness.set_input` | Set a harness input/global/program variable |
| `harness.get_output` | Read a harness output/global/program variable |
| `harness.advance_time` | Advance virtual time without executing a cycle |
| `harness.run_until` | Cycle until a named output matches an expected value |

## Method Notes

### `workspace.read`

Params:

```json
{"path":"src/main.st"}
```

### `workspace.write`

Params:

```json
{"path":"src/main.st","text":"PROGRAM Main\nEND_PROGRAM\n","create_parents":true}
```

### `workspace.project_info`

Optional params:

```json
{"project":"runtime-a","sources_root":"src"}
```

Returns a stable orientation payload for agents:

- canonical `project`
- resolved `sourcesRoot`
- `sourceCount`
- `sources`
- `dependencyRoots`
- `resolvedDependencies`
- file presence for `runtime.toml`, `io.toml`, `simulation.toml`,
  `trust-lsp.toml`, and `program.stbc`
- `lsp.vendorProfile` when `trust-lsp.toml` declares one
- runtime summary (`controlEndpoint`, web/discovery/mesh settings, etc.) when
  `runtime.toml` parses cleanly
- IO summary (`drivers`, `driverCount`, `safeStateCount`) when `io.toml`
  parses cleanly

If runtime or IO config files exist but fail to parse, the payload keeps the
file path and reports `parseError` instead of failing the whole request.

### `runtime.build`

Optional params:

```json
{"project":"runtime-a","sources_root":"src"}
```

Returns the same machine-readable payload shape as `trust-runtime build --ci`.

### `lsp.diagnostics`

Optional params:

```json
{"project":"runtime-a","sources_root":"src","path":"src/main.st","content":"PROGRAM Main\nEND_PROGRAM\n"}
```

Rules:

- `project` is relative to the agent workspace root when present
- `sources_root` is relative to the selected project root when present
- `path` is relative to the selected project root when present
- `content` is an in-memory override for `path`; it is invalid without `path`

If `path` is omitted, the method analyzes all `.st` / `.pou` files under the
project `src/` tree (or `sources_root` override) and returns one aggregated
diagnostic list.

Returns:

- `target`
- `errors`
- `warnings`
- `issues[]` with `path`, absolute `file`, `line`, `column`, `severity`,
  `message`, optional `code`, and optional related entries

### `lsp.format`

Params:

```json
{"project":"runtime-a","path":"src/main.st","content":"PROGRAM Main\nEND_PROGRAM\n"}
```

Rules:

- `path` is required and is relative to the selected project root
- `content` is optional; when omitted, the formatter reads the file from disk

Returns a formatting preview only:

- `path`
- absolute `file`
- formatted `content`
- `changed`

### `runtime.validate`

Optional params:

```json
{"project":"runtime-a"}
```

Returns the same machine-readable payload shape as `trust-runtime validate --ci`.

### `runtime.test`

Optional params:

```json
{"project":"runtime-a","filter":"CI_","list":false,"timeout_seconds":5}
```

Returns the same JSON summary shape as `trust-dev test --output json`.

### `runtime.compile_reload`

Optional params:

```json
{"project":"runtime-a","sources_root":"src","endpoint":"tcp://127.0.0.1:9001","token":"secret"}
```

Behavior:

- collects diagnostics over the selected project source tree
- blocks build/reload when diagnostics contain errors
- reuses the normal `trust-runtime build` path when diagnostics are clean
- reuses the same `bytecode.reload` control-endpoint path as `runtime.reload`

Returns a single machine-readable loop result with:

- `target`
- `dirty` (currently always `false` for agent-driven file writes)
- `errors`
- `warnings`
- `issues`
- `runtimeStatus`
- `runtimeMessage`
- optional `build`
- optional `reload`

This is the preferred agent method for iterative `write -> diagnose -> build -> reload`
loops because it keeps diagnostics and reload state in one payload.

### `runtime.reload`

Optional params:

```json
{"project":"runtime-a","sources_root":"src","endpoint":"tcp://127.0.0.1:9001","token":"secret"}
```

Behavior:

- rebuilds `program.stbc` for the selected project
- reads the rebuilt bytecode from disk
- sends `bytecode.reload` to the runtime control endpoint from the project bundle or the explicit `endpoint` override

Returns a combined payload with the build report plus the control reload result.

Current scope:

- intended for same-project runtime iteration and agent repair loops
- source-aware attached-session reload workflows are still future work

### `harness.load`

Either load inline sources:

```json
{
  "inline_sources": [
    {"text":"PROGRAM Main\nEND_PROGRAM\n"}
  ]
}
```

Or load a project from the current workspace:

```json
{"project":"runtime-a"}
```

Or load explicit files relative to the workspace root:

```json
{"files":["src/main.st","src/tests.st"]}
```

### `harness.cycle`

```json
{"count":10,"dt_ms":10,"watch":["q","et"]}
```

`dt_ms` advances virtual time before each cycle. `watch` returns typed values
for the named outputs after the final cycle.

### `harness.set_input`

```json
{"name":"start_pb","value":{"type":"BOOL","value":true}}
```

### `harness.get_output`

```json
{"name":"motor_run"}
```

### `harness.advance_time`

```json
{"duration_ms":25}
```

### `harness.run_until`

```json
{
  "name":"q",
  "equals":{"type":"BOOL","value":true},
  "dt_ms":10,
  "max_cycles":5,
  "watch":["q","et"]
}
```

If the cycle budget is exhausted, the response uses error code `-32004` and
returns the expected value in the error data.

## Typed Values

Harness values use typed JSON payloads so agents do not need to guess IEC data
shapes. Examples:

```json
{"type":"BOOL","value":true}
{"type":"INT","value":7}
{"type":"TIME","nanos":30000000}
{"type":"STRING","value":"hello"}
{"type":"ARRAY","dimensions":[[0,1]],"elements":[{"type":"BOOL","value":true},{"type":"BOOL","value":false}]}
```

`trust-harness` uses the same value encoding. See
`docs/guides/TRUST_HARNESS_PROTOCOL.md`.

## Current Scope Boundary

The following are intentionally not in v1 yet:

- `lsp.code_actions`
- attached-session source-aware `runtime.reload`

Those stay pending until the code-action surface is extracted into a shared
service instead of living only inside the LSP handler stack, and until
attached-session reload semantics are lifted out of the VS Code-specific
runtime panel flow.
