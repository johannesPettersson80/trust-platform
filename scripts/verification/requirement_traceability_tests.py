"""Focused tests for explicit Phase 6 traceability analysis."""

from __future__ import annotations

import copy
import unittest

from scripts.verification.metadata_validator.core import Validator
from scripts.verification.requirement_traceability import analyze_requirement_traceability


def loaded_validator() -> Validator:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise AssertionError([failure.message for failure in validator.failures])
    return validator


def analyze(validator: Validator) -> dict:
    return analyze_requirement_traceability(
        invariants=validator.invariants,
        tests=validator.tests,
        suites=validator.suites,
        evidence=validator.evidence,
        spec_sources=validator.spec_sources,
    )


class RequirementTraceabilityTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = loaded_validator()
        cls.analysis = analyze(cls.validator)

    def test_forward_denominator_is_every_invariant_and_uses_exact_backlinks(self) -> None:
        rows = {row["invariant_id"]: row for row in self.analysis["forward_traceability"]}
        self.assertEqual(set(self.validator.invariants), set(rows))
        for invariant_id, invariant in self.validator.invariants.items():
            with self.subTest(invariant_id=invariant_id):
                row = rows[invariant_id]
                self.assertTrue(set(invariant["tests"]).issubset(row["test_ids"]))
                for test_id in row["test_ids"]:
                    self.assertIn(invariant_id, self.validator.tests[test_id]["invariants"])
                self.assertTrue(set(invariant["evidence_refs"]).issubset(row["evidence_ids"]))

    def test_reverse_denominator_is_every_registered_public_claim(self) -> None:
        expected = {
            source_id
            for source_id, source in self.validator.spec_sources.items()
            if source["authority"] == "public_claim"
        }
        rows = self.analysis["reverse_public_claim_traceability"]
        self.assertEqual(expected, {row["public_claim_id"] for row in rows})
        for row in rows:
            for invariant_id in row["invariant_ids"]:
                self.assertIn(
                    row["public_claim_id"],
                    self.validator.invariants[invariant_id]["spec"]["source_refs"],
                )

    def test_orphan_partitions_are_exhaustive_and_identity_bound(self) -> None:
        forward = self.analysis["forward_traceability"]
        referenced_sources = {item for row in forward for item in row["spec_source_ids"]}
        self.assertEqual(
            set(self.validator.spec_sources),
            referenced_sources | set(self.analysis["orphans"]["spec_source_ids"]),
        )
        linked_evidence = {
            evidence_id
            for evidence_id, evidence in self.validator.evidence.items()
            if evidence.get("linked_invariants")
            or evidence.get("linked_tests")
            or evidence.get("linked_spec_gaps")
        }
        self.assertEqual(
            set(self.validator.evidence),
            linked_evidence | set(self.analysis["orphans"]["evidence_ids"]),
        )
        self.assertFalse(linked_evidence & set(self.analysis["orphans"]["evidence_ids"]))

    def test_names_paths_and_titles_cannot_create_trace_edges(self) -> None:
        validator = loaded_validator()
        before = analyze(validator)
        decoy = copy.deepcopy(next(iter(validator.evidence.values())))
        decoy["id"] = "EVID_DECOY_TRACE_NAME"
        decoy["title"] = next(iter(validator.invariants))
        decoy["path"] = "docs/specs/11-runtime-engine.md"
        decoy["linked_invariants"] = []
        decoy["linked_tests"] = []
        decoy["linked_spec_gaps"] = []
        validator.evidence[decoy["id"]] = decoy
        after = analyze(validator)
        self.assertEqual(
            before["forward_traceability"],
            after["forward_traceability"],
        )
        self.assertIn(decoy["id"], after["orphans"]["evidence_ids"])

    def test_unknown_explicit_links_fail_closed(self) -> None:
        validator = loaded_validator()
        invariant_id = next(iter(validator.invariants))
        invariant = copy.deepcopy(validator.invariants[invariant_id])
        invariant["tests"] = [*invariant["tests"], "TEST_UNKNOWN_TRACE"]
        validator.invariants[invariant_id] = invariant
        with self.assertRaisesRegex(ValueError, "unknown test"):
            analyze(validator)


if __name__ == "__main__":
    unittest.main()
