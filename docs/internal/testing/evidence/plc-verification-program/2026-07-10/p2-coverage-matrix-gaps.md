# Coverage-Matrix Gap Report

Generator: `coverage-matrix-gap-report v1`
Source revision: `437af609c1d1dd6d2e0a6aabbda87a4ed84ee955`
Generated: `2026-07-10T20:00:00Z`
Platform: `linux-aarch64`
Generated JSON SHA-256: `161a21f93e7edfc687e763d72a531dbd9c47f4bb1e5ed2eb4f9c795dfc7e3115`
Input SHA-256: `sha256:93721081410ed0f60158884f268cf9dd20445e74dc9e90c8d3c6f95ecea49864`

`complete` means the report was generated and bound successfully. It does not
mean every required coverage slot is assigned or covered.

## Summary

- Mapped areas: 1
- Mapped-area invariants: 8
- Out-of-scope invariants: 44
- Required family slots: 80
- Assigned required slots: 16
- Missing required slots: 64
- Additional recorded cells: 1
- Recorded mapped-area cells: 17
- Catalog-bound case files: 4
- Case observations: 21
- Blocked case observations: 21

## Declared State Counts

| State | Cells |
| --- | ---: |
| `covered` | 0 |
| `covered_by_fuzz` | 0 |
| `not_applicable` | 0 |
| `blocked` | 0 |
| `spec_gap` | 17 |
| `gap_open` | 0 |
| `deferred` | 0 |

## Area: `bytecode_vm`

Required families: `above_max`, `below_min`, `boundary_high`, `boundary_low`, `encoding_or_unicode`, `extra_or_unknown`, `happy_path`, `missing_required`, `resource_limit`, `wrong_type_or_shape`

### `VM_SEAM_DECLARED_TYPE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `assigned` | `spec_gap` | `VM_SEAM_DECLARED_TYPE_001_INT_EXPRESSION_TO_DINT_SLOT_9BF228AA`, `VM_SEAM_DECLARED_TYPE_001_INT_LITERAL_TO_REAL_SLOT_04979927`, `VM_SEAM_DECLARED_TYPE_001_INT_VARIABLE_TO_REAL_SLOT_C3821866` | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | `VM_SEAM_DECLARED_TYPE_001_WRONG_TYPE_D8E1DB83` | none |

### `VM_SEAM_DETERMINISM_LIMITS_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `missing_cell` | none | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `assigned` | `spec_gap` | none | none |
| `wrong_type_or_shape` | `missing_cell` | none | none | none |
| `time_or_clock_fault` | `additional_recorded` | `spec_gap` | none | none |

### `VM_SEAM_ENC_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `assigned` | `spec_gap` | none | none |
| `happy_path` | `missing_cell` | none | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `missing_cell` | none | none | none |

### `VM_SEAM_OWNER_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `missing_cell` | none | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | none | none |

### `VM_SEAM_REF_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `missing_cell` | none | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | none | none |

### `VM_SEAM_STRING_BOUND_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `assigned` | `spec_gap` | `VM_SEAM_STRING_BOUND_001_ABOVE_MAX_63B3906D`, `VM_SEAM_STRING_BOUND_001_FB_INPUT_COPY_IN_LENGTH_6_0EC59DCF` | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | `VM_SEAM_STRING_BOUND_001_MAX_D165B4CE` | none |
| `boundary_low` | `missing_cell` | none | `VM_SEAM_STRING_BOUND_001_MIN_D165B4CE` | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `assigned` | `spec_gap` | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | `VM_SEAM_STRING_BOUND_001_WRONG_TYPE_13C1C8F2` | none |

### `VM_SEAM_SUBRANGE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `assigned` | `spec_gap` | `VM_SEAM_SUBRANGE_001_ABOVE_MAX_2EDDED03` | none |
| `below_min` | `assigned` | `spec_gap` | `VM_SEAM_SUBRANGE_001_BELOW_MIN_F2BC55A3` | none |
| `boundary_high` | `missing_cell` | none | `VM_SEAM_SUBRANGE_001_MAX_AE132E71` | none |
| `boundary_low` | `missing_cell` | none | `VM_SEAM_SUBRANGE_001_MIN_AE132E71` | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `assigned` | `spec_gap` | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | `VM_SEAM_SUBRANGE_001_WRONG_TYPE_817CB4E0` | none |

### `VM_SEAM_VALID_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `assigned` | `spec_gap` | `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_80_CA909A71`, `VM_SEAM_VALID_001_UNKNOWN_OPCODE_POU_BODY_FIRST_OPCODE_FF_32935955` | none |
| `happy_path` | `missing_cell` | none | none | none |
| `missing_required` | `assigned` | `spec_gap` | `VM_SEAM_VALID_001_TRUNCATE_BEFORE_POU_BODIES_D6833A8D`, `VM_SEAM_VALID_001_TRUNCATE_BEFORE_SECTION_TABLE_58B11C2B` | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND_100_6DD115EE`, `VM_SEAM_VALID_001_JUMP_TARGET_POU_BODY_JMP_OPERAND__100_09FC189F`, `VM_SEAM_VALID_001_STACK_UNDERFLOW_POU_BODY_POP_EMPTY_STACK_1CBF84A9` | none |

## Out-Of-Scope Invariants

- `DEBUG_AUTH_001` (`control_security`): 1 recorded cells
- `DEBUG_BEHAVIOR_LOCKED_001` (`editor_safety`): 1 recorded cells
- `DEBUG_PAUSE_001` (`editor_safety`): 1 recorded cells
- `DEV_COMMIT_SCOPE_001` (`plcopen_devtools`): 1 recorded cells
- `DEV_TEST_DISCOVERY_001` (`plcopen_devtools`): 1 recorded cells
- `EDIT_DIAG_CANCEL_001` (`editor_safety`): 1 recorded cells
- `EDIT_LSP_POS_001` (`editor_safety`): 1 recorded cells
- `EDIT_RENAME_001` (`editor_safety`): 1 recorded cells
- `EDIT_RENAME_002` (`editor_safety`): 1 recorded cells
- `IEC_PARSE_RECOVER_001` (`compiler_iec`): 1 recorded cells
- `IEC_PREC_001` (`compiler_iec`): 1 recorded cells
- `IEC_STRING_001` (`compiler_iec`): 1 recorded cells
- `IEC_SUBRANGE_001` (`compiler_iec`): 1 recorded cells
- `IEC_TIMER_001` (`compiler_iec`): 1 recorded cells
- `PLAT_PATH_001` (`supply_chain_platform`): 1 recorded cells
- `PLAT_VSCODE_001` (`supply_chain_platform`): 1 recorded cells
- `PLCO_IMPORT_001` (`plcopen_devtools`): 1 recorded cells
- `PROTO_ADS_001` (`protocols`): 1 recorded cells
- `PROTO_DISCOVERY_TRUTH_001` (`protocols`): 1 recorded cells
- `PROTO_ETHERCAT_001` (`protocols`): 1 recorded cells
- `PROTO_MODBUS_001` (`protocols`): 1 recorded cells
- `PROTO_MQTT_001` (`protocols`): 1 recorded cells
- `PROTO_OPCUA_001` (`protocols`): 1 recorded cells
- `PROTO_STATUS_TRUTH_001` (`protocols`): 1 recorded cells
- `RELEASE_PLATFORM_MATRIX_001` (`release`): 1 recorded cells
- `RELEASE_SOURCE_BUILD_OPENOT_001` (`release`): 1 recorded cells
- `REL_CLAIM_001` (`release`): 1 recorded cells
- `REL_CONF_001` (`release`): 1 recorded cells
- `REL_VERSION_001` (`release`): 1 recorded cells
- `RT_RELOAD_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_DEADLINE_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_FORCE_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_IO_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_IO_WORKER_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_NAN_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_PANIC_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_RESTART_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_RETAIN_001` (`runtime_safety`): 1 recorded cells
- `RT_SAFE_STOP_001` (`runtime_safety`): 1 recorded cells
- `RUNTIME_BEHAVIOR_LOCKED_001` (`release`): 1 recorded cells
- `SEC_ARTIFACT_001` (`supply_chain_platform`): 1 recorded cells
- `SEC_AUTHZ_001` (`control_security`): 1 recorded cells
- `SEC_DEP_AUDIT_001` (`supply_chain_platform`): 1 recorded cells
- `UI_STATUS_001` (`hmi_ui`): 1 recorded cells

## Limitations

- Completeness is assessed only for invariants in mapped planning-matrix areas.
- Missing required slots are structural debt and never receive a synthetic coverage state.
- Recorded coverage states are copied from invariant metadata and are not independently promoted.
- Committed cases are planning observations only; blocked or runnable cases never upgrade a state.
- Covered and covered_by_fuzz remain metadata claims rather than standalone behavior proof.
- Debt is report output and does not make successful report generation fail.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
