# Test-Class Completeness Report

Generator: `test-class-completeness v1`
Source revision: `437af609c1d1dd6d2e0a6aabbda87a4ed84ee955`
Generated: `2026-07-10T20:00:00Z`
Platform: `linux-aarch64`
Generated JSON SHA-256: `d99144aab0a3588f956750dcc3919dcffe0a7ec5c3a1a1d9d5636565cb4f6134`
Input SHA-256: `sha256:26c449e9f1c5b6ec26b910fd04cc119275018a17f61c065bf86c05d0531ffa6c`

`complete` means the report was generated and bound successfully. It does not
mean every scanner fact or required test class is mapped.

## Summary

- Scanner facts: 3816
- Classified scanner facts: 1
- Unmapped scanner facts: 3815
- Catalog records: 6
- Runnable catalog records: 2
- Non-runnable catalog records: 4
- Mapped areas: 1
- Required class slots: 5
- Complete required class slots: 2
- Missing required class slots: 3

## Scanner Classification

| Source kind | Facts | Classified | Unmapped |
| --- | ---: | ---: | ---: |
| `conformance_case` | 21 | 0 | 21 |
| `fuzz_target` | 2 | 0 | 2 |
| `gate_script` | 29 | 0 | 29 |
| `github_workflow_job` | 30 | 0 | 30 |
| `rust_integration_test` | 1369 | 1 | 1368 |
| `rust_unit_test` | 1652 | 0 | 1652 |
| `structured_text_test` | 257 | 0 | 257 |
| `vscode_test` | 456 | 0 | 456 |

Classified mappings:

- `DISC_88F921D24D3708CEF3E1` -> `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`

## Area: `bytecode_vm`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `failing_regression` | none | none | no |
| `iec_conformance` | none | none | no |
| `metadata_validation` | none | `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_VALID_001` (planned; catalog_status:planned) | no |
| `mutation` | `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001` | none | yes |
| `negative_malformed_input` | `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` | none | yes |

## Limitations

- A scanner fact is classified only by an exact discovery_id on a generated_test catalog row.
- Case-table and mutation-runner artifacts never enter the scanner-fact classification denominator.
- A required class is complete only when the mapped area has at least one catalog row in a runnable status.
- Planned and other non-runnable rows remain visible but do not satisfy required-class completeness.
- An unmapped fact or missing class is report debt, not proof that no relevant executable test exists.
- The report never infers area, class, invariant, oracle, or expected behavior from names or lexical references.
- Generated tests marked ignored or conditional by the scanner do not satisfy runnable class completeness.
- Scanner exclusions remain those documented by the generated existing-test catalog.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
