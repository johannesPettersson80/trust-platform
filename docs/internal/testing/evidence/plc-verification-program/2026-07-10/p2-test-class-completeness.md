# Test-Class Completeness Report

Generator: `test-class-completeness v1`
Source revision: `c25c62f87b6fe4d768c4ce47a416d1d464cff157`
Generated: `2026-07-11T20:43:20+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `e09758d15a4c6dd776f7b8085956f6bbaff19bc96bee95286753db86129dbdd4`
Input SHA-256: `sha256:aa99703f7e00b86e041044be7f1b7e5e7474efd9b4b84a8816f79602dbd8136f`

`complete` means the report was generated and bound successfully. It does not
mean every scanner fact or required test class is mapped.

## Summary

- Scanner facts: 3816
- Classified scanner facts: 1
- Unmapped scanner facts: 3815
- Catalog records: 6
- Runnable catalog records: 2
- Non-runnable catalog records: 4
- Mapped areas: 11
- Required class slots: 32
- Complete required class slots: 2
- Missing required class slots: 30

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

## Area: `compiler_iec`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `iec_conformance` | none | none | no |
| `metadata_validation` | none | none | no |
| `negative_malformed_input` | none | none | no |
| `unit` | none | none | no |

## Area: `control_security`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `integration` | none | none | no |
| `rbac_security` | none | none | no |
| `runtime_vertical` | none | none | no |

## Area: `editor_safety`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `lsp_protocol` | none | none | no |
| `unit` | none | none | no |
| `vscode_extension` | none | none | no |

## Area: `hmi_ui`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `browser_webview_visual` | none | none | no |
| `integration` | none | none | no |
| `runtime_vertical` | none | none | no |
| `ui_journey_acceptance` | none | none | no |

## Area: `plcopen_devtools`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `integration` | none | none | no |
| `negative_malformed_input` | none | none | no |
| `unit` | none | none | no |

## Area: `protocols`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `integration` | none | none | no |
| `protocol_loopback` | none | none | no |
| `unit` | none | none | no |

## Area: `release`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `release_docs` | none | none | no |

## Area: `runtime_safety`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `integration` | none | none | no |
| `runtime_vertical` | none | none | no |
| `unit` | none | none | no |

## Area: `supply_chain_platform`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `platform_package` | none | none | no |
| `supply_chain_security` | none | none | no |

## Area: `verification`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `metadata_validation` | none | none | no |

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
