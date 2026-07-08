# Rust PLC CLI And JSON Contract

**Status:** implementation contract, v1 (2026-07-03).
**Master:** `rust-support-architecture-spec-v1.md`.
**Applies to:** `trust check`, `trust build`, `trust replay`, admission JSON,
crate verdict JSON, VS Code Problems/report ingestion.

## 1. Rule

Any CLI output consumed by VS Code, CI dashboards, acceptance tools, or public
reports must be versioned JSON. Human text may exist, but no implementation
may parse it.

## 2. Envelope

Every JSON command emits:

```json
{
  "schema": "trust.rust_plc.check",
  "schema_version": 1,
  "tool_version": "0.0.0",
  "project": {
    "root": "/abs/path",
    "kind": "rust-plc",
    "profile": "development"
  },
  "started_at_unix_ms": 0,
  "finished_at_unix_ms": 0,
  "status": "ok",
  "diagnostics": []
}
```

Allowed status values: `ok`, `failed`, `refused`, `partial`, `internal_error`.
`partial` is never a deployable/admitted success. It means the command
completed enough to return diagnostics but at least one mandatory section is
unavailable; it exits non-zero and must include a diagnostic naming the
missing section.

## 3. Diagnostics

Diagnostic object:

```json
{
  "code": "F16",
  "severity": "error",
  "message": "worst frame exceeds base scan interval",
  "claim_grade": "admitted",
  "primary_location": {
    "file": "trust.toml",
    "line": 12,
    "column": 1
  },
  "related_locations": [],
  "fixes": [
    {
      "title": "Open admission report",
      "kind": "show_report",
      "command": "trust timing report --last"
    }
  ],
  "details": {}
}
```

Rules:

- `code` is stable and maps to RS/F rows where possible.
- `primary_location` is required for user-actionable diagnostics.
- `fixes` name actions; actions may be disabled by the UI if the backend is
  absent.
- Messages use claim vocabulary.
- JSON contains enough structure for Problems, reports, and tests.

## 4. `trust check --json`

Sections:

```json
{
  "generate": {},
  "cargo": {},
  "st_compile": {},
  "digests": {},
  "admission": {},
  "crates": {},
  "artifacts": {}
}
```

Semantics:

- runs generation into a deterministic staging area;
- checks generated artifact drift;
- runs cargo check/build as required by profile;
- checks generated ST;
- runs development admission dry-run;
- runs crate policy;
- emits report artifacts under `target/trust/<profile>/`;
- exits non-zero on `failed` or `refused`.

## 5. `trust build --json`

Includes everything from check plus:

- bundle identity;
- target triple;
- signed artifact status;
- `.trusttime` and `.trustcrate` freshness;
- deploy preflight status.

Production build refuses missing evidence grades required by profile.

## 6. Admission JSON

Admission object:

```json
{
  "verdict": "admitted",
  "profile": "development",
  "base_frame_us": 1000,
  "hyperperiod_us": 10000,
  "evidence_floor": "declared",
  "worst_frame": {
    "index": 0,
    "release_time_us": 0,
    "total_us": 683,
    "reserve_us": 317,
    "coincident_tasks": ["fast", "main"],
    "contributors": []
  },
  "frames": [],
  "record_path": "target/trust/development/app.trusttime"
}
```

Contributor object:

- id;
- label;
- source location;
- budget_us;
- measured_us if available;
- evidence grade;
- kind: task, input, output, retain, recorder, service, overhead, margin.

## 7. Crate Verdict JSON

Crate verdict:

```json
{
  "crate": "reqwest",
  "version": "0.12.0",
  "verdict": "refused",
  "evidence": "classified",
  "reason": "service-only API reached from scan path",
  "path": ["palletizer::cycle", "reqwest::Client::send"],
  "location": {"file": "Cargo.toml", "line": 14, "column": 1}
}
```

## 8. `trust replay --json`

Replay output:

- trace identity;
- build identity;
- verdict: pass/diverged/internal_error;
- first divergence cycle;
- variable diffs with expected/actual/tolerance;
- overrun explanation links;
- shrunk trace path if generated.

## 9. Instance Snapshot JSON

The instance snapshot rides the existing debug/control pipeline, not the CLI,
but uses the same schema discipline:

- schema name and version;
- snapshot cycle;
- freshness;
- tasks;
- instances;
- fields;
- state/fault/timing/sequence metadata.

The shape is detailed in `rust-plc-vscode-workflow-v1.md` section 8 and must be
frozen before S14.

## 10. Exit Codes

- `0`: `ok`
- `1`: user-correctable failed/refused result
- `2`: invalid CLI invocation
- `3`: internal tool error
- `4`: environment/bootstrap missing
- `5`: partial result; mandatory section missing

VS Code must not infer semantics from stderr text.

## 11. Tests

- JSON schema snapshots for ok, failed, refused, partial, internal_error.
- Golden JSON for F16, F17, F22, F23.
- Roundtrip parsing tests in Rust.
- TypeScript parser/model tests consuming fixture JSON verbatim.
- Backward-compatibility test for previous schema minor versions once v2
  exists.
