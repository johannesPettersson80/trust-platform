"""Known-good and known-bad tests for mapping control-plane integration."""

from __future__ import annotations

import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.mapping_metadata import (
    DECISION_PATH,
    PROPOSAL_PATH,
    mapping_control_plane_state,
    validate_mapping_control_plane,
)
from scripts.verification.metadata_validator.mapping_schema_contracts import (
    MAPPING_SCHEMA_FILES,
    MAPPING_SCHEMA_REQUIRED_FIELDS,
)
from scripts.verification.metadata_validator.constants import SCHEMA_FILES
from scripts.verification.mapping_review import build_mapping_decision


ROOT = Path(__file__).resolve().parents[3]
DISCOVERY_ID = "DISC_00000000000000000001"
PROPOSAL_ID = "MAP_CAND_00000000000000000001_A1B2C3D4"
SOURCE_PATH = "scripts/verification/metadata_validator/mapping_metadata_tests.py"


def _proposal() -> dict[str, object]:
    source_digest = "sha256:" + hashlib.sha256(
        (ROOT / SOURCE_PATH).read_bytes()
    ).hexdigest()
    return {
        "schema_version": 1,
        "id": PROPOSAL_ID,
        "proposal_digest": "sha256:" + "1" * 64,
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_unit_test",
        "path": SOURCE_PATH,
        "name": "mapping_metadata_fixture",
        "ignore_state": "not_ignored",
        "command": "cargo nextest run -p example -E 'test(=mapping_metadata_fixture)'",
        "command_authority": "verified_synthesized",
        "duration_ms": 75,
        "duration_class": "very_fast",
        "status": "pending",
        "candidate_tier": "C2",
        "candidate_invariant_ids": ["INV_ONE"],
        "covered_code_ids": ["CODE_" + "A" * 64],
        "specified_code_ids": ["CODE_" + "A" * 64],
        "spec_gap_code_ids": [],
        "spec_gap_refs": [],
        "mutation_kill_ids": [],
        "test_source_digest": source_digest,
        "assertion_inventory_digest": "sha256:" + "6" * 64,
        "assertions": [
            {
                "assertion_id": "ASSERT_" + "A" * 24,
                "kind": "macro:assert",
                "function": "mapping_metadata_fixture",
                "line": 1,
                "expression_digest": "sha256:" + "7" * 64,
                "expression": "assert ! (observed)",
            }
        ],
        "unresolved_assertion_helpers": [],
        "trace_record_digest": "sha256:" + "2" * 64,
        "candidate_signal_digest": "sha256:" + "3" * 64,
        "oracle_refs": ["SPEC_EXAMPLE_001#behavior"],
        "behavior_refs": ["INV_ONE#behavior:0"],
        "suggestions": {
            "authoritative": False,
            "test_class": "behavior_lock",
            "expected_result": "Candidate text requiring confirmation.",
            "expected_failure_mode": "Candidate failure text requiring confirmation.",
            "source_behavior_refs": ["INV_ONE#behavior:0"],
        },
    }


def _proposal_document(proposals: list[dict[str, object]]) -> dict[str, object]:
    return {
        "schema_version": 1,
        "generator": "test-catalog-proposals",
        "generator_version": "1",
        "source_commit": "a" * 40,
        "trace_digest": "sha256:" + "4" * 64,
        "candidate_report_digest": "sha256:" + "5" * 64,
        "last_generated": "2026-07-23T20:30:00Z",
        "demands": [],
        "demand_ranking": [],
        "unmapped_code_demand_ranking": [],
        "proposals": proposals,
    }


def _decision_document(
    *,
    snapshots: list[dict[str, object]] | None = None,
    decisions: list[dict[str, object]] | None = None,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "generator": "test-catalog-mapping-review",
        "generator_version": "1",
        "source_commit": "a" * 40,
        "proposal_snapshots": snapshots or [],
        "decisions": decisions or [],
    }


def _fact() -> dict[str, object]:
    return {
        "discovery_id": DISCOVERY_ID,
        "discovery_source_kind": "rust_unit_test",
        "path": SOURCE_PATH,
        "name": "mapping_metadata_fixture",
        "ignore_state": "not_ignored",
    }


def _review(disposition: str = "reviewed_nonmapping") -> dict[str, object]:
    row = {
        **_fact(),
        "disposition": disposition,
        "last_reviewed": "2026-07-23",
    }
    if disposition == "reviewed_nonmapping":
        row["reason_code"] = "no_reviewed_spec_or_invariant_binding"
    else:
        row["catalog_test_id"] = "TEST_EXISTING"
    return row


def _invariants() -> dict[str, dict[str, object]]:
    return {
        "INV_ONE": {
            "status": "mapped",
            "risk": "maintenance",
            "oracle": {
                "kind": "trust_contract",
                "ref": "SPEC_EXAMPLE_001#behavior",
            },
            "behavior": [{}],
        }
    }


class MappingMetadataTests(unittest.TestCase):
    def test_mapping_schema_registry_is_separate_from_case_generator_constants(
        self,
    ) -> None:
        self.assertEqual(
            MAPPING_SCHEMA_FILES,
            frozenset(MAPPING_SCHEMA_REQUIRED_FIELDS),
        )
        self.assertTrue(MAPPING_SCHEMA_FILES.isdisjoint(SCHEMA_FILES))

    def test_uninitialized_control_plane_requires_neither_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.assertEqual((False, []), mapping_control_plane_state(root))

    def test_initialized_control_plane_requires_both_files(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / PROPOSAL_PATH).parent.mkdir(parents=True)
            (root / PROPOSAL_PATH).touch()

            initialized, failures = mapping_control_plane_state(root)

            self.assertFalse(initialized)
            self.assertEqual(DECISION_PATH, failures[0][0])
            self.assertIn("exists without", failures[0][1])

            (root / DECISION_PATH).touch()
            self.assertEqual((True, []), mapping_control_plane_state(root))

    def validate(
        self,
        *,
        proposals: list[dict[str, object]] | None = None,
        decision_document: dict[str, object] | None = None,
        tests: dict[str, dict[str, object]] | None = None,
        review: dict[str, object] | None = None,
    ) -> list[str]:
        return validate_mapping_control_plane(
            root=ROOT,
            proposal_document=_proposal_document(proposals or []),
            decision_document=decision_document or _decision_document(),
            facts={DISCOVERY_ID: _fact()},
            tests=tests or {},
            denominator_reviews={DISCOVERY_ID: review or _review()},
            invariants=_invariants(),
        )

    def test_valid_pending_quarantine_is_schema_and_live_join_clean(self) -> None:
        self.assertEqual([], self.validate(proposals=[_proposal()]))

    def test_known_bad_proposal_counted_as_catalog_coverage_fails(self) -> None:
        failures = self.validate(
            proposals=[_proposal()],
            tests={
                "TEST_EXISTING": {
                    "subject_kind": "generated_test",
                    "discovery_id": DISCOVERY_ID,
                }
            },
            review=_review("catalog_mapped"),
        )

        self.assertTrue(any("quarantine leakage" in item for item in failures))

    def test_known_bad_stale_proposal_fails(self) -> None:
        proposal = _proposal()
        proposal["name"] = "renamed_after_trace"

        failures = self.validate(proposals=[proposal])

        self.assertTrue(any("does not match live fact" in item for item in failures))

    def test_known_bad_catalog_acceptance_without_review_fails(self) -> None:
        failures = self.validate(
            tests={
                "TEST_UNREVIEWED": {
                    "subject_kind": "generated_test",
                    "discovery_id": DISCOVERY_ID,
                    "mapping_proposal_id": PROPOSAL_ID,
                    "mapping_proposal_digest": "sha256:" + "1" * 64,
                    "mapping_decision_id": "MAP_DEC_" + "A" * 24,
                }
            },
            review=_review("catalog_mapped"),
        )

        self.assertTrue(any("without an accepted review decision" in item for item in failures))

    def test_historical_snapshot_keeps_accepted_decision_verifiable_after_queue_removal(
        self,
    ) -> None:
        proposal = _proposal()
        decision = build_mapping_decision(
            proposal=proposal,
            review_input={
                "proposal_id": PROPOSAL_ID,
                "outcome": "accepted",
                "reviewer_ids": ["reviewer-a"],
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
                "command": proposal["command"],
                "command_authority": "verified_exact",
                "confirmed_catalog": {
                    "test_class": "metadata_validation",
                    "area": "verification",
                    "owner": "verification",
                    "expected_result": "The exact assertion passes.",
                    "expected_failure_mode": "A wrong value is accepted.",
                    "suite_tiers": ["pr"],
                    "requires_hardware": False,
                    "requires_network": False,
                    "duration_class": proposal["duration_class"],
                },
            },
        )
        tests = {
            "TEST_ACCEPTED": {
                "subject_kind": "generated_test",
                "discovery_id": DISCOVERY_ID,
                "mapping_proposal_id": PROPOSAL_ID,
                "mapping_proposal_digest": proposal["proposal_digest"],
                "mapping_decision_id": decision["id"],
            }
        }

        failures = self.validate(
            decision_document=_decision_document(
                snapshots=[proposal],
                decisions=[decision],
            ),
            tests=tests,
            review=_review("catalog_mapped"),
        )

        self.assertEqual([], failures)


if __name__ == "__main__":
    unittest.main()
