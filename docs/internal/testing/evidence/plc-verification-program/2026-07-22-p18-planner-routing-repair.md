# Phase 18 planner routing repair review

Date: 2026-07-22  
Implementation commit: `9c7563a51b4e1aeb4203715c708ab13afc382ba8`

## Scope

The signed-integer authority diff exposed two verification-planner defects:

- normative sources under `docs/specs/**` and `docs/IEC_*.md` were default-denied
  as unmapped changed paths;
- docs-only `metadata_validation` was demanded separately in every changed
  product area even though the cataloged repository metadata gate validates the
  complete committed metadata graph once.

The repair extends the existing `verification_tooling` route to the normative
source paths and treats intent-level metadata validation as repository-global
only for `docs`. Per-area failing-regression, malformed-input, unit, and other
behavior requirements remain unchanged for behavior-changing intents.

## Red and green evidence

Expected red command:

```text
python3 -m unittest scripts.verification.area_routing_tests.AreaRoutingTests.test_normative_specification_sources_are_routed_through_verification scripts.verification.planner_tests.PlannerAreaRoutingTests.test_docs_metadata_validation_is_repository_global_not_per_area
```

Before the repair, the command produced four unmapped-path assertion failures
and one duplicate area-local metadata-requirement failure. After the repair,
the same two focused tests passed. The complete routing/planner modules then
passed 26 tests, verification metadata validated 885 records, and the docs
planner returned `clear` for the complete authority-slice changed-path set.

## Code-fact review

`CODE_2047ABF848EF5FF0144439A4538C28D9841A488858F38E4116A398FA0913FF8E`
still carries the parser-derived `protocol_response_or_status` and
`state_mutation` signals. Review of body digest
`sha256:de8454effa30d15d527b8eb7e0fac46048629cc416aa03200ae26b425f03555b`
confirmed that the implementation changes only docs-intent requirement
aggregation; it does not weaken behavior-intent requirements or convert an
unmapped path into an inferred semantic area.

The code fact deliberately remains `spec_gap` against
`SPEC_GAP_P17_HISTORICAL_CODE_VERIFICATION_001`. This review refreshes exact
body and risk identity only; it does not close that historical semantic debt.

## SOLID/KISS/DRY review

Path ownership stays in `verification/matrix.toml`, normalization and route
selection stay in `area_routing.py`, and planning stays in `Planner.plan`.
No ownership, component boundary, or execution-flow diagram changed. No new
validator module or parallel routing list was introduced.
