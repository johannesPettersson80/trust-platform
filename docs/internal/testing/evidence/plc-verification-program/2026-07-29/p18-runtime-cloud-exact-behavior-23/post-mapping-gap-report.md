# Frozen test-catalogue gap report

- Source commit: `dce893b021f95d2156ce882c1d43c9c0a19ca7df`
- Basis: committed frozen denominator; no live scan, coverage run, build, or test execution

## Summary

- Frozen live tests: 4399
- Catalog mapped: 3918
- Reviewed nonmapping: 481
- Missing spec authority tests: 0
- Owned nonmapping tests: 481
- Specification batches: 0
- Specified gap open cells: 216
- Invariants requiring exact behavior mapping: 29
- Confirmed missing tests: 67

## Specification batches

| Tests | Source kind | Path |
|---:|---|---|

## Explicit owned nonmapping routes

- `assertion_oracle_unresolved`: 261
- `fuzz_program_owned`: 8
- `gate_inventory_owned`: 59
- `ignored_test_registry_owned`: 23
- `not_catalogable_product_oracle`: 130

## Missing-test rule

A specified behavior is confirmed missing only after every existing test has left the missing-spec-authority queue and no accepted catalog mapping binds that exact behavior.
