"""Focused tests for the Phase 12 workflow and UI audit."""

from __future__ import annotations

import copy
import json
import unittest
from pathlib import Path

from scripts.verification.phase12_audit import (
    BOUNDARIES,
    LIMITATIONS,
    build_payload,
    build_rows,
    build_summary,
)
from scripts.verification.phase12_audit_live import SCHEMA_PATH
from scripts.verification.phase12_audit_validation import validate_payload, validate_schema
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


class Phase12AuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.reviews = [
            {
                "discovery_id": "WORKFLOW_A",
                "path": "docs/public/a.md",
                "heading_path": ["A"],
                "disposition": "workflow_spec",
                "spec_source_id": "SPEC_A",
            },
            {
                "discovery_id": "WORKFLOW_B",
                "path": "docs/public/b.md",
                "heading_path": ["B"],
                "disposition": "workflow_spec",
                "spec_source_id": "SPEC_B",
            },
            {
                "discovery_id": "NONWORKFLOW",
                "path": "docs/public/c.md",
                "heading_path": ["C"],
                "disposition": "reviewed_nonworkflow",
            },
        ]
        self.journeys = [
            journey(
                "J-A",
                status="provisional",
                workflows=["WORKFLOW_A"],
                invariants=["UI_A"],
                tests=["TEST_A"],
            ),
            journey(
                "J-B",
                status="evidence_missing",
                workflows=["WORKFLOW_B"],
                invariants=[],
                tests=["TEST_B"],
            ),
        ]
        self.workflows, self.rows = build_rows(self.reviews, self.journeys)
        self.payload = build_payload(
            commit="a" * 40,
            timestamp="2026-07-19T00:00:00+02:00",
            platform="linux-x86_64",
            input_paths=["docs/public/a.md"],
            input_digest="sha256:" + "b" * 64,
            output_json="target/report.json",
            output_markdown="docs/report.md",
            command=[
                "python3",
                "scripts/report_phase12_workflow_ui_audit.py",
                "--json-out",
                "target/report.json",
                "--markdown-out",
                "docs/report.md",
                "--timestamp",
                "2026-07-19T00:00:00+02:00",
            ],
            workflow_rows=self.workflows,
            journey_rows=self.rows,
        )

    def test_partition_exposes_missing_links_without_promoting_evidence(self) -> None:
        by_id = {row["discovery_id"]: row for row in self.workflows}
        self.assertEqual(by_id["WORKFLOW_A"]["acceptance_status"], "provisional")
        self.assertFalse(by_id["WORKFLOW_A"]["missing_invariant_link"])
        self.assertEqual(by_id["WORKFLOW_B"]["acceptance_status"], "missing")
        self.assertTrue(by_id["WORKFLOW_B"]["missing_invariant_link"])
        self.assertTrue(by_id["WORKFLOW_B"]["missing_acceptance_evidence"])
        self.assertEqual(by_id["NONWORKFLOW"]["acceptance_status"], "not_applicable")
        self.assertEqual(self.payload["boundaries"], BOUNDARIES)
        self.assertEqual(self.payload["limitations"], list(LIMITATIONS))

    def test_backend_support_without_visual_is_reported_not_accepted(self) -> None:
        by_id = {row["id"]: row for row in self.rows}
        self.assertFalse(by_id["J-A"]["backend_support_without_fresh_visual"])
        self.assertTrue(by_id["J-B"]["backend_support_without_fresh_visual"])
        self.assertEqual(self.payload["summary"]["backend_support_without_fresh_visual"], 1)

    def test_semantic_tampering_fails_closed(self) -> None:
        valid = self._expanded_payload()
        self.assertEqual(validate_payload(valid), [])

        tampered = copy.deepcopy(valid)
        tampered["journey_rows"][0]["fresh_visual_evidence"] = False
        self.assertIn("fresh visual flag is inconsistent", "\n".join(validate_payload(tampered)))

        tampered = copy.deepcopy(valid)
        tampered["summary"]["journeys"] = 999
        self.assertIn("summary does not match", "\n".join(validate_payload(tampered)))

    def test_committed_schema_is_closed_and_matches_validator(self) -> None:
        schema = json.loads((Path.cwd() / SCHEMA_PATH).read_text(encoding="utf-8"))
        self.assertEqual(validate_schema(schema), [])

        payload = self._expanded_payload()
        self.assertEqual(validate_json_schema_instance(payload, schema), [])

    def _expanded_payload(self) -> dict:
        payload = copy.deepcopy(self.payload)
        workflow_template = self.workflows[0]
        journey_template = self.rows[0]
        payload["workflow_rows"] = []
        for index in range(47):
            row = copy.deepcopy(workflow_template)
            row["discovery_id"] = f"WORKFLOW_{index:02d}"
            row["linked_journey_ids"] = []
            payload["workflow_rows"].append(row)
        payload["journey_rows"] = []
        for index in range(30):
            row = copy.deepcopy(journey_template)
            row["id"] = f"J-{index:02d}"
            payload["journey_rows"].append(row)
        payload["summary"] = build_summary(
            payload["workflow_rows"], payload["journey_rows"]
        )
        return payload


def journey(
    journey_id: str,
    *,
    status: str,
    workflows: list[str],
    invariants: list[str],
    tests: list[str],
) -> dict:
    return {
        "id": journey_id,
        "title": journey_id,
        "surface": "VS Code",
        "status": status,
        "journey_source": "batch",
        "workflow_candidate_ids": workflows,
        "invariant_ids": invariants,
        "supporting_test_ids": tests,
        "source_transformation": False,
    }


if __name__ == "__main__":
    unittest.main()
