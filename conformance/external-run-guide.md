# External Conformance Run Guide

This guide describes how to run the same conformance suite against another
runtime/tool and submit comparable results.

## What To Reuse

Use these versioned artifacts directly:

- `conformance/cases/`
- `conformance/naming.md`
- `conformance/contract.md`
- `conformance/schemas/summary-v1.schema.json`
- `conformance/schemas/summary-v2.schema.json`

Expected artifacts in `conformance/expected/` define `trust-runtime` baseline
behavior for this suite revision.

## Adapter Workflow

1. Implement an adapter that can:
   - read each `manifest.toml` and `program.st`
   - execute cycles and restart directives deterministically
   - capture watched globals/direct addresses per cycle
2. Emit per-case actual artifacts compatible with conformance contract.
3. Emit one summary JSON file compatible with the schema for the emitted
   `version`/`profile`.

## Minimum Output Contract

Your summary JSON must include:

- `version = 1`, `profile = "trust-conformance-v1"` for the frozen v1
  category set.
- `version = 2`, `profile = "trust-conformance-v2"` when any expanded v2
  category is included.
- deterministic `results` ordering by `case_id` ascending
- `status` per case (`passed`, `failed`, `error`, `skipped`)
- failure `reason.code` from taxonomy in `conformance/failure-taxonomy.md`

## Comparison Strategy

- Compare your per-case artifacts to the baseline expected artifacts:
  `conformance/expected/<category>/<case_id>.json`
- Record mismatches as `failed`.
- Record adapter/runtime execution failures as `error`.

## Validation

Validate your summary against the schemas:

```bash
python3 scripts/validate_conformance_summary_schema.py \
  --schema conformance/schemas/summary-v1.schema.json \
  --schema conformance/schemas/summary-v2.schema.json \
  --summary your-summary.json
```

Any external JSON Schema validator may also be used against the matching
`summary-v*.schema.json` file.

## Submit Results

Follow `conformance/submissions.md`.
