# Test-Class Completeness Report

Generator: `test-class-completeness v1`
Source revision: `b32dc62ee71dd66b2985f94342d5c19bd70ae559`
Generated: `2026-07-13T16:48:52+02:00`
Platform: `linux-aarch64`
Generated JSON SHA-256: `7e6b122d3942a58a1eaf016395b6a0cba98229de10f4a1acc875d29cc4406493`
Input SHA-256: `sha256:c0af3f3cd2c812f536152e0e26a20d230bc76f9f42ae8b66d0b3b85e2025a225`

`complete` means the report was generated and bound successfully. It does not
mean every scanner fact or required test class is mapped.

## Summary

- Scanner facts: 3823
- Classified scanner facts: 22
- Unmapped scanner facts: 3801
- Catalog records: 27
- Runnable catalog records: 23
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
| `rust_integration_test` | 1372 | 21 | 1351 |
| `rust_unit_test` | 1656 | 1 | 1655 |
| `structured_text_test` | 257 | 0 | 257 |
| `vscode_test` | 456 | 0 | 456 |

Classified mappings:

- `DISC_88F921D24D3708CEF3E1` -> `TEST_BYTECODE_CONTAINER_INVALID_MAGIC`
- `DISC_C5A04B37E39DDBE237C5` -> `TEST_IEC_TIMER_TRACE_001`
- `DISC_21561AEEDB07941017B8` -> `TEST_RUNTIME_RESTART_TRACE_001`
- `DISC_4E0EFE2F6CE2913C5C99` -> `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001`
- `DISC_F85382D0B4505D5984DB` -> `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001`
- `DISC_34DA98F67689CD13B551` -> `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001`
- `DISC_266A60F3E75C685188EB` -> `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001`
- `DISC_934C37AAF7B29C216166` -> `TEST_VM_COERCION_INITIALIZER_WIDENING_001`
- `DISC_B7E97F9BAA11D0743107` -> `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001`
- `DISC_56F608E0FA7014D18845` -> `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001`
- `DISC_F103626E494B610C2A79` -> `TEST_VM_COERCION_RETURN_WIDENING_001`
- `DISC_C1C1116CE99213C96040` -> `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001`
- `DISC_043E03287D0BD498DBFE` -> `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001`
- `DISC_475B227B49C510855C19` -> `TEST_VM_DECLARED_LITERAL_CONTEXT_001`
- `DISC_8E59F13125886D732DA2` -> `TEST_VM_DECLARED_PARAMETER_COPY_IN_001`
- `DISC_278EEBF061FD97B539D0` -> `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001`
- `DISC_1D261CB173DC0CF72297` -> `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001`
- `DISC_547AC119BF2B3BEFB960` -> `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001`
- `DISC_265444021E11E0C2B452` -> `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001`
- `DISC_1257BED418738BD81458` -> `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001`
- `DISC_D76D0BCB14E615250E0B` -> `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001`
- `DISC_093B7EAE0DCB979D4540` -> `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001`

## Area: `bytecode_vm`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `failing_regression` | none | none | no |
| `iec_conformance` | none | none | no |
| `metadata_validation` | none | `TEST_CASE_TABLE_VM_SEAM_DECLARED_TYPE_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_STRING_BOUND_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_SUBRANGE_001` (planned; catalog_status:planned), `TEST_CASE_TABLE_VM_SEAM_VALID_001` (planned; catalog_status:planned) | no |
| `mutation` | `TEST_BYTECODE_VALIDATOR_MUTATION_SHARD_001` | none | yes |
| `negative_malformed_input` | `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` | none | yes |

Additional catalog classes:

- `integration`: runnable `TEST_VM_COERCION_ASSIGNMENT_WIDENING_001`, `TEST_VM_COERCION_FUNCTION_INPUT_WIDENING_001`, `TEST_VM_COERCION_FUNCTION_OUTPUT_WIDENING_001`, `TEST_VM_COERCION_INITIALIZER_WIDENING_001`, `TEST_VM_COERCION_INOUT_NARROWING_REJECTION_001`, `TEST_VM_COERCION_NARROWING_ASSIGNMENT_REJECTION_001`, `TEST_VM_COERCION_RETURN_WIDENING_001`, `TEST_VM_DECLARED_DINT_CONVERSION_PATHS_001`, `TEST_VM_DECLARED_DINT_RUNTIME_TAG_001`, `TEST_VM_DECLARED_LITERAL_CONTEXT_001`, `TEST_VM_DECLARED_PARAMETER_COPY_IN_001`, `TEST_VM_DECLARED_REAL_CONVERSION_PATHS_001`, `TEST_VM_DECLARED_REAL_RUNTIME_TAG_001`, `TEST_VM_SUBRANGE_ASSIGNMENT_REJECTION_001`, `TEST_VM_SUBRANGE_FB_INPUT_REJECTION_001`, `TEST_VM_SUBRANGE_REF_WRITE_REJECTION_001`, `TEST_VM_SUBRANGE_RETAIN_RELOAD_REJECTION_001`; non-runnable none
- `unit`: runnable `TEST_VM_SUBRANGE_HMI_WRITE_REJECTION_001`; non-runnable none

## Area: `compiler_iec`

| Required class | Runnable tests | Non-runnable rows | Complete |
| --- | --- | --- | --- |
| `iec_conformance` | none | none | no |
| `metadata_validation` | none | none | no |
| `negative_malformed_input` | none | none | no |
| `unit` | none | none | no |

Additional catalog classes:

- `failing_regression`: runnable `TEST_IEC_TIMER_TRACE_001`; non-runnable none

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

Additional catalog classes:

- `failing_regression`: runnable `TEST_RUNTIME_RESTART_TRACE_001`, `TEST_RUNTIME_WATCHDOG_BEFORE_OUTPUT_COMMIT_001`; non-runnable none

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
