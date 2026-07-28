# Frozen test-catalogue gap report

- Source commit: `1338cd7a48e690c43b65c7239216f651a97178b3`
- Basis: committed frozen denominator; no live scan, coverage run, build, or test execution

## Summary

- Frozen live tests: 4399
- Catalog mapped: 3921
- Reviewed nonmapping: 478
- Missing spec authority tests: 0
- Owned nonmapping tests: 478
- Specification batches: 0
- Specified gap open cells: 216
- Invariants requiring exact behavior mapping: 27
- Confirmed missing tests: 38

## Specification batches

| Tests | Source kind | Path |
|---:|---|---|

## Explicit owned nonmapping routes

- `assertion_oracle_unresolved`: 258
- `fuzz_program_owned`: 8
- `gate_inventory_owned`: 59
- `ignored_test_registry_owned`: 23
- `not_catalogable_product_oracle`: 130

## Missing-test rule

A specified behavior is confirmed missing only after every existing test has left the missing-spec-authority queue and no accepted catalog mapping binds that exact behavior.
