# Coverage-Matrix Gap Report

Generator: `coverage-matrix-gap-report v1`
Source revision: `f07b81016252439c649a44761d5b0d7997c9e2c0`
Generated: `2026-07-14T10:16:15+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `ae11a9601d7e1cc60ed73f76a42f45d1c9be6d0c1925764b06dcd0861f259a99`
Input SHA-256: `sha256:a1331332657ca22e28f85ff00b135f54fb903551a5149369ad33b0c0b4192d05`

`complete` means the report was generated and bound successfully. It does not
mean every required coverage slot is assigned or covered.

## Summary

- Mapped areas: 11
- Mapped-area invariants: 53
- Out-of-scope invariants: 0
- Required family slots: 80
- Assigned required slots: 16
- Missing required slots: 64
- Additional recorded cells: 49
- Recorded mapped-area cells: 65
- Catalog-bound case files: 4
- Case observations: 27
- Blocked case observations: 14

## Declared State Counts

| State | Cells |
| --- | ---: |
| `covered` | 6 |
| `covered_by_fuzz` | 0 |
| `not_applicable` | 0 |
| `blocked` | 0 |
| `spec_gap` | 47 |
| `gap_open` | 12 |
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
| `happy_path` | `assigned` | `spec_gap` | none | none |
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
| `above_max` | `assigned` | `covered` | none | none |
| `below_min` | `assigned` | `gap_open` | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `assigned` | `gap_open` | none | none |
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

## Area: `compiler_iec`

Required families: none

### `IEC_PARSE_RECOVER_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `wrong_type_or_shape` | `additional_recorded` | `spec_gap` | none | none |

### `IEC_PREC_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `gap_open` | none | none |

### `IEC_STRING_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `boundary_high` | `additional_recorded` | `spec_gap` | none | none |

### `IEC_SUBRANGE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `boundary_high` | `additional_recorded` | `gap_open` | none | none |

### `IEC_TIMER_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `covered` | none | none |
| `time_or_clock_fault` | `additional_recorded` | `gap_open` | none | none |

## Area: `control_security`

Required families: none

### `DEBUG_AUTH_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `auth_or_permission` | `additional_recorded` | `spec_gap` | none | none |

### `SEC_AUTHZ_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `auth_or_permission` | `additional_recorded` | `spec_gap` | none | none |

## Area: `editor_safety`

Required families: none

### `DEBUG_BEHAVIOR_LOCKED_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `happy_path` | `additional_recorded` | `spec_gap` | none | none |

### `DEBUG_PAUSE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `time_or_clock_fault` | `additional_recorded` | `spec_gap` | none | none |

### `EDIT_DIAG_CANCEL_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `concurrency_or_cancellation` | `additional_recorded` | `spec_gap` | none | none |

### `EDIT_LSP_POS_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `encoding_or_unicode` | `additional_recorded` | `spec_gap` | none | none |

### `EDIT_RENAME_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `duplicate_or_collision` | `additional_recorded` | `spec_gap` | none | none |

### `EDIT_RENAME_002`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `duplicate_or_collision` | `additional_recorded` | `spec_gap` | none | none |

## Area: `hmi_ui`

Required families: none

### `UI_STATUS_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `spec_gap` | none | none |

## Area: `plcopen_devtools`

Required families: none

### `DEV_COMMIT_SCOPE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `duplicate_or_collision` | `additional_recorded` | `spec_gap` | none | none |

### `DEV_TEST_DISCOVERY_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `platform_or_filesystem_variation` | `additional_recorded` | `spec_gap` | none | none |

### `PLCO_IMPORT_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `wrong_type_or_shape` | `additional_recorded` | `gap_open` | none | none |

## Area: `protocols`

Required families: none

### `PROTO_ADS_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `spec_gap` | none | none |

### `PROTO_DISCOVERY_TRUTH_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `spec_gap` | none | none |

### `PROTO_ETHERCAT_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `resource_limit` | `additional_recorded` | `spec_gap` | none | none |

### `PROTO_MODBUS_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `spec_gap` | none | none |

### `PROTO_MQTT_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `spec_gap` | none | none |

### `PROTO_OPCUA_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `persistence_or_recovery` | `additional_recorded` | `gap_open` | none | none |

### `PROTO_STATUS_TRUTH_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `spec_gap` | none | none |

## Area: `release`

Required families: none

### `RELEASE_PLATFORM_MATRIX_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `happy_path` | `additional_recorded` | `spec_gap` | none | none |

### `RELEASE_SOURCE_BUILD_OPENOT_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `supply_chain_or_artifact_fault` | `additional_recorded` | `spec_gap` | none | none |

### `REL_CLAIM_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `spec_gap` | none | none |

### `REL_CONF_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `supply_chain_or_artifact_fault` | `additional_recorded` | `spec_gap` | none | none |

### `REL_VERSION_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `supply_chain_or_artifact_fault` | `additional_recorded` | `spec_gap` | none | none |

### `RUNTIME_BEHAVIOR_LOCKED_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `happy_path` | `additional_recorded` | `spec_gap` | none | none |

## Area: `runtime_safety`

Required families: none

### `RT_RELOAD_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `spec_gap` | none | none |

### `RT_SAFE_DEADLINE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `time_or_clock_fault` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_FORCE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `spec_gap` | none | none |

### `RT_SAFE_IO_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_IO_WORKER_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `concurrency_or_cancellation` | `additional_recorded` | `spec_gap` | none | none |

### `RT_SAFE_NAN_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `persistence_or_recovery` | `additional_recorded` | `covered` | none | none |
| `wrong_type_or_shape` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_PANIC_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `concurrency_or_cancellation` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_RESTART_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `persistence_or_recovery` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_RESTART_TIME_002`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `persistence_or_recovery` | `additional_recorded` | `covered` | none | none |
| `time_or_clock_fault` | `additional_recorded` | `covered` | none | none |

### `RT_SAFE_RETAIN_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `persistence_or_recovery` | `additional_recorded` | `covered` | none | none |

### `RT_SAFE_STOP_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `spec_gap` | none | none |

## Area: `supply_chain_platform`

Required families: none

### `PLAT_PATH_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `platform_or_filesystem_variation` | `additional_recorded` | `spec_gap` | none | none |

### `PLAT_VSCODE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `platform_or_filesystem_variation` | `additional_recorded` | `spec_gap` | none | none |

### `SEC_ARTIFACT_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `supply_chain_or_artifact_fault` | `additional_recorded` | `spec_gap` | none | none |

### `SEC_DEP_AUDIT_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `supply_chain_or_artifact_fault` | `additional_recorded` | `spec_gap` | none | none |

## Area: `verification`

Required families: none

## Out-Of-Scope Invariants

- none

## Limitations

- Completeness is assessed only for invariants in mapped planning-matrix areas.
- Missing required slots are structural debt and never receive a synthetic coverage state.
- Recorded coverage states are copied from invariant metadata and are not independently promoted.
- Committed cases are planning observations only; blocked or runnable cases never upgrade a state.
- Covered and covered_by_fuzz remain metadata claims rather than standalone behavior proof.
- Debt is report output and does not make successful report generation fail.
- Platform is historical provenance requiring evidence review; at-rest validation cannot rederive a prior host.
