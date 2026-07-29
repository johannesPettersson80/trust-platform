# Frozen test-catalogue gap report

- Source commit: `4c35370499ff2423a22de327a67be444f5c5fdf5`
- Basis: committed frozen denominator; no live scan, coverage run, build, or test execution

## Summary

- Frozen live tests: 4409
- Catalog mapped: 3942
- Reviewed nonmapping: 467
- Missing spec authority tests: 0
- Owned nonmapping tests: 467
- Specification batches: 0
- Specified gap open cells: 208
- Invariants requiring exact behavior mapping: 20
- Confirmed missing tests: 0

## Specification batches

| Tests | Source kind | Path |
|---:|---|---|

## Explicit owned nonmapping routes

- `assertion_oracle_unresolved`: 247
- `fuzz_program_owned`: 8
- `gate_inventory_owned`: 59
- `ignored_test_registry_owned`: 23
- `not_catalogable_product_oracle`: 130

## Missing-test rule

A specified behavior is confirmed missing only after every existing test has left the missing-spec-authority queue and no accepted catalog mapping binds that exact behavior.
