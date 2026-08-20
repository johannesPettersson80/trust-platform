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

Each non-empty input line is one complete request. The top-level value MUST be
an object with `jsonrpc` exactly equal to `"2.0"`, a non-empty string `method`,
and an explicit `id` whose value is a string or a JSON number. Notifications
(requests with no `id`) and `null`, Boolean, object, or array identifiers are
not supported by v1 and return `-32600` with a `null` response ID. `params`,
when present, MUST be an object or array. A scalar `params` value is an invalid
request envelope rather than method-specific input.

Invalid JSON returns one parse-error response with code `-32700` and a `null`
ID. A syntactically valid but invalid envelope returns `-32600`; it MUST NOT be
misreported as a parse error. Blank lines are ignored. Unknown top-level
members are ignored for JSON-RPC interoperability.

## Response Envelope

Success:

```json
{"jsonrpc":"2.0","id":1,"result":{"transport":"stdio"}}
```

Failure:

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method 'missing' is not available."}}
```

Every accepted request line produces exactly one response line with the same
ID. A response contains exactly one of `result` or `error`, never both and
never neither. Error responses preserve the original string or numeric ID
whenever the envelope supplied a valid one.

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

Error `data`, when present, is machine-readable. Workspace containment errors
include `kind = "path_outside_workspace"` and the submitted path. I/O errors
include the normalized in-workspace path when one exists. Harness boundary
errors preserve their stable boundary kind, path, and candidate list. Secrets,
including runtime control tokens and source contents unrelated to the failed
operation, MUST NOT be copied into an error message or error data.

## Current Methods

| Method | Purpose |
| --- | --- |
| `agent.describe` | Return transport info, workspace root, and supported method list |
| `workspace.read` | Read a file inside the workspace root |
| `workspace.write` | Write a file inside the workspace root |
| `workspace.project_info` | Return project-root, source-root, dependency, config, and runtime-orientation metadata |
| `lsp.diagnostics` | Return machine-readable diagnostics for one ST file or a whole project source tree |
| `lsp.ast_canonicalize` | Return the canonical normalized AST token stream and 5-gram fingerprints for one source |
| `lsp.ast_similarity` | Compare two canonical AST token streams and return their similarity evidence |
| `lsp.format` | Return a formatting preview for one ST file without mutating disk |
| `runtime.build` | Reuse the `trust-runtime build --ci` payload path |
| `runtime.compile_reload` | Run the full diagnose -> build -> reload loop and return a single structured result |
| `runtime.validate` | Reuse the `trust-runtime validate --ci` payload path |
| `runtime.test` | Reuse the `trust-dev test --output json` payload path |
| `runtime.reload` | Rebuild `program.stbc` and issue `bytecode.reload` to a running runtime control endpoint |
| `harness.load` | Load inline/project sources into a deterministic in-process harness |
| `harness.reload` | Reload harness sources while preserving retain semantics when supported |
| `harness.execute` | Run one isolated deterministic fixture transaction and return structured step/assertion evidence |
| `harness.cycle` | Advance cycles, optionally advancing virtual time first |
| `harness.set_input` | Set a harness input/global/program variable |
| `harness.get_output` | Read a harness output/global/program variable |
| `harness.advance_time` | Advance virtual time without executing a cycle |
| `harness.run_until` | Cycle until a named output matches an expected value |

`agent.describe.methods` is the authoritative, duplicate-free list of methods
accepted by this version. Every listed method MUST have a dispatch route, and
no accepted public method may be omitted from the list. The response also
reports the canonical workspace root, `stdio` transport, and `jsonl` framing.

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

`bytes_written` is the UTF-8 byte length, not the Unicode scalar or display
character count. When `create_parents` is false, a missing parent is an I/O
error and no file or directory is created.

### Workspace filesystem containment

All workspace, project, source-root, and explicit harness-file paths pass two
independent gates before any read, write, source collection, build, validation,
formatting, reload, or harness load:

1. lexical normalization rejects absolute paths, platform drive/UNC roots,
   empty paths, and any parent traversal that escapes the submitted relative
   path; and
2. filesystem resolution confirms that the target, or the nearest existing
   ancestor for a target that does not yet exist, remains below the canonical
   workspace root.

Symbolic links, junctions, reparse points, or other aliasing components MUST
NOT permit access outside the canonical workspace root. For writes with parent
creation enabled, containment is checked before creating any directory. A
containment rejection uses `-32001` and performs no filesystem mutation.

Project paths MUST resolve to an existing directory inside the workspace.
Project-relative source roots and file paths are resolved relative to that
canonical project directory and must remain inside both the project and the
workspace. A symlinked project or source path that escapes either boundary is
rejected rather than treated as a valid project.

Successful path-bearing results use normalized forward-slash relative paths
for machine portability. An I/O failure does not silently return an empty file,
an empty project, or a successful zero-byte write.

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

Whole-project discovery is recursive and literal: metacharacters in a valid
project or source-root name do not acquire glob meaning. The two supported
extensions are matched case-insensitively for every ASCII spelling, only
regular files are analyzed, and the normalized project-relative source list is
deduplicated and ordered lexicographically. A source alias that resolves
outside the canonical project root is rejected rather than analyzed.

Returns:

- `target`
- `errors`
- `warnings`
- `issues[]` with `path`, absolute `file`, `line`, `column`, `severity`,
  `message`, optional `code`, and optional related entries

Every issue uses one-based line and column coordinates while `span.start` and
`span.end` remain zero-based UTF-8 byte offsets. Related entries use the same
coordinate convention. `errors` and `warnings` are the exact counts of issues
whose normalized severity is `error` and `warning`; other severities remain in
`issues` but do not inflate either counter. Aggregate issues are sorted
deterministically by file, position, severity, code, and message. An inline
`content` override is analyzed without writing it to disk.

### `lsp.ast_canonicalize`

Optional params:

```json
{"project":"runtime-a","path":"src/main.st","content":"PROGRAM Main\nEND_PROGRAM\n"}
```

Exactly one effective source is required. Inline `content` takes precedence
over disk, including when the named file does not exist; otherwise `path` is
read relative to the selected project. Supplying neither is an error. The
operation never writes the source.

The result contains `algorithm`, `gramSize`, `parseErrorCount`, `stream`, and
`fiveGrams`. Version 1 uses
`canonical_ast_jaccard_5gram_v1` with a gram size of five. Trivia, identifier
spellings, and literal values do not enter the canonical token stream.
`fiveGrams` is the sorted, duplicate-free set of canonical five-token windows,
and parse errors remain visible through `parseErrorCount` rather than silently
changing the algorithm.

### `lsp.ast_similarity`

Optional params:

```json
{
  "project":"runtime-a",
  "left_path":"src/before.st",
  "right_content":"PROGRAM After\nEND_PROGRAM\n"
}
```

The left and right inputs independently follow the canonicalization source
selection rule: inline content wins, otherwise that side requires a
project-relative path. The result reports the same algorithm and gram size,
the Jaccard `score`, unique `sharedGrams`, `leftGrams`, and `rightGrams`, plus
strict `threshold070` and `threshold095` decisions. A score exactly equal to a
threshold does not clear it. Two inputs with no five-grams have score `1.0`;
the operation does not mutate either file.

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

Inline content takes precedence over the file on disk and is never persisted.
When content is omitted, the selected file must exist and be valid UTF-8.
`changed` is true exactly when returned content differs byte-for-byte from the
effective input. The returned `path` is normalized and project-relative;
`file` identifies its absolute project location.

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

Source selection is unambiguous and fail-closed:

- `inline_sources`, `files`, and `project` are mutually exclusive selectors;
- an explicit inline or file list must be non-empty;
- every inline source must contain non-empty source text;
- explicit file paths must be unique after normalization and must pass the
  workspace containment rules; and
- when no selector is supplied, the configured agent workspace is the project.

`harness.load` replaces the stateful harness session. `harness.reload` requires
an already loaded stateful session and uses the selected replacement sources.
Load or reload failure leaves no partially replaced session.

### `harness.execute`

`harness.execute` is an isolated fixture transaction. It accepts the same
source selector as `harness.load`, plus ordered `steps`, ordered `assertions`,
and a final `watch` list:

```json
{
  "inline_sources": [
    {"path":"main.st","text":"PROGRAM Main\nEND_PROGRAM\n"}
  ],
  "steps": [
    {"op":"set_input","name":"start","value":{"type":"BOOL","value":true}},
    {"op":"cycle","count":1,"dt_ms":10}
  ],
  "assertions": [
    {"kind":"output_equals","name":"running","equals":{"type":"BOOL","value":true}}
  ],
  "watch":["running"]
}
```

Every execute request creates a fresh harness. It neither reads nor mutates the
stateful session owned by `harness.load`, `harness.reload`, and the individual
harness methods. The harness performs its documented initial cycle during
load; `stepsRun` counts only explicitly requested steps.

Supported steps are `set_input`, `set_access`, `bind_direct`,
`set_direct_input`, `advance_time`, `cycle`, `run_until`, and `restart`.
Steps execute in request order. `cycle.count` defaults to one. Restart mode
defaults to `cold`; accepted spellings are case-insensitive `cold` and `warm`.
An unknown operation, malformed typed value, negative duration, zero or
otherwise invalid execution bound, or unsupported restart mode is invalid
params (`-32602`) before any later step executes.

Supported assertions are `output_equals`, `access_equals`, and
`direct_output_equals`. Assertions execute in request order after all steps.
A value mismatch is an `assertion_failed` result entry with typed `expected`
and `actual` values. A boundary-resolution error is a structured failure entry
with the stable boundary kind, path, and candidates; it is not converted into
a successful assertion and does not erase earlier assertion results.

The method returns a JSON-RPC success envelope whenever it produced a complete
fixture result, even when that result has `status = "fail"`. The result
contains:

- `sourceCount`, `stepsRun`, `status`, and `passed`;
- assertion `total`, `evaluated`, `passed`, and `failed`, satisfying
  `evaluated = passed + failed` and never exceeding `total`;
- a final typed `watchSnapshot`; and
- ordered failure entries identifying compile, runtime-cycle, step,
  run-until-timeout, boundary, or assertion failures.

A compile failure occurs before any step and reports `stepsRun = 0`. A step
failure stops later steps and assertions, retains the zero-based `stepIndex`
and failing step, and returns the best available watch snapshot. A run-until
timeout preserves the typed expected value and the last readable actual value.
Assertion mismatches do not prevent later independent assertions from being
evaluated. `passed` is true exactly when the failure list is empty; successful
results omit `failures`.

Malformed request structure and invalid argument values use JSON-RPC
`-32602`. Compile/runtime/boundary outcomes reached by a valid fixture request
are represented inside the result so batch callers can distinguish a failed
fixture from a broken protocol call.

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
