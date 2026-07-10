"""Protective tests for Phase 4A spec-gap close-out rules."""

from __future__ import annotations

import unittest

from .metadata_validator.spec_gap_closure import validate_spec_gap_closure


def source(
    source_id: str = "SPEC_OWNER",
    *,
    authority: str = "normative_product",
    source_status: str = "active",
) -> dict:
    return {
        "id": source_id,
        "authority": authority,
        "source_status": source_status,
        "path": "docs/specs/owner.md",
        "last_reviewed": "2026-07-10",
    }


def gap(**overrides: object) -> dict:
    record = {
        "id": "SPEC_GAP_X",
        "resolution_status": "closed",
        "candidate_spec_sources": ["SPEC_OWNER"],
        "resolution_source_ref": "SPEC_OWNER",
        "affected_invariants": ["INV_X"],
        "affected_tests": ["TEST_X"],
        "closeout_evidence": ["EVID_X"],
        "last_reviewed": "2026-07-10",
    }
    record.update(overrides)
    return record


def invariant(**overrides: object) -> dict:
    record = {
        "id": "INV_X",
        "risk": "safety_critical",
        "status": "spec_gap",
        "spec": {"status": "missing"},
        "spec_gap_refs": ["SPEC_GAP_X"],
        "coverage": {
            "cells": [
                {
                    "dimension": "hardware_or_network_fault",
                    "state": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_X",
                }
            ]
        },
    }
    record.update(overrides)
    return record


def closeout_evidence(*, linked_tests: list[str] | None = None) -> dict:
    return {
        "id": "EVID_X",
        "linked_spec_gaps": ["SPEC_GAP_X"],
        "linked_tests": ["TEST_X"] if linked_tests is None else linked_tests,
    }


class SpecGapClosureTests(unittest.TestCase):
    def validate(
        self,
        *,
        spec_gaps: dict[str, dict] | None = None,
        spec_sources: dict[str, dict] | None = None,
        tests: dict[str, dict] | None = None,
        evidence: dict[str, dict] | None = None,
        invariants: dict[str, dict] | None = None,
        required_specs: dict[str, dict] | None = None,
        risks: dict[str, dict] | None = None,
    ) -> list[str]:
        return validate_spec_gap_closure(
            spec_gaps=spec_gaps or {},
            spec_sources=spec_sources or {},
            tests=tests or {},
            evidence=evidence or {},
            invariants=invariants or {},
            required_specs=required_specs or {},
            risks=risks or {},
        )

    def test_open_gap_does_not_require_closeout_fields(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap(
                resolution_status="open",
                resolution_source_ref=None,
                affected_tests=[],
                closeout_evidence=[],
            )},
            invariants={"INV_X": invariant()},
        )
        self.assertEqual([], failures)

    def test_closed_gap_requires_owning_source_test_disposition_and_evidence(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap(
                resolution_source_ref=None,
                affected_tests=[],
                closeout_evidence=[],
            )},
            invariants={"INV_X": invariant(status="gap_open")},
        )
        joined = "\n".join(failures)
        self.assertIn("resolution_source_ref", joined)
        self.assertIn("written mapped test or explicit reviewed deferral", joined)
        self.assertIn("closeout_evidence", joined)

    def test_closed_gap_rejects_public_claim_as_resolution_source(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap()},
            spec_sources={"SPEC_OWNER": source(authority="public_claim")},
            tests={"TEST_X": {"id": "TEST_X", "status": "mapped"}},
            evidence={"EVID_X": closeout_evidence()},
            invariants={"INV_X": invariant(status="gap_open")},
        )
        self.assertTrue(any("cannot close a spec gap" in item for item in failures))

    def test_closed_gap_accepts_active_owner_and_written_mapped_test(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap()},
            spec_sources={"SPEC_OWNER": source()},
            tests={"TEST_X": {"id": "TEST_X", "status": "mapped"}},
            evidence={"EVID_X": closeout_evidence()},
            invariants={"INV_X": invariant(
                risk="wrong_result",
                status="gap_open",
                spec={"status": "specified"},
                spec_gap_refs=[],
                coverage={"cells": [{"dimension": "happy_path", "state": "gap_open"}]},
            )},
        )
        self.assertEqual([], failures)

    def test_closed_gap_accepts_explicit_active_reviewed_test_deferral(self) -> None:
        deferred_gap = gap(
            affected_tests=[],
            test_deferral_ref="DECISION_DEFER",
        )
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": deferred_gap},
            spec_sources={
                "SPEC_OWNER": source(),
                "DECISION_DEFER": source(
                    "DECISION_DEFER",
                    authority="reviewed_decision",
                ),
            },
            evidence={"EVID_X": closeout_evidence(linked_tests=[])},
            invariants={"INV_X": invariant(
                risk="wrong_result",
                status="gap_open",
                spec={"status": "specified"},
                spec_gap_refs=[],
                coverage={"cells": [{"dimension": "happy_path", "state": "gap_open"}]},
            )},
        )
        self.assertEqual([], failures)

    def test_closed_gap_rejects_planned_test_as_written(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap()},
            spec_sources={"SPEC_OWNER": source()},
            tests={"TEST_X": {"id": "TEST_X", "status": "planned"}},
            evidence={"EVID_X": closeout_evidence()},
            invariants={"INV_X": invariant(status="gap_open")},
        )
        self.assertTrue(any("is not a written mapped test" in item for item in failures))

    def test_closed_gap_rejects_all_remaining_metadata_references(self) -> None:
        clean_invariant = invariant(
            risk="wrong_result",
            status="gap_open",
            spec={"status": "specified"},
            spec_gap_refs=["SPEC_GAP_X"],
            behavior=[{"spec_gap_ref": "SPEC_GAP_X"}],
            coverage={
                "cells": [
                    {
                        "dimension": "happy_path",
                        "state": "spec_gap",
                        "spec_gap_ref": "SPEC_GAP_X",
                    }
                ]
            },
        )
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap()},
            spec_sources={"SPEC_OWNER": source()},
            tests={
                "TEST_X": {"id": "TEST_X", "status": "mapped"},
                "TEST_REF": {
                    "id": "TEST_REF",
                    "status": "mapped",
                    "spec_gap_ref": "SPEC_GAP_X",
                },
            },
            evidence={"EVID_X": closeout_evidence()},
            invariants={"INV_X": clean_invariant},
            required_specs={"REQ_X": {"id": "REQ_X", "spec_gap_ref": "SPEC_GAP_X"}},
            risks={"RISK_X": {"id": "RISK_X", "related_spec_gaps": ["SPEC_GAP_X"]}},
        )
        joined = "\n".join(failures)
        for marker in (
            "required_spec:REQ_X",
            "invariant:INV_X:spec_gap_refs",
            "invariant:INV_X:coverage",
            "invariant:INV_X:behavior",
            "test:TEST_REF",
            "risk:RISK_X",
        ):
            self.assertIn(marker, joined)

    def test_closed_gap_resolution_source_requires_workspace_path(self) -> None:
        owner = source()
        owner["path"] = "https://example.invalid/owner"
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap()},
            spec_sources={"SPEC_OWNER": owner},
            tests={"TEST_X": {"id": "TEST_X", "status": "mapped"}},
            evidence={"EVID_X": closeout_evidence()},
            invariants={"INV_X": invariant(
                risk="wrong_result",
                status="gap_open",
                spec={"status": "specified"},
                spec_gap_refs=[],
                coverage={"cells": [{"dimension": "happy_path", "state": "gap_open"}]},
            )},
        )
        self.assertTrue(any("external-reference-only" in item for item in failures))

    def test_malformed_optional_reference_lists_fail_without_crashing(self) -> None:
        malformed = gap(candidate_spec_sources=None)
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": malformed},
            spec_sources={"SPEC_OWNER": source()},
            tests={"TEST_X": {"id": "TEST_X", "status": "mapped"}},
            evidence={
                "EVID_X": {
                    "id": "EVID_X",
                    "linked_spec_gaps": None,
                    "linked_tests": None,
                }
            },
            invariants={"INV_X": invariant(
                risk="wrong_result",
                status="gap_open",
                spec={"status": "specified"},
                spec_gap_refs=None,
                behavior=None,
                coverage={"cells": None},
            )},
            risks={"RISK_X": {"related_spec_gaps": None}},
        )
        self.assertTrue(failures)

    def test_safety_critical_validated_is_blocked_by_open_gap_reference(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap(resolution_status="open")},
            invariants={"INV_X": invariant(
                status="validated",
                spec={"status": "specified"},
                coverage={"cells": [{"dimension": "happy_path", "state": "covered"}]},
            )},
        )
        self.assertTrue(any("safety-critical validated" in item for item in failures))

    def test_safety_critical_validated_is_blocked_by_reverse_gap_link(self) -> None:
        failures = self.validate(
            spec_gaps={"SPEC_GAP_X": gap(resolution_status="open")},
            invariants={"INV_X": invariant(
                status="validated",
                spec={"status": "specified"},
                spec_gap_refs=[],
                coverage={"cells": [{"dimension": "happy_path", "state": "covered"}]},
            )},
        )
        self.assertTrue(any("SPEC_GAP_X" in item for item in failures))


if __name__ == "__main__":
    unittest.main()
