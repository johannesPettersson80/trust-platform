# Phase 10 Source-Mutation Execution

Execution source revision: `56f68f2bbdb12c655f681668c5a5fddda4f4d659`

Environment: `trust-builder`, isolated clone
`~/projects/trust-platform-mutation-execution`, cargo-mutants 27.0.0, shared
generated target `~/.cache/codex-targets/trust-platform-mutation`.

## Scope

This execution measured the four reviewed source-only shards. The delivered
connector-projection shard was not run. Association IDs remain traceability
labels and do not claim that a specific associated test killed a mutant. These
artifacts create no proof, invariant promotion, spec-gap closure, release
evidence, product behavior change, or CI enforcement.

Each accepted run started from the clean execution revision, created an
isolated `git archive`, cleaned only the selected package, passed the exact
focused baseline, applied one exact generated mutant, built it, ran the exact
focused test, restored the source, and cleaned the package again.

## Commands

The following command shape was run once for each shard, using the shard's
reserved JSON path:

```text
python3 scripts/run_focused_mutation_shard.py \
  --shard-id <SHARD_ID> \
  --json-out <RESERVED_RESULT_ARTIFACT_PATH> \
  --target-dir "$HOME/.cache/codex-targets/trust-platform-mutation"
```

Measured shards:

- `MUTATION_SHARD_RUNTIME_VALUE_CONVERSION_001`
- `MUTATION_SHARD_HIR_DIAGNOSTICS_001`
- `MUTATION_SHARD_PARSER_RECOVERY_001`
- `MUTATION_SHARD_RETAIN_RESTART_001`

## Results

| Shard | Result | Artifact SHA-256 |
| --- | --- | --- |
| runtime value conversion | 1 caught, 0 survived, 0 unviable, 0 timeout, 0 error | `dcfe38dad50f4e2008bec764335ff920c63065731b2015d9f8d4e834ec637940` |
| HIR subrange diagnostics | 1 caught, 0 survived, 0 unviable, 0 timeout, 0 error | `d9a2c0af24cf9ec69b5a0c7f3109a0c02ed2152d27fc7e8fd5c69e2e71ee22d8` |
| parser recovery | 1 caught, 0 survived, 0 unviable, 0 timeout, 0 error | `3330e2600c1ea938bf57ebf67b4126ae1372d4ad8447032cb46832dab3a55c8e` |
| retain/restart | 1 caught, 0 survived, 0 unviable, 0 timeout, 0 error | `2d7d09ba12e20aa791c467fdce969c45dd7c25e6e75f8d2333df10680d30ca9c` |

No survivor required test strengthening, and no current-product baseline failed.
Therefore this batch found no product bug and made no product change.

The generic Phase 10 report was regenerated from clean source commit
`81f475234f199a5299f7442c8314e0b6f1d30696` at
`2026-07-16T10:18:30Z`. Its JSON SHA-256 is
`e893d67e1514c6bb5e117e2e57e5a391e01fb9cec4f21e5b829f90578fa3770c`;
the report independently records five measured shards, one planned shard, six
caught mutants, and zero survivor, unviable, timeout, or error outcomes.

## Discarded Attempts

The first operational attempt was invalidated before producing evidence because
two runner processes overlapped on the same generated target. Both process
trees were stopped, the generated target was deleted, and the clean clone was
verified before any accepted run.

The first subsequent clean conversion attempt measured the originally reviewed
`apply_conversion -> Ok(Default::default())` selector as unviable because
`Value` does not implement `Default`. Its canonical diagnostic artifact is
retained as
`p10-runtime-value-conversion-mutation-attempt-01-unviable.json`. The manifest
was changed tests-first to the generated, compiling `convert_value` identity
comparison selector; all four accepted artifacts were then produced from the
same clean descendant revision shown above.
