# Verification Tooling Self-Test Fixture Report

Contract: `PLC_VERIFICATION_TOOLING_BYPASSES_V1`
Fixtures matched: `33/33`
Spec-source scanner self-tests: `mapped`
Metadata proves assertion strength: `false`

| Fixture | Board row | Assigned layer | Expected | Actual | Signal matched | Full wiring |
| --- | --- | --- | --- | --- | --- | --- |
| `P6A_BAD_ASSERT_NOTHING_RED_001` | `VERIF-P6A-010` | `proof_producer` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_CASE_UNKNOWN_FAMILY_001` | `VERIF-P6A-009` | `case_file_validator` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_COMPILE_ERROR_AS_RED_001` | `VERIF-P6A-009` | `proof_producer` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_DECISION_TABLE_MISSING_BEHAVIOR_001` | `VERIF-P6A-009` | `metadata_validator.validate_invariants` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_GREEN_MISSING_RED_PAIR_001` | `VERIF-P6A-009` | `evidence_pairing` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_HARNESS_PANIC_AS_RED_001` | `VERIF-P6A-009` | `proof_producer` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_HIGH_RISK_GREEN_PRODUCER_001` | `VERIF-P6A-009` | `evidence_pairing` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_HIGH_RISK_RED_PRODUCER_001` | `VERIF-P6A-009` | `evidence_pairing` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_IGNORED_DURABLE_EVIDENCE_001` | `VERIF-P6A-002A` | `metadata_validator.validate_evidence` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_MAPPED_EMPTY_INVARIANTS_001` | `VERIF-P6A-002A` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_MISSING_REQUIRED_FIELD_001` | `VERIF-P6A-002` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_PUBLIC_CLAIM_WITHOUT_PROOF_OR_GAP_001` | `VERIF-P6A-002` | `metadata_validator.validate_public_claim_links` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_RISK_DOWNGRADE_NO_DECISION_001` | `VERIF-P6A-009` | `planner_report` | `report` | `report` | `true` | `n/a` |
| `P6A_BAD_SAFETY_VALIDATED_GAP_OPEN_001` | `VERIF-P6A-002A` | `metadata_validator.validate_invariants` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_SAFETY_VALIDATED_SPEC_GAP_001` | `VERIF-P6A-002A` | `metadata_validator.validate_invariants` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_SCHEMA_VERSION_001` | `VERIF-P6A-002` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_SKIPPED_CASE_ARTIFACT_001` | `VERIF-P6A-009` | `case_artifact_validator` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_SPEC_SOURCE_ESCAPING_INCLUDE_001` | `VERIF-P6A-005` | `spec_source_scanner` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_SPEC_SOURCE_MISSING_REGISTERED_PATH_001` | `VERIF-P6A-005` | `spec_source_scanner` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_SPEC_SOURCE_STALE_CLAIM_TEXT_001` | `VERIF-P6A-005` | `spec_source_scanner` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_SPEC_SOURCE_UNCLOSED_FENCE_001` | `VERIF-P6A-005` | `spec_source_scanner` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_STALE_CASE_DIGEST_001` | `VERIF-P6A-009` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_STALE_RUNNABLE_PATH_001` | `VERIF-P6A-002` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_STALE_TEST_NAME_001` | `VERIF-P6A-002A` | `catalog_staleness` | `reject` | `reject` | `true` | `n/a` |
| `P6A_BAD_UNKNOWN_EVIDENCE_001` | `VERIF-P6A-002A` | `metadata_validator.validate_invariants` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_UNKNOWN_INVARIANT_001` | `VERIF-P6A-002` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_UNKNOWN_STATUS_001` | `VERIF-P6A-002` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_UNKNOWN_SUITE_001` | `VERIF-P6A-002` | `metadata_validator.validate_tests` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_VALIDATED_EMPTY_EVIDENCE_001` | `VERIF-P6A-002A` | `metadata_validator.validate_invariants` | `reject` | `reject` | `true` | `true` |
| `P6A_BAD_VALIDATED_LOW_PROOF_001` | `VERIF-P6A-002A` | `metadata_validator.validate_invariants` | `reject` | `reject` | `true` | `true` |
| `P6A_BOUNDARY_SPEC_SOURCE_UNREVIEWED_PROSE_001` | `VERIF-P6A-005` | `spec_source_scanner` | `report` | `report` | `true` | `n/a` |
| `P6A_GOOD_COMMITTED_METADATA_001` | `VERIF-P6A-001` | `metadata_validator` | `accept` | `accept` | `true` | `true` |
| `P6A_GOOD_SPEC_SOURCE_SCAN_001` | `VERIF-P6A-005` | `spec_source_scanner` | `accept` | `accept` | `true` | `n/a` |

## Limitations

- The known-good fixture is the unmodified committed metadata graph, not a second hand-maintained copy.
- The bypass registry covers VERIF-P6A-002, VERIF-P6A-002A, VERIF-P6A-005, VERIF-P6A-009, and the assertion-strength boundary in VERIF-P6A-010.
- Spec-source scanner fixtures invoke production discovery and association analysis; unreviewed public prose remains a report-only boundary and creates no inferred classification or claim mapping.
- Metadata validation does not establish assertion strength; proof production and mutation evidence own that question.
