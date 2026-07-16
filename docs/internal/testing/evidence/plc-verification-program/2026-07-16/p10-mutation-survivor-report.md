# Phase 10 Focused Mutation Program

Generator: `mutation-program-audit v1`
Source revision: `f71729e9ec9314de8dea09a7062d3546c81b546b`
Generated: `2026-07-16T16:12:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `fc05a0ce5262888a88a6dd51f0be93eeb8415ed2c59af3842cb3f5e7d4d6b815`
Input SHA-256: `sha256:577dcc780355a914e4b572dbcc625d264ae51b15a1ed9ce20e44966d9d58848b`

This report separates five validated measured shards from one planned connector
shard. It creates no proof, invariant coverage, spec-gap closure, release
evidence, product behavior, or CI enforcement change.

## Summary

- Shards: 6
- Measured shards: 5
- Planned shards: 1
- Defined mutants: 7
- Measured mutants: 6
- Caught: 6
- Survived: 0
- Unviable: 0
- Timeout: 0
- Error: 0
- Coverage runs: 0

## Shards

| Shard | Area | Status | Defined | Measured | Result artifact |
| --- | --- | --- | ---: | ---: | --- |
| `MUTATION_SHARD_BYTECODE_VALIDATOR_001` | `bytecode_vm` | `measured` | 2 | 2 | `docs/internal/testing/evidence/plc-verification-program/2026-07-08/p1b-bytecode-validator-mutation-report.json` (`sha256:88e97c57ed47e698f5c3c8d33611e3d8f0a4cdae0135ee84451bfa10c00713a2`) |
| `MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001` | `bytecode_vm` | `measured` | 1 | 1 | `docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-runtime-value-conversion-mutation.json` (`sha256:dcfe38dad50f4e2008bec764335ff920c63065731b2015d9f8d4e834ec637940`) |
| `MUTATION_SHARD_HIR_DIAGNOSTICS_001` | `compiler_iec` | `measured` | 1 | 1 | `docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-hir-subrange-diagnostics-mutation.json` (`sha256:d9a2c0af24cf9ec69b5a0c7f3109a0c02ed2152d27fc7e8fd5c69e2e71ee22d8`) |
| `MUTATION_SHARD_PARSER_RECOVERY_001` | `compiler_iec` | `measured` | 1 | 1 | `docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-parser-recovery-mutation.json` (`sha256:3330e2600c1ea938bf57ebf67b4126ae1372d4ad8447032cb46832dab3a55c8e`) |
| `MUTATION_SHARD_RETAIN_RESTART_001` | `runtime_safety` | `measured` | 1 | 1 | `docs/internal/testing/evidence/plc-verification-program/2026-07-16/p10-retain-restart-mutation.json` (`sha256:2d7d09ba12e20aa791c467fdce969c45dd7c25e6e75f8d2333df10680d30ca9c`) |
| `MUTATION_SHARD_CONNECTOR_STATUS_PROJECTION_001` | `protocols` | `planned` | 1 | 0 | none |

## Outcomes

| Shard | Mutant | Result |
| --- | --- | --- |
| `MUTATION_SHARD_BYTECODE_VALIDATOR_001` | `MUTANT_VALIDATE_INSTRUCTION_STREAM_BYPASS` | `caught` |
| `MUTATION_SHARD_BYTECODE_VALIDATOR_001` | `MUTANT_VALIDATE_STACK_SHAPE_BYPASS` | `caught` |
| `MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001` | `MUTANT_RUNTIME_CONVERT_VALUE_IDENTITY_COMPARISON` | `caught` |
| `MUTATION_SHARD_HIR_DIAGNOSTICS_001` | `MUTANT_HIR_SUBRANGE_DIAGNOSTIC_NOOP` | `caught` |
| `MUTATION_SHARD_PARSER_RECOVERY_001` | `MUTANT_PARSER_RECOVERY_EOF_COMPARISON` | `caught` |
| `MUTATION_SHARD_RETAIN_RESTART_001` | `MUTANT_RETAIN_ON_WARM_FALSE` | `caught` |

## Survivors

No survivors are present in the measured shards.

## Boundaries

- `report_creates_proof`: `false`
- `report_creates_invariant_coverage`: `false`
- `report_closes_spec_gaps`: `false`
- `report_is_release_evidence`: `false`
- `new_mutation_or_coverage_run_executed_by_report`: `false`
- `runtime_or_product_behavior_changed`: `false`
- `ci_enforcement_changed`: `false`

## Limitations

- The bytecode-validator pilot and four source-only shards are measured; the connector-projection shard remains planned with an empty result array.
- Report generation resolves selectors but executes no mutation or coverage command; source outcomes come only from separately committed clean-HEAD execution artifacts.
- Caught and survived are derived from raw build/test exit and timeout fields; infrastructure failures are errors and cannot count as caught or unviable.
- Associated scanner and case identities are traceability labels, not claims that a specific test or blocked case killed a mutant.
- Mutation and coverage results are test-adequacy signals, never release safety proof, invariant coverage, or spec-gap closure.
- A future measured connector-projection shard must bind a delivered artifact SHA-256 and direct execution confirmation.
- The implementation board is checked live but excluded from the digest because board and evidence closure follow report generation.
