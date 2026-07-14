# Malformed-Input Coverage Report

Generator: `malformed-input-coverage v1`
Source revision: `27694b329a62206b51cb8392378d6eb9ee0fd8e2`
Generated: `2026-07-14T23:30:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `a142d9d01fc27864b94e49c769803c6aed01e0b66e3fd1bb3fa464047a01f587`
Input SHA-256: `sha256:63fb1aa4f14f4aed4577b32329f65c34a05c6c0e5f4889269fd302bf937168dd`

`complete` means the reviewed taxonomy and live joins validated. It does not
mean every malformed-input class is covered.

## Summary

- Taxonomy classes: 28
- Classes with catalog mappings: 2
- Explicit test mappings: 2
- `covered`: 1
- `covered_by_fuzz`: 0
- `not_applicable`: 0
- `blocked`: 0
- `spec_gap`: 25
- `gap_open`: 2
- `deferred`: 0

## Classes

| Class | Disposition | State | Runnable tests | Fuzz tests | Non-runnable tests | Open spec gaps |
| --- | --- | --- | --- | --- | --- | --- |
| `ambiguous_instance_owner` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `argument_count_resource_limit` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `bad_magic` | `required` | `covered` | `TEST_BYTECODE_CONTAINER_INVALID_MAGIC` | none | none | `SPEC_GAP_VM_ERROR_MODEL_001` |
| `call_depth_resource_limit` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `call_target_mismatch` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `const_type_incompatible` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `duplicate_section` | `spec_gap` | `spec_gap` | `TEST_BYTECODE_CONTAINER_DUPLICATE_STANDARD_SECTION_001` | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `instructions_resource_limit` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `invalid_checksum` | `required` | `gap_open` | none | none | none | none |
| `jump_target_not_instruction_boundary` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `jump_target_out_of_bounds` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `local_frame_reference_persistence` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `locals_resource_limit` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `missing_instance_owner` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `missing_section` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `operand_index_out_of_bounds` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `parameter_direction_mismatch` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `reference_escape` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `refs_resource_limit` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `stack_leftover` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `stack_type_mismatch` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `stack_underflow` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `stale_instance_owner` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `truncated_section` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `unknown_opcode` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `unsupported_schema_tag` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `unsupported_version` | `required` | `gap_open` | none | none | none | none |
| `wrong_section` | `spec_gap` | `spec_gap` | none | none | none | `SPEC_GAP_BYTECODE_VALIDATOR_001` |

## Limitations

- The v1 machine taxonomy covers only the inventoried bytecode_vm area and bytecode container/instruction-stream surface.
- Mappings come only from reviewed malformed_input_class_ids on generated native or fuzz rows.
- Names, paths, commands, lexical references, case IDs, and mutation associations never create coverage.
- A spec-gap disposition remains spec_gap even when an associated test exists.
- Covered means an explicit effectively runnable catalog mapping exists; it is not behavior proof or spec-gap closure.
- Unmapped classes and tests are report debt and do not make generation fail.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
