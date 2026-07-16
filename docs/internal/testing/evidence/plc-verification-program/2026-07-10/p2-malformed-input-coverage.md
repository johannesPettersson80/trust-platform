# Malformed-Input Coverage Report

Generator: `malformed-input-coverage v1`
Source revision: `f9f416580a2e7b22a184bb856ab4a0847235d0f8`
Generated: `2026-07-16T23:26:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `2058b526f289a0e333f2d61f59ce46daf9cb1d5a0f118e397f9766a4596d8c1e`
Input SHA-256: `sha256:aed4d81f77bf5564da90796246c9c76d4f3b50431586a187f2fa646f78b5970a`

`complete` means the reviewed taxonomy and live joins validated. It does not
mean every malformed-input class is covered.

## Summary

- Taxonomy classes: 28
- Classes with catalog mappings: 28
- Explicit test mappings: 37
- `covered`: 28
- `covered_by_fuzz`: 0
- `not_applicable`: 0
- `blocked`: 0
- `spec_gap`: 0
- `gap_open`: 0
- `deferred`: 0

## Classes

| Class | Disposition | State | Runnable tests | Fuzz tests | Non-runnable tests | Open spec gaps |
| --- | --- | --- | --- | --- | --- | --- |
| `ambiguous_instance_owner` | `required` | `covered` | `TEST_BYTECODE_OWNER_PRODUCT_REJECTION_001`, `TEST_BYTECODE_OWNER_SHARED_FRAME_REJECTION_001`, `TEST_BYTECODE_OWNER_VALIDATOR_REJECTION_001` | none | none | none |
| `argument_count_resource_limit` | `required` | `covered` | `TEST_VM_RESOURCE_LIMIT_CASES_001` | none | none | none |
| `bad_magic` | `required` | `covered` | `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `call_depth_resource_limit` | `required` | `covered` | `TEST_VM_RESOURCE_LIMIT_CASES_001` | none | none | none |
| `call_target_mismatch` | `required` | `covered` | `TEST_BYTECODE_CALL_TARGET_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `const_type_incompatible` | `required` | `covered` | `TEST_BYTECODE_STORE_TYPE_REJECTION_001` | none | none | none |
| `duplicate_section` | `required` | `covered` | `TEST_BYTECODE_CONTAINER_DUPLICATE_STANDARD_SECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `instructions_resource_limit` | `required` | `covered` | `TEST_VM_RESOURCE_LIMIT_CASES_001` | none | none | none |
| `invalid_checksum` | `required` | `covered` | `TEST_BYTECODE_CHECKSUM_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `jump_target_not_instruction_boundary` | `required` | `covered` | `TEST_BYTECODE_JUMP_BOUNDARY_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `jump_target_out_of_bounds` | `required` | `covered` | `TEST_BYTECODE_VALIDATOR_CASES_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `local_frame_reference_persistence` | `required` | `covered` | `TEST_BYTECODE_REF_ESCAPE_PRODUCT_REJECTION_001`, `TEST_BYTECODE_REF_ESCAPE_VALIDATOR_REJECTION_001` | none | none | none |
| `locals_resource_limit` | `required` | `covered` | `TEST_VM_RESOURCE_LIMIT_CASES_001` | none | none | none |
| `missing_instance_owner` | `required` | `covered` | `TEST_BYTECODE_MISSING_OWNER_FIELD_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `missing_section` | `required` | `covered` | `TEST_BYTECODE_MISSING_SECTION_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `operand_index_out_of_bounds` | `required` | `covered` | `TEST_BYTECODE_LOCAL_REF_RANGE_REJECTION_001` | none | none | none |
| `parameter_direction_mismatch` | `required` | `covered` | `TEST_BYTECODE_INOUT_LITERAL_REJECTION_001`, `TEST_BYTECODE_PARAMETER_DIRECTION_REJECTION_001` | none | none | none |
| `reference_escape` | `required` | `covered` | `TEST_BYTECODE_REF_ESCAPE_PRODUCT_REJECTION_001`, `TEST_BYTECODE_REF_ESCAPE_VALIDATOR_REJECTION_001` | none | none | none |
| `refs_resource_limit` | `required` | `covered` | `TEST_VM_RESOURCE_LIMIT_CASES_001` | none | none | none |
| `stack_leftover` | `required` | `covered` | `TEST_BYTECODE_STACK_EXIT_REJECTION_001` | none | none | none |
| `stack_type_mismatch` | `required` | `covered` | `TEST_BYTECODE_ARITHMETIC_TYPE_REJECTION_001`, `TEST_BYTECODE_STORE_TYPE_REJECTION_001` | none | none | none |
| `stack_underflow` | `required` | `covered` | `TEST_BYTECODE_STACK_UNDERFLOW_REJECTION_001`, `TEST_BYTECODE_VALIDATOR_CASES_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `stale_instance_owner` | `required` | `covered` | `TEST_BYTECODE_OWNER_PRODUCT_REJECTION_001`, `TEST_BYTECODE_OWNER_VALIDATOR_REJECTION_001` | none | none | none |
| `truncated_section` | `required` | `covered` | `TEST_BYTECODE_VALIDATOR_CASES_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `unknown_opcode` | `required` | `covered` | `TEST_BYTECODE_LEGACY_CALL_REJECTION_001`, `TEST_BYTECODE_VALIDATOR_CASES_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `unsupported_schema_tag` | `required` | `covered` | `TEST_BYTECODE_SCHEMA_TAG_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `unsupported_version` | `required` | `covered` | `TEST_BYTECODE_VERSION_REJECTION_001` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `wrong_section` | `required` | `covered` | `TEST_BYTECODE_FIXED_SECTION_COUNT_BOUND_001` | none | none | none |

## Limitations

- The v1 machine taxonomy covers only the inventoried bytecode_vm area and bytecode container/instruction-stream surface.
- Mappings come only from reviewed malformed_input_class_ids on generated native or fuzz rows.
- Names, paths, commands, lexical references, case IDs, and mutation associations never create coverage.
- A spec-gap disposition remains spec_gap even when an associated test exists.
- Covered means an explicit effectively runnable catalog mapping exists; it is not behavior proof or spec-gap closure.
- Unmapped classes and tests are report debt and do not make generation fail.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
