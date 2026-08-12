"""Closed-schema tests for execution-guided catalog mapping artifacts."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.test_catalog_json_schema import validate_json_schema_instance
from scripts.verification.mapping_review import stable_decision_id
from scripts.verification.metadata_validator.mapping_schema_contracts import (
    MAPPING_SCHEMA_FILES,
    MAPPING_SCHEMA_REQUIRED_FIELDS,
)


SCHEMA_ROOT = Path("verification/schemas")


def _schema(name: str) -> dict:
    return json.loads((SCHEMA_ROOT / name).read_text())


class MappingContractTests(unittest.TestCase):
    def test_mapping_schemas_are_owned_by_the_metadata_validator(self) -> None:
        expected = {
            "mapping-candidates.schema.json",
            "mapping-ground-truth-report.schema.json",
            "test-catalog-mapping-decisions.schema.json",
            "test-catalog-mapping-review-card.schema.json",
            "test-catalog-mapping-review-selection.schema.json",
            "test-catalog-proposals.schema.json",
            "test-coverage-trace.schema.json",
        }

        self.assertTrue(expected.issubset(MAPPING_SCHEMA_FILES))
        self.assertTrue(expected.issubset(MAPPING_SCHEMA_REQUIRED_FIELDS))

    def test_trace_schema_forbids_proof_and_requires_exact_observation_shape(self) -> None:
        record = {
            "schema_version": 1,
            "generator": "test-coverage-trace",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "platform": "linux-x86_64",
            "scope": {
                "mode": "focused_shard",
                "requested_discovery_ids": ["DISC_00000000000000000001"],
                "source_kinds": ["rust_unit_test"],
            },
            "input_digest": "sha256:" + "1" * 64,
            "code_inventory_digests": {
                "rust_product": "sha256:" + "3" * 64,
            },
            "tests": [
                {
                    "discovery_id": "DISC_00000000000000000001",
                    "discovery_source_kind": "rust_unit_test",
                    "path": "crates/example/src/lib.rs",
                    "name": "example",
                    "command": "cargo nextest run example",
                    "command_authority": "exact",
                    "trace_status": "observed",
                    "attribution_kind": "llvm_source_coverage",
                    "attempt_id": "attempt-1",
                    "exit_code": 0,
                    "duration_ms": 10,
                    "profile_digest": "sha256:" + "2" * 64,
                    "covered_code_ids": ["CODE_" + "A" * 64],
                    "limitations": [],
                }
            ],
        }
        schema = _schema("test-coverage-trace.schema.json")
        self.assertEqual([], validate_json_schema_instance(record, schema))

        forged = copy.deepcopy(record)
        forged["tests"][0]["proof_level"] = "T1"
        self.assertTrue(validate_json_schema_instance(forged, schema))

    def test_candidate_schema_uses_c_tiers_and_forbids_coverage_claims(self) -> None:
        record = {
            "schema_version": 1,
            "generator": "test-mapping-candidates",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "trace_digest": "sha256:" + "1" * 64,
            "candidates": [
                {
                    "discovery_id": "DISC_00000000000000000001",
                    "candidate_tier": "C2",
                    "candidate_invariant_ids": ["INV_ONE"],
                    "covered_code_ids": ["CODE_" + "A" * 64],
                    "specified_code_ids": ["CODE_" + "A" * 64],
                    "spec_gap_code_ids": [],
                    "spec_gap_refs": [],
                    "unmapped_code_ids": [],
                    "concentration_ppm": {"INV_ONE": 1000000},
                    "mutation_kill_ids": [],
                    "authority_route_digest": "sha256:" + "a" * 64,
                    "exact_behavior_refs": ["INV_ONE#behavior:0"],
                }
            ],
            "demands": [],
            "spec_demand_observations": [],
            "demand_ranking": [],
            "structural_demand_ranking": [],
            "unmapped_code_demand_ranking": [],
        }
        schema = _schema("mapping-candidates.schema.json")
        self.assertEqual([], validate_json_schema_instance(record, schema))

        forged = copy.deepcopy(record)
        forged["candidates"][0]["candidate_tier"] = "T1"
        forged["candidates"][0]["coverage_state"] = "covered"
        self.assertTrue(validate_json_schema_instance(forged, schema))

        unpaired = copy.deepcopy(record)
        unpaired["candidates"][0].pop("exact_behavior_refs")
        self.assertTrue(validate_json_schema_instance(unpaired, schema))

        malformed = copy.deepcopy(record)
        malformed["candidates"][0]["exact_behavior_refs"] = []
        self.assertTrue(validate_json_schema_instance(malformed, schema))

        malformed = copy.deepcopy(record)
        malformed["candidates"][0]["authority_route_digest"] = "not-a-digest"
        self.assertTrue(validate_json_schema_instance(malformed, schema))

        malformed = copy.deepcopy(record)
        malformed["candidates"][0]["exact_behavior_refs"] = ["not-a-ref"]
        self.assertTrue(validate_json_schema_instance(malformed, schema))

        duplicate = copy.deepcopy(record)
        duplicate["candidates"][0]["exact_behavior_refs"] = [
            "INV_ONE#behavior:0",
            "INV_ONE#behavior:0",
        ]
        self.assertTrue(validate_json_schema_instance(duplicate, schema))

        demand_with_route = copy.deepcopy(record)
        demand_with_route["candidates"] = []
        demand_with_route["demands"] = [
            {
                "discovery_id": "DISC_00000000000000000001",
                "candidate_tier": "C4",
                "covered_code_ids": ["CODE_" + "A" * 64],
                "spec_gap_refs": ["SPEC_GAP_EXAMPLE_001"],
                "unmapped_code_ids": [],
                "authority_route_digest": "sha256:" + "a" * 64,
            }
        ]
        self.assertTrue(validate_json_schema_instance(demand_with_route, schema))

    def test_proposal_schema_keeps_suggestions_non_authoritative(self) -> None:
        record = {
            "schema_version": 1,
            "generator": "test-catalog-proposals",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "trace_digest": "sha256:" + "1" * 64,
            "candidate_report_digest": "sha256:" + "2" * 64,
            "last_generated": "2026-07-23T20:30:00Z",
            "demands": [],
            "demand_ranking": [],
            "structural_demand_ranking": [],
            "unmapped_code_demand_ranking": [],
            "proposals": [
                {
                    "schema_version": 1,
                    "id": "MAP_CAND_00000000000000000001_A1B2C3D4",
                    "proposal_digest": "sha256:" + "3" * 64,
                    "discovery_id": "DISC_00000000000000000001",
                    "discovery_source_kind": "rust_unit_test",
                    "path": "crates/example/src/lib.rs",
                    "name": "example",
                    "ignore_state": "not_ignored",
                    "command": "cargo nextest run example",
                    "command_authority": "exact",
                    "duration_ms": 10,
                    "duration_class": "very_fast",
                    "status": "pending",
                    "candidate_tier": "C2",
                    "candidate_invariant_ids": ["INV_ONE"],
                    "covered_code_ids": ["CODE_" + "A" * 64],
                    "specified_code_ids": ["CODE_" + "A" * 64],
                    "spec_gap_code_ids": [],
                    "spec_gap_refs": [],
                    "mutation_kill_ids": [],
                    "test_source_digest": "sha256:" + "7" * 64,
                    "assertion_inventory_digest": "sha256:" + "8" * 64,
                    "assertions": [
                        {
                            "assertion_id": "ASSERT_" + "A" * 24,
                            "kind": "macro:assert",
                            "function": "example",
                            "line": 10,
                            "expression_digest": "sha256:" + "9" * 64,
                            "expression": "assert ! (observed)",
                        }
                    ],
                    "unresolved_assertion_helpers": [],
                    "trace_record_digest": "sha256:" + "4" * 64,
                    "candidate_signal_digest": "sha256:" + "5" * 64,
                    "authority_route_digest": "sha256:" + "a" * 64,
                    "oracle_refs": ["SPEC_ONE"],
                    "behavior_refs": ["INV_ONE#behavior:0"],
                    "suggestions": {
                        "authoritative": False,
                        "test_class": "behavior_lock",
                        "expected_result": "candidate",
                        "expected_failure_mode": "candidate",
                        "source_behavior_refs": ["INV_ONE#behavior:0"],
                    },
                }
            ],
        }
        schema = _schema("test-catalog-proposals.schema.json")
        self.assertEqual([], validate_json_schema_instance(record, schema))

        legacy = copy.deepcopy(record)
        legacy["proposals"][0].pop("authority_route_digest")
        self.assertEqual([], validate_json_schema_instance(legacy, schema))

        forged = copy.deepcopy(record)
        forged["proposals"][0]["suggestions"]["authoritative"] = True
        self.assertTrue(validate_json_schema_instance(forged, schema))

        malformed = copy.deepcopy(record)
        malformed["proposals"][0]["authority_route_digest"] = "not-a-digest"
        self.assertTrue(validate_json_schema_instance(malformed, schema))

    def test_decision_schema_separates_review_from_generated_metadata(self) -> None:
        decision = {
            "schema_version": 1,
            "proposal_id": "MAP_CAND_00000000000000000001_A1B2C3D4",
            "proposal_digest": "sha256:" + "1" * 64,
            "discovery_id": "DISC_00000000000000000001",
            "outcome": "accepted",
            "reviewer_ids": ["reviewer-a", "reviewer-b"],
            "reviewed_source_digest": "sha256:" + "2" * 64,
            "assertion_reviews": [
                {
                    "assertion_id": "ASSERT_" + "A" * 24,
                    "disposition": "oracle_bound",
                    "behavior_refs": ["INV_ONE#behavior:0"],
                    "rationale": "The assertion checks the selected behavior.",
                }
            ],
            "selected_invariant_ids": ["INV_ONE"],
            "selected_behavior_refs": ["INV_ONE#behavior:0"],
            "command": "cargo test -p example exact::test -- --exact",
            "command_authority": "verified_exact",
            "confirmed_catalog": {
                "test_class": "behavior_lock",
                "area": "runtime_safety",
                "owner": "trust-runtime",
                "expected_result": "The exact value is preserved.",
                "expected_failure_mode": "A different value is accepted.",
                "suite_tiers": ["pr"],
                "requires_hardware": False,
                "requires_network": False,
                "duration_class": "fast",
            },
        }
        decision = {"id": stable_decision_id(decision), **decision}
        document = {
            "schema_version": 1,
            "generator": "test-catalog-mapping-review",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "proposal_snapshots": [],
            "decisions": [decision],
        }
        schema = _schema("test-catalog-mapping-decisions.schema.json")
        self.assertEqual([], validate_json_schema_instance(document, schema))

        forged = copy.deepcopy(document)
        forged["decisions"][0]["reviewer_ids"] = []
        self.assertTrue(validate_json_schema_instance(forged, schema))


if __name__ == "__main__":
    unittest.main()
