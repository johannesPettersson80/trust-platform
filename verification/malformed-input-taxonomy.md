# Bytecode Malformed-Input Taxonomy

This reviewed v1 taxonomy covers only the inventoried `bytecode_vm` area and
the `bytecode_container_instruction_stream` surface. Class IDs are atomic
coverage labels. They do not define product behavior or stable error codes.
Expected behavior still comes only from the named oracle or spec gap.

| Class ID | Title | Disposition | Authority |
| --- | --- | --- | --- |
| `ambiguous_instance_owner` | Ambiguous instance owner | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `argument_count_resource_limit` | Argument-count resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `bad_magic` | Bad bytecode magic | `required` | `SPEC_BYTECODE_FORMAT_001` |
| `call_depth_resource_limit` | Call-depth resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `call_target_mismatch` | Call-target mismatch | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `const_type_incompatible` | Constant type incompatible with use | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `duplicate_section` | Duplicate bytecode section | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `instructions_resource_limit` | Instruction-count resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `invalid_checksum` | Invalid bytecode checksum | `required` | `SPEC_BYTECODE_FORMAT_001` |
| `jump_target_not_instruction_boundary` | Jump target not on instruction boundary | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `jump_target_out_of_bounds` | Jump target outside code | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `local_frame_reference_persistence` | Local-frame reference persistence | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `locals_resource_limit` | Locals resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `missing_instance_owner` | Missing instance owner | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `missing_section` | Missing required bytecode section | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `operand_index_out_of_bounds` | Operand index out of bounds | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `parameter_direction_mismatch` | Parameter-direction mismatch | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `reference_escape` | Reference escape | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `refs_resource_limit` | Reference-count resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `stack_leftover` | Operand stack leftover | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `stack_type_mismatch` | Operand stack type mismatch | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `stack_underflow` | Operand stack underflow | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `stale_instance_owner` | Stale instance owner | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `truncated_section` | Truncated bytecode section | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `unknown_opcode` | Unknown opcode | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `unsupported_schema_tag` | Unsupported bytecode schema tag | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |
| `unsupported_version` | Unsupported bytecode version | `required` | `SPEC_BYTECODE_FORMAT_001` |
| `wrong_section` | Wrong bytecode section shape | `spec_gap` | `SPEC_GAP_BYTECODE_VALIDATOR_001` |

`required` means a written oracle exists and absence of an explicit,
effectively runnable catalog mapping is `gap_open`. `spec_gap` remains
`spec_gap` even if an associated test exists. `covered` and
`covered_by_fuzz` are derived report states and are never hand-authored here.
