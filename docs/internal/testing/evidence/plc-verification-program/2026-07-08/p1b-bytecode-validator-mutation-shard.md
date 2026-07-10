# P1B Bytecode-Validator Mutation Shard

Date: 2026-07-09; refreshed 2026-07-10 after the Phase 4 metadata migration

Implemented row:

- `VERIF-P1B-013`: first bytecode-validator-only mutation shard, with survivor
  reporting against committed case IDs.
- This satisfies only the bytecode-validator slice of `VERIF-P10-001`.
  `VERIF-P10-001`, `VERIF-P1B-012`, and `VERIF-P1B-014` remain open.

## Scope And Method

- `cargo-mutants 27.0.0` package discovery returned zero candidates for the
  validator because its implementation is assembled from `include!()` files.
  The shard therefore uses cargo-mutants single-file candidate generation and
  applies only two selected function-bypass mutants in an isolated archive of
  commit `3bf92dd9a4c373cc988d0836ace51366f1c34bb2`.
- The runner cleans only `trust-runtime` outputs in the dedicated mutation
  target before baseline, before each mutant, and after restoration. No product
  source in the working checkout is edited.
- Committed blocked case IDs are associations only. The cases were not executed,
  no expected behavior was invented, and no spec gap or coverage cell changed.

## Final Result

Platform: `trust-builder`, Linux x86-64.

| Mutant | Outcome | Associated committed case IDs |
| --- | --- | --- |
| `MUTANT_VALIDATE_INSTRUCTION_STREAM_BYPASS` | caught | `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_FF_32935955`, `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_80_CA909A71`, `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND_100_6DD115EE`, `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND__100_09FC189F` |
| `MUTANT_VALIDATE_STACK_SHAPE_BYPASS` | caught | `VM_SEAM_VALID_001_STACK_UNDERFLOW_POU_BODY_POP_EMPTY_STACK_1CBF84A9` |

Summary: 2 total, 2 caught, 0 survived, 0 unviable, 0 timeouts, 0
errors.

Explicitly out of scope:

- `VM_SEAM_VALID_001_TRUNCATE_BEFORE_SECTION_TABLE_58B11C2B`
- `VM_SEAM_VALID_001_TRUNCATE_BEFORE_POU_BODIES_D6833A8D`

Those inputs fail during bytecode decoding before `BytecodeModule.validate()`;
decoder mutation is outside this validator-only shard.

Machine report:

- `p1b-bytecode-validator-mutation-report.json`
- SHA-256:
  `4086046a2bc49ff2767fdea058eeace1b6da5031a018fd3b2a48beb33ee62ef6`

The latest 2026-07-10 refresh was required because the Phase 4 validator and
oracle-eligibility modules changed the case-generator provenance digest and
therefore the bytecode-validator case-file digest. The case IDs, mutant
selectors, commands, and outcomes are unchanged; the shard was rerun against
the final clean implementation commit rather than hand-editing the binding.

## Tests-First And Tooling Corrections

- The new mutation contract test initially failed because the runner and catalog
  did not exist.
- A full `Validator.validate()` corruption fixture replaced the earlier direct
  digest-catcher-only check.
- The first remote attempt stopped before mutation because `cargo-mutants` was
  absent. Version 27.0.0 was installed and pinned in the final report.
- A pre-final configuration used an ignored Phase 11 stack test and produced a
  zero-test false survivor. A contract test now pins the active
  `bytecode_vm_core::vm_rejects_stack_underflow` regression instead; that
  pre-final report is not indexed as evidence.
- Reusing a general Cargo target exposed stale mutant-artifact reuse across
  archived workspaces. A regression test now pins package-scoped cleanup and the
  final run uses a dedicated mutation target. The contaminated general target
  was cleaned before further validation.
- Independent review found that the first at-rest contract checked result labels
  but did not bind selectors, commands, or raw exit/timeout fields. Failing
  adversarial fixtures now pin those fields, full `Validator.validate()` uses
  the already-loaded catalog record, and infrastructure failures abort instead
  of being mislabeled as mutation outcomes. The machine report was regenerated
  from the hardened runner.
- The closing review also found a lexical `source_file` prefix escape and
  impossible build/test phase combinations that the committed report did not
  contain. Tests now require resolved source paths to stay under the validator
  directory and reject test results recorded after a failed or timed-out build.
- The acceptance review found that a hand-edited complete report could retain a
  visible infrastructure-error outcome even though the runner aborts before
  writing such a report. A regression fixture now requires every complete report
  to contain zero infrastructure errors.

## Architecture Review

- SOLID: mutation execution, catalog contracts, report validation, committed
  metadata orchestration, and the CLI entry remain separate modules.
- KISS: two explicit function-bypass mutants cover the first useful validator
  slice; decoder and other Phase 10 shards stay out of scope.
- DRY: the catalog is the single source for mutant selectors, commands, case
  mappings, and survivor actions; the runner and at-rest validator consume it.
- No runtime/VM product behavior, CI enforcement, skills, agent instructions,
  release metadata, or spec-gap status changed.

## Validation

Final remote mutation run on `trust-builder`:

```text
cd "$HOME/projects/trust-platform-p4-validation-85af612b2"
TMPDIR="$HOME/.cache/codex-targets/trust-platform-p4-mutation-tmp-3bf9" \
  python3 scripts/bytecode_validator_mutation.py \
  --target-dir "$HOME/.cache/codex-targets/trust-platform-p4-mutation-3bf9" \
  --output-json "$HOME/p4-evidence-3bf9/bytecode-validator-mutation-report.json" \
  --output-markdown "$HOME/p4-evidence-3bf9/bytecode-validator-mutation-report.md"
```

Result: 2 caught, 0 survived, 0 unviable, 0 timeout, 0 error. Its two cataloged
baseline commands passed before mutation; their exact argv, exit status, and
duration are in the machine report.

Final focused validation was run locally and in the isolated remote copy:

```text
python3 -m unittest \
  scripts.verification.adversarial_selftest_tests \
  scripts.verification.report_gate_tests \
  scripts.verification.prover_tests \
  scripts.verification.metadata_validator.evidence_proof_tests \
  scripts.verification.bytecode_transforms_tests \
  scripts.verification.bytecode_validator_mutation_tests
python3 scripts/validate_verification_metadata.py
scripts/verification_metadata_gate.sh
python3 scripts/gen_cases.py --invariant VM_SEAM_SUBRANGE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_DECLARED_TYPE_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_STRING_BOUND_001 --check
python3 scripts/gen_cases.py --invariant VM_SEAM_VALID_001 --check
python3 -m py_compile \
  scripts/bytecode_validator_mutation.py \
  scripts/verification/bytecode_validator_mutation.py \
  scripts/verification/metadata_validator/mutation_contracts.py \
  scripts/verification/metadata_validator/mutation_reports.py \
  scripts/verification/metadata_validator/mutation_shards.py
git diff --check
```

Latest mutation refresh result: 2 caught, 0 survived, 0 unviable, 0 timeout,
and 0 error. The source commit and regenerated case-file digest are bound in
the machine report. The broader Phase 4 focused and remote closure commands are
recorded in the dated Phase 4 validation evidence rather than retroactively
claimed here.
