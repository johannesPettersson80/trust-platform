# Phase 10 Focused Mutation Program

Generator: `mutation-program-audit v1`
Source revision: `de94ec228fe9ff07f015e9395a39d7282c37371b`
Generated: `2026-07-15T20:06:12+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `9903193c160f9014895160c3ba7c8ab158b7c90662d21a964660fb9e251a4157`
Input SHA-256: `sha256:963803b185a0e27650c064f6ab1719593deb9c47325614acee3edb46cdfc0e00`

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
