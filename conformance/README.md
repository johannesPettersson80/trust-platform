# Conformance Suite

This directory defines the deterministic conformance suite contract for
`trust-platform` and external runtime/tool comparisons.

The original Deliverable 1 MVP remains available as the v1 contract. The
expanded v2 contract is the current flagship suite used by CI for public
language/runtime proof.

## Contract Versions

### v1 Frozen Categories

The v1 contract is frozen to the six Deliverable 1 categories:

1. `timers`
2. `edges`
3. `scan_cycle`
4. `init_reset`
5. `arithmetic`
6. `memory_map`

Suites containing only these categories continue to emit
`trust-conformance-v1` summaries compatible with
`conformance/schemas/summary-v1.schema.json`.

### v2 Expanded Categories

The v2 suite extends v1 with:

7. `strings`
8. `arrays`
9. `structs`
10. `enums`
11. `nested_values`
12. `oop_dispatch`
13. `references`
14. `retain_matrix`
15. `scheduler`
16. `comms_determinism`

Suites containing any expanded category emit `trust-conformance-v2` summaries
compatible with `conformance/schemas/summary-v2.schema.json`.

## Repository Layout

```text
conformance/
  README.md
  contract.md
  naming.md
  schemas/
    summary-v1.schema.json
    summary-v2.schema.json
  cases/
    <category>/
      <case_id>/
        program.st
        manifest.toml
  expected/
    <category>/
      <case_id>.json
  reports/
    .gitkeep
```

Generated reports are CI/local artifacts and are not committed under
`conformance/reports/`.

## Determinism Contract

- Case execution order is lexicographic by `case_id`.
- Inputs and expected outputs are versioned in-repo.
- A case only passes when observed results match expected artifacts exactly.
- Output summaries must comply with the schema matching the emitted
  `version`/`profile`.
- Communication determinism cases use simulated or loopback state transitions
  only. They must not depend on live sockets, brokers, PLCs, or fieldbus
  hardware.

## Documents

- Contract: `conformance/contract.md`
- Naming rules: `conformance/naming.md`
- Summary schemas: `conformance/schemas/summary-v1.schema.json`,
  `conformance/schemas/summary-v2.schema.json`
- Failure taxonomy: `conformance/failure-taxonomy.md`
- External run guide: `conformance/external-run-guide.md`
- Known gaps: `conformance/known-gaps.md`
- External submission process: `conformance/submissions.md`

## Running The Suite

Generate or refresh expected artifacts:

```bash
trust-runtime conformance --suite-root conformance --update-expected
```

Run verification against versioned expected artifacts:

```bash
trust-runtime conformance --suite-root conformance
```

Optional output override:

```bash
trust-runtime conformance --suite-root conformance --output target/conformance/local-summary.json
```

Runner exits non-zero when any case is `failed` or `error`.

CI gate uses repeated runs and normalized summary comparison to verify
deterministic ordering/status behavior.
