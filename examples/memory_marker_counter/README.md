# Memory Marker Counter Fixture

This fixture validates `%M` marker synchronization in the runtime cycle:

- cycle start: `%M` -> bound variables
- cycle end: bound variables -> `%M`

## Files

- `src/Main.st`
- `src/Configuration.st`

## Manual UI Check (Runtime Panel)

1. Open this folder in the VS Code extension.
2. Start the runtime from the Runtime panel.
3. In `I/O`, set memory `%MW0` to `Word(41)` and write it.
4. Let one cycle execute.

Expected:
- `%QW0` becomes `Word(41)` (latched value read at cycle start).
- `%MW0` becomes `Word(42)` (counter written back at cycle end).
- On subsequent cycles `%MW0` keeps increasing by `1`.

## Automated Check

Run:

```bash
./scripts/test_memory_marker_sync.sh
```

