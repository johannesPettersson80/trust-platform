"""Focused tests for the exhaustive public-workflow review ledger."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.core import Validator
from scripts.verification.public_workflow_inventory import (
    discover_candidates_from_sources,
    validate_public_workflow_inventory,
)


SOURCES = {
    "docs/public/start.md": """# Start

## Quick Start

1. Open the project.
2. Run it.

## Architecture Order

1. Parse.
2. Lower.
""",
}


class PublicWorkflowInventoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.candidates = discover_candidates_from_sources(SOURCES)
        self.assertEqual(2, len(self.candidates))
        workflow, nonworkflow = self.candidates
        self.source = {
            "schema_version": 2,
            "id": "SPEC_WORKFLOW_QUICK_START_001",
            "authority": "normative_product",
            "source_status": "active",
            "oracle_eligible": True,
            "path": workflow.path,
            "covers": [f"public_workflow:{workflow.discovery_id}"],
            "actor": "PLC engineer",
            "entry_point": "Open the public quick start",
            "preconditions": ["A project exists"],
            "visible_steps": list(workflow.visible_steps),
            "success_state": "The project is running",
            "failure_status_behavior": "The failing step remains visible",
            "safety_authz_boundaries": ["No unsafe write is implied"],
            "acceptance_evidence": ["JOURNEY_QUICK_START_001"],
        }
        self.document = {
            "schema_version": 1,
            "reviews": [
                {
                    "discovery_id": workflow.discovery_id,
                    "path": workflow.path,
                    "heading_path": list(workflow.heading_path),
                    "disposition": "workflow_spec",
                    "spec_source_id": self.source["id"],
                    "rationale": "The ordered steps describe a user task.",
                },
                {
                    "discovery_id": nonworkflow.discovery_id,
                    "path": nonworkflow.path,
                    "heading_path": list(nonworkflow.heading_path),
                    "disposition": "reviewed_nonworkflow",
                    "rationale": "This is an internal architecture sequence, not a user task.",
                },
            ],
        }

    def test_exact_review_partition_is_accepted(self) -> None:
        self.assertEqual([], self._validate())

    def test_missing_and_invented_candidates_fail(self) -> None:
        missing = copy.deepcopy(self.document)
        missing["reviews"].pop()
        self.assertIn("missing review", "\n".join(self._validate(missing)))

        invented = copy.deepcopy(self.document)
        invented["reviews"][0]["discovery_id"] = "WORKFLOW_CANDIDATE_INVENTED"
        failures = "\n".join(self._validate(invented))
        self.assertIn("invented review", failures)
        self.assertIn("missing review", failures)

    def test_identity_and_closed_fields_fail(self) -> None:
        tampered = copy.deepcopy(self.document)
        tampered["reviews"][0]["path"] = "docs/public/other.md"
        tampered["reviews"][0]["inferred_from_name"] = True
        failures = "\n".join(self._validate(tampered))
        self.assertIn("path does not match discovered candidate", failures)
        self.assertIn("unexpected fields", failures)

    def test_workflow_requires_exact_complete_spec_source(self) -> None:
        for mutation, signal in (
            (("visible_steps", ["Invented step"]), "visible_steps do not match"),
            (("authority", "public_claim"), "active oracle-eligible normative_product"),
            (("acceptance_evidence", []), "complete workflow fields"),
        ):
            with self.subTest(field=mutation[0]):
                source = copy.deepcopy(self.source)
                source[mutation[0]] = mutation[1]
                self.assertIn(signal, "\n".join(self._validate(spec_source=source)))

    def test_nonworkflow_forbids_spec_binding(self) -> None:
        tampered = copy.deepcopy(self.document)
        tampered["reviews"][1]["spec_source_id"] = self.source["id"]
        self.assertIn(
            "reviewed_nonworkflow forbids spec_source_id",
            "\n".join(self._validate(tampered)),
        )

    def test_line_movement_does_not_change_identity(self) -> None:
        moved = {key: "\n\n" + value for key, value in SOURCES.items()}
        moved_candidates = discover_candidates_from_sources(moved)
        self.assertEqual(
            [row.discovery_id for row in self.candidates],
            [row.discovery_id for row in moved_candidates],
        )
        self.assertNotEqual(
            [row.line for row in self.candidates],
            [row.line for row in moved_candidates],
        )

    def test_full_metadata_validator_consumes_live_inventory(self) -> None:
        validator = Validator()
        validator.load_records()
        validator.public_workflow_inventory = copy.deepcopy(
            validator.public_workflow_inventory
        )
        validator.public_workflow_inventory["reviews"].pop()

        validator.validate()

        self.assertTrue(
            any(
                "missing review for WORKFLOW_CANDIDATE_28057E7E1763569B539C8DB3"
                in failure.message
                for failure in validator.failures
            )
        )

    def _validate(self, document=None, *, spec_source=None) -> list[str]:
        source = spec_source or self.source
        return validate_public_workflow_inventory(
            Path.cwd(),
            document or self.document,
            spec_sources={source["id"]: source},
            candidates=self.candidates,
        )


if __name__ == "__main__":
    unittest.main()
