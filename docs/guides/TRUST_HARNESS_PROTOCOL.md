# trust-harness Protocol

`trust-harness` is the canonical deterministic executor for fast ST automation
outside the full runtime lifecycle.

It is designed for:

- agent repair loops
- CI validation
- docs/examples that need executable behavior
- future website sandboxes and local-model evaluation loops

## Transport

- process-local only
- newline-delimited JSON over `stdin` / `stdout`
- one request per line, one response per line

Each response is either:

```json
{"ok":true,"protocol_version":2,"data":{...}}
```

or:

```json
{"ok":false,"protocol_version":2,"error":{"kind":"invalid_argument","message":"...","data":null}}
```

Malformed JSON, an unsupported command, or a failed command produces one error
response for that input line. The process then continues with the same session
and reads the next non-empty line. Process startup fails only when command-line
or protocol-version selection is invalid, or when the transport itself cannot
be read or written.

## Protocol Versions

Protocol version 2 is the default. Set
`TRUST_HARNESS_PROTOCOL_VERSION=1|2`, `--protocol-version 1|2`, or
`--protocol-version=1|2` to select explicitly. A command-line selection
overrides the environment. Any other argument, a missing command-line value,
or a version other than 1 or 2 fails process startup.

Every line response includes the selected `protocol_version`, including
errors. Version 2 reports each watched value independently:

```json
{
  "values": {
    "ready": {"status":"ok","value":{"type":"BOOL","value":true}},
    "missing": {
      "status":"error",
      "code":"unresolved_name",
      "message":"boundary path 'missing' did not resolve to a declared value",
      "path":"missing",
      "candidates":[]
    }
  }
}
```

Version 1 preserves the legacy `name -> typed value` watch shape. Because that
shape cannot represent an entry error, any failed watch promotes the complete
request to a boundary error response.

## Commands

| Command | Purpose |
| --- | --- |
| `load` | Load one or more ST sources into a fresh harness |
| `reload` | Reload source(s) while preserving retain semantics where supported |
| `cycle` | Execute one or more cycles |
| `set_input` | Set a named input/global/program variable |
| `get_output` | Read a named output/global/program variable |
| `set_access` | Write a `VAR_ACCESS` binding |
| `get_access` | Read a `VAR_ACCESS` binding |
| `bind_direct` | Bind a named variable to a direct I/O address |
| `set_direct_input` | Write to a direct input address |
| `get_direct_output` | Read from a direct output address |
| `advance_time` | Advance virtual time without executing a cycle |
| `run_until` | Cycle until a named output matches an expected value |
| `restart` | Restart the harness runtime (`cold` or `warm`) |
| `snapshot` | Return watched values without executing more work |

## Source Loading

Single-source load:

```json
{"cmd":"load","source":"PROGRAM Main\nEND_PROGRAM\n"}
```

Multi-source load:

```json
{"cmd":"load","sources":["PROGRAM Main\nEND_PROGRAM\n","FUNCTION_BLOCK Fb\nEND_FUNCTION_BLOCK\n"]}
```

`load` performs an initial cycle and fails if that first cycle reports runtime
errors.

`reload` uses the same `source` / `sources` parameters.

`sources` must contain at least one source. When both `source` and `sources`
are present, `sources` is authoritative. An explicitly empty authoritative
`sources` list is an `invalid_argument` error; omitting both fields is an
`invalid_request` error. A successful `load` replaces the session with a fresh
harness only after compilation and its initial cycle succeed. A failed load
preserves the previous loaded session, or leaves the session unloaded if none
existed.

`reload` requires a loaded session. It preserves supported retained values,
virtual time, and cycle count. A compilation or retained-state migration
failure preserves the complete previous session.

## Cycle Control

Advance ten cycles while moving virtual time forward by `10 ms` each cycle:

```json
{"cmd":"cycle","count":10,"dt_ms":10,"watch":["q","et"]}
```

Advance virtual time only:

```json
{"cmd":"advance_time","duration_ms":25}
```

Take a passive snapshot:

```json
{"cmd":"snapshot","watch":["motor_run","fault","et"]}
```

`cycle` defaults to `count: 1`, `dt_ms: 0`, and an empty watch list.
`advance_time` accepts `duration_ms`; the legacy `dt_ms` spelling remains an
alias, and `duration_ms` wins when both are present. `snapshot` does not run a
cycle or advance virtual time. `run_until` checks the current value before its
first cycle, so an already-matching value reports `cycles_ran: 0`. Earlier
completed cycles and time advances remain committed if a later requested cycle
or bounded run fails. Negative time increments return `invalid_argument`.

## I/O Manipulation

Set input:

```json
{"cmd":"set_input","name":"start_pb","value":{"type":"BOOL","value":true}}
```

Get output:

```json
{"cmd":"get_output","name":"motor_run"}
```

Write/read `VAR_ACCESS`:

```json
{"cmd":"set_access","name":"RemoteSpeed","value":{"type":"INT","value":42}}
{"cmd":"get_access","name":"RemoteSpeed"}
```

Bind/read direct I/O:

```json
{"cmd":"bind_direct","name":"start_pb","address":"%IX0.0"}
{"cmd":"set_direct_input","address":"%IX0.0","value":{"type":"BOOL","value":true}}
{"cmd":"get_direct_output","address":"%QX0.0"}
```

## Bounded Run Loop

```json
{
  "cmd":"run_until",
  "name":"q",
  "equals":{"type":"BOOL","value":true},
  "dt_ms":10,
  "max_cycles":5,
  "watch":["q","et"]
}
```

If `max_cycles` is exceeded, the protocol returns:

```json
{
  "ok": false,
  "error": {
    "kind": "run_until_timeout",
    "message": "run_until exceeded 5 cycles before 'q' matched the expected value",
    "data": {
      "name": "q",
      "max_cycles": 5,
      "expected": {"type":"BOOL","value":true}
    }
  }
}
```

## Restart

```json
{"cmd":"restart","mode":"cold"}
{"cmd":"restart","mode":"warm"}
```

The mode defaults to `cold` and is ASCII case-insensitive. `warm` preserves
variables declared `RETAIN`; `cold` restores their declared initial values.
Unsupported modes return `invalid_request`.

## Typed Value Format

The protocol uses a stable typed JSON shape.

Common scalar examples:

```json
{"type":"BOOL","value":true}
{"type":"INT","value":7}
{"type":"DINT","value":42}
{"type":"REAL","value":1.5}
{"type":"TIME","nanos":30000000}
{"type":"STRING","value":"hello"}
```

Structured examples:

```json
{"type":"ARRAY","dimensions":[[0,1]],"elements":[{"type":"BOOL","value":true},{"type":"BOOL","value":false}]}
{"type":"STRUCT","type_name":"MyStruct","fields":{"enabled":{"type":"BOOL","value":true}}}
{"type":"ENUM","type_name":"Mode","variant":"Auto","numeric":1}
{"type":"NULL"}
```

## Error Kinds

Current stable `error.kind` values:

- `invalid_request`
- `invalid_argument`
- `not_loaded`
- `compile_error`
- `runtime_error`
- `runtime_cycle_error`
- `run_until_timeout`

Boundary failures use their stable boundary code as `error.kind`:

- `unresolved_name`
- `unbound_program`
- `ambiguous_name`
- `unsupported_path_syntax`
- `wrong_kind`
- `undeclared_binding`
- `internal_lock_failure`
- `internal_failure`

Boundary error data contains `path` when one exists and a deterministic
`candidates` array. `runtime_cycle_error` data contains the ordered runtime
error strings. `run_until_timeout` data contains the target name, cycle
budget, and expected typed value. Missing request fields, unsupported commands,
and unsupported restart modes are structural `invalid_request` errors;
semantic value and non-negative-duration validation uses `invalid_argument`.

## Current Observability Scope

The protocol currently returns watched value snapshots. Full trace streaming is
still deferred, but the watch output is already enough for:

- timer/state assertions
- small repair loops
- deterministic evidence capture in docs and CI

That keeps the surface small while still making the harness useful today.
