"""Focused tests for reviewed catalog-authority reassignment."""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

from scripts.verification.catalog_authority_reassignment import (
    build_catalog_authority_reassignment,
)
from scripts.verification.mapping_catalog_updates import (
    build_accepted_catalog_record,
)
from scripts.verification.mapping_review import build_mapping_decision
from scripts.verification.mapping_review_batch_tests import (
    _accepted_input,
    _proposal,
)


def _invariant(invariant_id: str, area: str) -> dict[str, object]:
    return {
        "id": invariant_id,
        "area": area,
        "owner": "trust-runtime",
        "risk": "safety_critical" if area == "runtime_safety" else "maintenance",
        "status": "mapped",
        "oracle": {"ref": f"SPEC_RUNTIME#{invariant_id.lower()}"},
        "behavior": [
            {
                "partition": {"equals": "EXACT_FIELDS"},
                "case_family": "configuration",
                "oracle_ref": f"SPEC_RUNTIME#{invariant_id.lower()}",
                "outcome": "accept_value",
                "rationale": "The exact configuration fields are retained.",
            }
        ],
    }


def _invariant_source(invariant_id: str, tests: list[str]) -> str:
    rendered = ", ".join(f'"{test_id}"' for test_id in tests)
    return f'id = "{invariant_id}"\ntests = [{rendered}]\n'


class CatalogAuthorityReassignmentTests(unittest.TestCase):
    def test_metadata_gate_registers_catalog_reassignment_tests(self) -> None:
        gate = Path("scripts/verification_metadata_gate.sh").read_text(
            encoding="utf-8"
        )
        self.assertIn(
            "scripts.verification.catalog_authority_reassignment_tests",
            gate,
            "metadata gate does not execute catalog reassignment self-tests",
        )
        self.assertIn(
            "scripts.verification.mapping_catalog_updates_tests",
            gate,
            "metadata gate does not execute catalog update self-tests",
        )

    def test_reviewed_reassignment_moves_one_identity_and_reciprocal_link(self) -> None:
        old_proposal = _proposal(1)
        old_decision = build_mapping_decision(
            proposal=old_proposal,
            review_input=_accepted_input(old_proposal),
        )
        old_invariant_id = str(old_proposal["candidate_invariant_ids"][0])
        new_invariant_id = "INV_TASK_CONFIG"
        invariants = {
            old_invariant_id: _invariant(old_invariant_id, "bytecode_vm"),
            new_invariant_id: _invariant(new_invariant_id, "runtime_safety"),
        }
        old_record = build_accepted_catalog_record(
            proposal=old_proposal,
            decision=old_decision,
            invariants=invariants,
            reviewed_date="2026-08-12",
        )
        test_id = str(old_record["id"])
        catalog_source = "tests = []\n"
        from scripts.verification.mapping_catalog_updates import (
            append_catalog_record,
        )

        catalog_source = append_catalog_record(catalog_source, old_record)
        new_proposal = {
            **old_proposal,
            "id": str(old_proposal["id"]) + "_REFRESHED",
            "proposal_digest": "sha256:" + "9" * 64,
            "candidate_invariant_ids": [new_invariant_id],
            "oracle_refs": [f"SPEC_RUNTIME#{new_invariant_id.lower()}"],
            "behavior_refs": [f"{new_invariant_id}#behavior:0"],
        }
        review_input = _accepted_input(new_proposal)
        review_input["confirmed_catalog"] = {
            **review_input["confirmed_catalog"],
            "area": "runtime_safety",
            "owner": "trust-runtime",
        }
        review_input["reviewer_ids"] = ["reviewer-a", "reviewer-b"]
        decision_document = {
            "schema_version": 1,
            "generator": "test-catalog-mapping-review",
            "generator_version": "1",
            "source_commit": "a" * 40,
            "proposal_snapshots": [old_proposal],
            "decisions": [old_decision],
        }

        plan = build_catalog_authority_reassignment(
            proposal=new_proposal,
            review_input=review_input,
            decision_document=decision_document,
            catalog_source=catalog_source,
            invariant_sources={
                old_invariant_id: (
                    f"verification/invariants/bytecode_vm/{old_invariant_id}.toml",
                    _invariant_source(old_invariant_id, [test_id, "TEST_KEEP"]),
                ),
                new_invariant_id: (
                    f"verification/invariants/runtime_safety/{new_invariant_id}.toml",
                    _invariant_source(new_invariant_id, []),
                ),
            },
            invariants=invariants,
            expected_test_id=test_id,
            expected_old_invariant_ids=[old_invariant_id],
            reviewed_date="2026-08-13",
            source_commit="b" * 40,
        )

        catalog = tomllib.loads(
            plan.replacements["verification/test-catalog.toml"].decode()
        )["tests"]
        self.assertEqual(1, len(catalog))
        self.assertEqual(test_id, catalog[0]["id"])
        self.assertEqual("runtime_safety", catalog[0]["area"])
        self.assertEqual([new_invariant_id], catalog[0]["invariants"])
        self.assertNotEqual(
            old_record["mapping_decision_id"],
            catalog[0]["mapping_decision_id"],
        )
        old_invariant = tomllib.loads(
            plan.replacements[
                f"verification/invariants/bytecode_vm/{old_invariant_id}.toml"
            ].decode()
        )
        new_invariant = tomllib.loads(
            plan.replacements[
                f"verification/invariants/runtime_safety/{new_invariant_id}.toml"
            ].decode()
        )
        self.assertEqual(["TEST_KEEP"], old_invariant["tests"])
        self.assertEqual([test_id], new_invariant["tests"])
        decisions = tomllib.loads(
            plan.replacements[
                "verification/test-catalog-mapping-decisions.toml"
            ].decode()
        )
        self.assertEqual("b" * 40, decisions["source_commit"])
        self.assertEqual(1, len(decisions["proposal_snapshots"]))
        self.assertEqual(1, len(decisions["decisions"]))
        self.assertEqual(
            catalog[0]["mapping_proposal_id"],
            decisions["proposal_snapshots"][0]["id"],
        )


if __name__ == "__main__":
    unittest.main()
