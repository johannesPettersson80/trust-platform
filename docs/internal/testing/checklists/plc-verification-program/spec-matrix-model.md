# Required Specification Matrix Model

This document owns canonical area values and the
`verification/spec-matrix.toml` record shape. The matrix defines what
specification truth must exist before tests and claims can be mapped.

## Canonical Areas

Machine-readable metadata must use one of these area values unless a new value
is added here and to the validator:

- `compiler_iec`
- `bytecode_vm`
- `runtime_safety`
- `protocols`
- `control_security`
- `editor_safety`
- `plcopen_devtools`
- `hmi_ui`
- `release`
- `supply_chain_platform`
- `verification`

`suite` is reserved for suite records. It is not a product area for
`spec-matrix.toml`.

Do not create prose-only areas. If a human grouping spans multiple machine
areas, either split its required specs across canonical areas or record the
explicit mapping in `spec-matrix.toml`.

## Record Shape

`verification/spec-matrix.toml` is a flat registry using
`[[required_specs]]` entries.

```toml
[[required_specs]]
schema_version = 1
id = "REQ_SPEC_BYTECODE_VALIDATOR_SEMANTIC_CONTRACT"
area = "bytecode_vm"
tag = "bytecode_validator_semantic_contract"
title = "Bytecode validator semantic contract"
owner = "trust-runtime"
status = "spec_gap"
expected_authority = ["normative_product", "reviewed_decision"]
spec_gap_ref = "SPEC_GAP_BYTECODE_VALIDATOR_001"
blocks = "test_mapping"
last_reviewed = "2026-07-08"

[[required_specs]]
schema_version = 1
id = "REQ_SPEC_BYTECODE_CONTAINER"
area = "bytecode_vm"
tag = "bytecode_container"
title = "Bytecode container format"
owner = "trust-runtime"
status = "mapped"
expected_authority = ["normative_product"]
source_ref = "SPEC_BYTECODE_FORMAT_001"
blocks = "test_mapping"
last_reviewed = "2026-07-08"
```

Required fields:

- `schema_version`
- `id`
- `area`
- `tag`
- `title`
- `owner`
- `status`
- `expected_authority`
- `blocks`
- `source_ref` or `spec_gap_ref`
- `last_reviewed`

Allowed `blocks` values:

- `test_mapping`: absence blocks test planning/mapping for that area.
- `release_claim`: absence blocks public/release claims but may not block
  internal exploratory test mapping.
- `none_yet`: visible debt that does not currently block test mapping or
  release claims.

## Validator Rules

- `area` must be a canonical area.
- `tag` is the required coverage tag. It must match a `covers` entry in the
  referenced active spec source or be represented by an open spec gap.
- `expected_authority` is an allowlist. A source with an authority outside that
  list cannot satisfy the requirement.
- A `public_claim` source may create a requirement, but it cannot satisfy
  safety-critical or high-risk behavior requirements.
- `plan_tests.py` defines an area as uninventoried when any
  `blocks = "test_mapping"` requirement for that area resolves to neither an
  active source nor an open spec gap.
- Bytecode/VM requirements must be complete enough for Phase 1B. Other areas
  may initially carry open gaps with owners.
- This matrix is a debt map, not a work order. It makes missing truth visible
  without blocking the bytecode/VM pilot on every product-area specification.

## Waivers and Risk Changes

Coverage cells with `state = "not_applicable"` and waived matrix requirements
must include `decision_ref`. The referenced spec source must have
`authority = "reviewed_decision"` or `authority = "reviewed_deviation"` and
`source_status = "active"`.

Free-text `rationale` is still required for reviewer readability, but it is not
enough to waive a required test class, risk, or coverage dimension. The planner
must print every waiver and every risk downgrade in the report for the current
diff.

Risk-change detection requires a baseline:

- locally, `plan_tests.py` must accept an explicit `--baseline <rev>` when it
  reports risk changes;
- in CI, the baseline is the merge base of the pull request unless the workflow
  passes a different reviewed baseline.
