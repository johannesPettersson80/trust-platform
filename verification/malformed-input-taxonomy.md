# Bytecode Malformed-Input Taxonomy

This reviewed v1 taxonomy covers only the inventoried `bytecode_vm` area and
the `bytecode_container_instruction_stream` surface. Class IDs are atomic
coverage labels. They do not define product behavior or stable error codes.
Expected behavior still comes only from the named oracle or spec gap.

| Class ID | Title | Disposition | Authority |
| --- | --- | --- | --- |
| `ambiguous_instance_owner` | Ambiguous instance owner | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `argument_count_resource_limit` | Argument-count resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `bad_magic` | Bad bytecode magic | `required` | `SPEC_BYTECODE_FORMAT_001` |
| `call_depth_resource_limit` | Call-depth resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `call_target_mismatch` | Call-target mismatch | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `const_type_incompatible` | Constant type incompatible with use | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `duplicate_section` | Duplicate bytecode section | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `instructions_resource_limit` | Instruction-count resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `invalid_checksum` | Invalid bytecode checksum | `required` | `SPEC_BYTECODE_FORMAT_001` |
| `jump_target_not_instruction_boundary` | Jump target not on instruction boundary | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `jump_target_out_of_bounds` | Jump target outside code | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `local_frame_reference_persistence` | Local-frame reference persistence | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `locals_resource_limit` | Locals resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `missing_instance_owner` | Missing instance owner | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `missing_section` | Missing required bytecode section | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `operand_index_out_of_bounds` | Operand index out of bounds | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `parameter_direction_mismatch` | Parameter-direction mismatch | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `reference_escape` | Reference escape | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `refs_resource_limit` | Reference-count resource limit | `spec_gap` | `SPEC_GAP_VM_DETERMINISM_RESOURCE_LIMITS_001` |
| `stack_leftover` | Operand stack leftover | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `stack_type_mismatch` | Operand stack type mismatch | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `stack_underflow` | Operand stack underflow | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `stale_instance_owner` | Stale instance owner | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `truncated_section` | Truncated bytecode section | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `unknown_opcode` | Unknown opcode | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `unsupported_schema_tag` | Unsupported bytecode schema tag | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |
| `unsupported_version` | Unsupported bytecode version | `required` | `SPEC_BYTECODE_FORMAT_001` |
| `wrong_section` | Wrong bytecode section shape | `required` | `SPEC_BYTECODE_FORMAT_001#validator-before-apply` |

`required` means a written oracle exists and absence of an explicit,
effectively runnable catalog mapping is `gap_open`. `spec_gap` remains
`spec_gap` even if an associated test exists. `covered` and
`covered_by_fuzz` are derived report states and are never hand-authored here.
