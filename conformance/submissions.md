# External Result Submissions

Community result submissions are open for v1 and v2 conformance.

## Submit

1. Run your adapter/runtime against `conformance/cases/`.
2. Produce:
   - summary JSON (`summary-v1` or `summary-v2` contract)
   - per-case artifacts for any `failed`/`error` case
   - runtime/tool metadata (name, version, target)
3. Open a GitHub issue with:
   - title: `Conformance submission: <runtime-name> <version>`
   - summary JSON attached
   - mismatch notes for failed cases

## Required Metadata

- Runtime/tool name
- Runtime/tool version
- Target OS/architecture
- Adapter commit hash (if custom adapter was used)
- Date of run (UTC)
- Conformance profile (`trust-conformance-v1` or `trust-conformance-v2`)

## Triage

Submissions are reviewed for:

- schema compatibility
- deterministic ordering
- reproducibility notes
- clear failure taxonomy usage
