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
{"ok":true,"data":{...}}
```

or:

```json
{"ok":false,"error":{"kind":"invalid_argument","message":"...","data":null}}
```

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

## Current Observability Scope

The protocol currently returns watched value snapshots. Full trace streaming is
still deferred, but the watch output is already enough for:

- timer/state assertions
- small repair loops
- deterministic evidence capture in docs and CI

That keeps the surface small while still making the harness useful today.
