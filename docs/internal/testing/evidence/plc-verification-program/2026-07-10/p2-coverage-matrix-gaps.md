# Coverage-Matrix Gap Report

Generator: `coverage-matrix-gap-report v1`
Source revision: `d6a0e75fa951131ce96df55894829df176253eb0`
Generated: `2026-07-16T09:38:00+02:00`
Platform: `linux-x86_64`
Generated JSON SHA-256: `606c441b9cfbd80a7755f5adfd5ddd035efb4e77253cf9a81007fa4a0c31e536`
Input SHA-256: `sha256:172606ac3063548d2de7dc9f57f90258ab8d1f8e86a750ad6f07e739e6353063`

`complete` means the report was generated and bound successfully. It does not
mean every required coverage slot is assigned or covered.

## Summary

- Mapped areas: 11
- Mapped-area invariants: 53
- Out-of-scope invariants: 0
- Required family slots: 80
- Assigned required slots: 17
- Missing required slots: 63
- Additional recorded cells: 51
- Recorded mapped-area cells: 68
- Catalog-bound case files: 4
- Case observations: 31
- Blocked case observations: 0

## Declared State Counts

| State | Cells |
| --- | ---: |
| `covered` | 21 |
| `covered_by_fuzz` | 0 |
| `not_applicable` | 0 |
| `blocked` | 0 |
| `spec_gap` | 24 |
| `gap_open` | 23 |
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
| `happy_path` | `assigned` | `covered` | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | none | none |

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
| `resource_limit` | `assigned` | `covered` | none | none |
| `wrong_type_or_shape` | `missing_cell` | none | none | none |
| `time_or_clock_fault` | `additional_recorded` | `gap_open` | none | none |

### `VM_SEAM_ENC_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `assigned` | `gap_open` | none | none |
| `happy_path` | `assigned` | `gap_open` | none | none |
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
| `wrong_type_or_shape` | `assigned` | `gap_open` | none | none |

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
| `wrong_type_or_shape` | `assigned` | `gap_open` | none | none |

### `VM_SEAM_STRING_BOUND_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `assigned` | `covered` | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `assigned` | `covered` | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | none | none |

### `VM_SEAM_SUBRANGE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `assigned` | `covered` | none | none |
| `below_min` | `assigned` | `covered` | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `missing_cell` | none | none | none |
| `happy_path` | `assigned` | `covered` | none | none |
| `missing_required` | `missing_cell` | none | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `spec_gap` | none | none |

### `VM_SEAM_VALID_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `above_max` | `missing_cell` | none | none | none |
| `below_min` | `missing_cell` | none | none | none |
| `boundary_high` | `missing_cell` | none | none | none |
| `boundary_low` | `missing_cell` | none | none | none |
| `encoding_or_unicode` | `missing_cell` | none | none | none |
| `extra_or_unknown` | `assigned` | `spec_gap` | none | none |
| `happy_path` | `missing_cell` | none | none | none |
| `missing_required` | `assigned` | `covered` | none | none |
| `resource_limit` | `missing_cell` | none | none | none |
| `wrong_type_or_shape` | `assigned` | `covered` | none | none |

## Area: `compiler_iec`

Required families: none

### `IEC_PARSE_RECOVER_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `extra_or_unknown` | `additional_recorded` | `gap_open` | none | none |
| `resource_limit` | `additional_recorded` | `gap_open` | none | none |
| `wrong_type_or_shape` | `additional_recorded` | `gap_open` | none | none |

### `IEC_PREC_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `gap_open` | none | none |

### `IEC_STRING_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `boundary_high` | `additional_recorded` | `gap_open` | none | none |

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
| `auth_or_permission` | `additional_recorded` | `covered` | none | none |

### `SEC_AUTHZ_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `auth_or_permission` | `additional_recorded` | `covered` | none | none |

## Area: `editor_safety`

Required families: none

### `DEBUG_BEHAVIOR_LOCKED_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `happy_path` | `additional_recorded` | `spec_gap` | none | none |

### `DEBUG_PAUSE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `time_or_clock_fault` | `additional_recorded` | `covered` | none | none |

### `EDIT_DIAG_CANCEL_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `concurrency_or_cancellation` | `additional_recorded` | `gap_open` | none | none |

### `EDIT_LSP_POS_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `encoding_or_unicode` | `additional_recorded` | `gap_open` | none | none |

### `EDIT_RENAME_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `duplicate_or_collision` | `additional_recorded` | `gap_open` | none | none |

### `EDIT_RENAME_002`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `duplicate_or_collision` | `additional_recorded` | `gap_open` | none | none |

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
| `ordering_or_lifecycle` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_DEADLINE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `time_or_clock_fault` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_FORCE_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `ordering_or_lifecycle` | `additional_recorded` | `covered` | none | none |

### `RT_SAFE_IO_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `hardware_or_network_fault` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_IO_WORKER_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `concurrency_or_cancellation` | `additional_recorded` | `covered` | none | none |

### `RT_SAFE_NAN_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `persistence_or_recovery` | `additional_recorded` | `covered` | none | none |
| `wrong_type_or_shape` | `additional_recorded` | `gap_open` | none | none |

### `RT_SAFE_PANIC_001`

| Dimension | Assignment | Declared state | Blocked cases | Issues |
| --- | --- | --- | --- | --- |
| `concurrency_or_cancellation` | `additional_recorded` | `covered` | none | none |

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
| `hardware_or_network_fault` | `additional_recorded` | `covered` | none | none |

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
