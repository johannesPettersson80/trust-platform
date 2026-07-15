# Phase 10 Focused Mutation Program

Generator: `mutation-program-audit v1`
Source revision: `df6c89e6425f93acc2949fc71cd8cce4d0d08435`
Generated: `2026-07-15T19:14:22+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `47a8b02dd58b91460bf6c93e91f24ad689081d69b0b903587106587fcedc2bb5`
Input SHA-256: `sha256:2be9e9cf3b1ddc6f7fcfecf27e52648e7a3ae2a05f3e15c672cc9a458aefe9c2`

This report separates one validated measured pilot from five planned focused
shards. It creates no proof, invariant coverage, spec-gap closure, release
evidence, product behavior, or CI enforcement change.

## Summary

- Shards: 6
- Measured shards: 1
- Planned shards: 5
- Defined mutants: 7
- Measured mutants: 2
- Caught: 2
- Survived: 0
- Unviable: 0
- Timeout: 0
- Error: 0
- Coverage runs: 0

## Shards

| Shard | Area | Status | Defined | Measured | Result artifact |
| --- | --- | --- | ---: | ---: | --- |
| `MUTATION_SHARD_BYTECODE_VALIDATOR_001` | `bytecode_vm` | `measured` | 2 | 2 | `docs/internal/testing/evidence/plc-verification-program/2026-07-08/p1b-bytecode-validator-mutation-report.json` (`sha256:329a2d46a14eb0c1f29c9dfac6f94ba112529a0daecb3b48f47038f429db9dc8`) |
| `MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001` | `bytecode_vm` | `planned` | 1 | 0 | none |
| `MUTATION_SHARD_HIR_DIAGNOSTICS_001` | `compiler_iec` | `planned` | 1 | 0 | none |
| `MUTATION_SHARD_PARSER_RECOVERY_001` | `compiler_iec` | `planned` | 1 | 0 | none |
| `MUTATION_SHARD_RETAIN_RESTART_001` | `runtime_safety` | `planned` | 1 | 0 | none |
| `MUTATION_SHARD_CONNECTOR_STATUS_PROJECTION_001` | `protocols` | `planned` | 1 | 0 | none |

## Outcomes

| Shard | Mutant | Result |
| --- | --- | --- |
| `MUTATION_SHARD_BYTECODE_VALIDATOR_001` | `MUTANT_VALIDATE_INSTRUCTION_STREAM_BYPASS` | `caught` |
| `MUTATION_SHARD_BYTECODE_VALIDATOR_001` | `MUTANT_VALIDATE_STACK_SHAPE_BYPASS` | `caught` |

## Survivors

No survivors are present in the measured pilot.

## Boundaries

- `report_creates_proof`: `false`
- `report_creates_invariant_coverage`: `false`
- `report_closes_spec_gaps`: `false`
- `report_is_release_evidence`: `false`
- `new_mutation_or_coverage_run_executed_by_report`: `false`
- `runtime_or_product_behavior_changed`: `false`
- `ci_enforcement_changed`: `false`

## Limitations

- Only the existing bytecode-validator pilot is measured; five other focused shards are definitions with empty result arrays.
- Cargo-mutants single-file listing resolves each selector but does not execute a baseline, build, test, mutation, or coverage command.
- Caught and survived are derived from raw build/test exit and timeout fields; infrastructure failures are errors and cannot count as caught or unviable.
- Associated scanner and case identities are traceability labels, not claims that a specific test or blocked case killed a mutant.
- Mutation and coverage results are test-adequacy signals, never release safety proof, invariant coverage, or spec-gap closure.
- A future measured connector-projection shard must bind a delivered artifact SHA-256 and direct execution confirmation.
- The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.
