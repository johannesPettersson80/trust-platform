"""Focused semantic-review and rejection-suppression contract tests."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.mapping_review import (
    build_mapping_decision,
    stable_decision_id,
    suppressed_proposal_digests,
    validate_mapping_decisions,
)
from scripts.verification.metadata_validator.constants import HIGH_RISKS


PROPOSAL_ID = "MAP_CAND_00000000000000000001_A1B2C3D4"
PROPOSAL_DIGEST = "sha256:" + "1" * 64
SOURCE_DIGEST = "sha256:" + "2" * 64
DISCOVERY_ID = "DISC_00000000000000000001"
ASSERTION_ID = "ASSERT_" + "A" * 24


def _proposal() -> dict[str, object]:
    return {
        "id": PROPOSAL_ID,
        "proposal_digest": PROPOSAL_DIGEST,
        "test_source_digest": SOURCE_DIGEST,
        "discovery_id": DISCOVERY_ID,
        "command": "cargo nextest run -p example -E 'test(=exact::test)'",
        "command_authority": "verified_synthesized",
        "duration_ms": 250,
        "duration_class": "fast",
        "candidate_invariant_ids": ["INV_ONE"],
        "behavior_refs": ["INV_ONE#behavior:0"],
        "assertions": [
            {
                "assertion_id": ASSERTION_ID,
                "kind": "macro:assert",
                "function": "observes_contract",
                "line": 10,
                "expression_digest": "sha256:" + "3" * 64,
                "expression": "assert ! (observed)",
            }
        ],
        "unresolved_assertion_helpers": [],
    }


def _accepted(reviewers: list[str] | None = None) -> dict[str, object]:
    semantic = {
        "schema_version": 1,
        "proposal_id": PROPOSAL_ID,
        "proposal_digest": PROPOSAL_DIGEST,
        "discovery_id": DISCOVERY_ID,
        "outcome": "accepted",
        "reviewer_ids": reviewers or ["reviewer-a", "reviewer-b"],
        "reviewed_source_digest": SOURCE_DIGEST,
        "assertion_reviews": [
            {
                "assertion_id": ASSERTION_ID,
                "disposition": "oracle_bound",
                "behavior_refs": ["INV_ONE#behavior:0"],
                "rationale": "The assertion checks the selected behavior.",
            }
        ],
        "selected_invariant_ids": ["INV_ONE"],
        "selected_behavior_refs": ["INV_ONE#behavior:0"],
        "command": "cargo nextest run -p example -E 'test(=exact::test)'",
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
    return {"id": stable_decision_id(semantic), **semantic}


class MappingReviewContractTests(unittest.TestCase):
    def validate(
        self,
        decision: dict[str, object],
        *,
        risk: str = "safety_critical",
    ) -> list[str]:
        return validate_mapping_decisions(
            decisions=[decision],
            proposals={PROPOSAL_ID: _proposal()},
            invariants={"INV_ONE": {"risk": risk}},
            candidate_generator_id="test-catalog-proposals",
        )

    def test_high_risk_acceptance_requires_two_exact_reviewers(self) -> None:
        self.assertEqual([], self.validate(_accepted()))
        failures = self.validate(_accepted(["reviewer-a"]))
        self.assertTrue(any("two distinct reviewers" in item for item in failures))

    def test_every_high_risk_vocabulary_value_uses_two_reviewers(self) -> None:
        for risk in HIGH_RISKS:
            with self.subTest(risk=risk):
                failures = self.validate(_accepted(["reviewer-a"]), risk=risk)
                self.assertTrue(any("two distinct reviewers" in item for item in failures))

    def test_source_proposal_invariant_and_behavior_bindings_are_exact(self) -> None:
        decision = _accepted()
        decision["proposal_digest"] = "sha256:" + "9" * 64
        decision["reviewed_source_digest"] = "sha256:" + "8" * 64
        decision["selected_invariant_ids"] = ["INV_OTHER"]
        decision["selected_behavior_refs"] = ["INV_ONE#behavior:9"]

        failures = self.validate(decision)

        self.assertTrue(any("proposal digest" in item for item in failures))
        self.assertTrue(any("source digest" in item for item in failures))
        self.assertTrue(any("selected invariants" in item for item in failures))
        self.assertTrue(any("selected behavior" in item for item in failures))

    def test_generator_cannot_review_and_assertion_sampling_is_rejected(self) -> None:
        decision = _accepted(["test-catalog-proposals", "reviewer-b"])
        decision["assertion_reviews"] = []

        failures = self.validate(decision)

        self.assertTrue(any("candidate generator cannot review" in item for item in failures))
        self.assertTrue(any("assertion" in item for item in failures))

    def test_decision_id_is_generated_from_semantic_content(self) -> None:
        decision = _accepted()
        decision["id"] = "MAP_DEC_WRONG"

        failures = self.validate(decision)

        self.assertTrue(any("generated decision ID" in item for item in failures))

    def test_accepted_command_and_duration_must_come_from_trace_proposal(self) -> None:
        decision = _accepted()
        decision["command"] = "cargo test manually-authored"
        decision["confirmed_catalog"]["duration_class"] = "slow"

        failures = self.validate(decision)

        self.assertTrue(any("command does not match traced proposal" in item for item in failures))
        self.assertTrue(any("duration class does not match traced proposal" in item for item in failures))

    def test_decision_builder_generates_identity_and_copies_proposal_provenance(self) -> None:
        review_input = {
            "proposal_id": PROPOSAL_ID,
            "outcome": "accepted",
            "reviewer_ids": ["reviewer-b", "reviewer-a"],
            "assertion_reviews": [
                {
                    "assertion_id": ASSERTION_ID,
                    "disposition": "oracle_bound",
                    "behavior_refs": ["INV_ONE#behavior:0"],
                    "rationale": "The assertion checks the selected behavior.",
                }
            ],
            "selected_invariant_ids": ["INV_ONE"],
            "selected_behavior_refs": ["INV_ONE#behavior:0"],
            "command": "cargo nextest run -p example -E 'test(=exact::test)'",
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

        decision = build_mapping_decision(
            proposal=_proposal(),
            review_input=review_input,
        )

        self.assertEqual(PROPOSAL_DIGEST, decision.get("proposal_digest"))
        self.assertEqual(SOURCE_DIGEST, decision.get("reviewed_source_digest"))
        self.assertEqual(DISCOVERY_ID, decision.get("discovery_id"))
        self.assertEqual(["reviewer-a", "reviewer-b"], decision.get("reviewer_ids"))
        self.assertEqual(stable_decision_id(decision), decision.get("id"))

    def test_rejection_suppresses_only_the_exact_proposal_digest(self) -> None:
        semantic = {
            "schema_version": 1,
            "proposal_id": PROPOSAL_ID,
            "proposal_digest": PROPOSAL_DIGEST,
            "discovery_id": DISCOVERY_ID,
            "outcome": "rejected",
            "reviewer_ids": ["reviewer-a"],
            "reviewed_source_digest": SOURCE_DIGEST,
            "assertion_reviews": [
                {
                    "assertion_id": ASSERTION_ID,
                    "disposition": "non_oracle",
                    "behavior_refs": [],
                    "rationale": "The assertion does not establish this candidate.",
                }
            ],
            "selected_invariant_ids": [],
            "selected_behavior_refs": [],
            "command": "",
            "command_authority": "not_applicable",
            "rejection_reason": "Assertions do not establish the candidate behavior.",
        }
        rejected = {"id": stable_decision_id(semantic), **semantic}

        self.assertEqual([], self.validate(rejected))
        self.assertEqual(
            {PROPOSAL_DIGEST},
            suppressed_proposal_digests([rejected]),
        )
        changed = copy.deepcopy(rejected)
        changed["proposal_digest"] = "sha256:" + "3" * 64
        self.assertEqual(
            {"sha256:" + "3" * 64},
            suppressed_proposal_digests([changed]),
        )


if __name__ == "__main__":
    unittest.main()
