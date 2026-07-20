"""Focused tests for UI journey acceptance and freshness."""

from __future__ import annotations

import copy
import hashlib
import tempfile
import unittest
from pathlib import Path

from scripts.verification.metadata_validator.core import Validator
from scripts.verification.ui_acceptance import validate_ui_acceptance_document


class UiAcceptanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.evidence = []
        for theme in ("dark", "light", "high_contrast"):
            screenshot = self.root / f"evidence/{theme}.png"
            result = self.root / f"evidence/{theme}.json"
            screenshot.parent.mkdir(parents=True, exist_ok=True)
            screenshot.write_bytes((theme + " screenshot").encode())
            result.write_bytes((theme + " result").encode())
            self.evidence.append(
                {
                    "theme": theme,
                    "screenshot_path": screenshot.relative_to(self.root).as_posix(),
                    "screenshot_sha256": hashlib.sha256(screenshot.read_bytes()).hexdigest(),
                    "result_path": result.relative_to(self.root).as_posix(),
                    "result_sha256": hashlib.sha256(result.read_bytes()).hexdigest(),
                }
            )
        runner = self.root / "runner.js"
        runner.write_text("// runner\n")
        self.tests = {
            "TEST_RENDER": {
                "test_class": "vscode_extension",
                "discovery_source_kind": "vscode_test",
                "suite_tiers": ["pr"],
                "invariants": ["UI_STATUS_001"],
            },
            "TEST_JOURNEY": {
                "test_class": "ui_journey_acceptance",
                "discovery_source_kind": "vscode_test",
                "suite_tiers": ["pr"],
                "invariants": ["UI_STATUS_001"],
            },
        }
        self.invariants = {
            "UI_STATUS_001": {
                "area": "hmi_ui",
                "risk": "false_status",
                "status": "implemented",
                "tests": ["TEST_RENDER", "TEST_JOURNEY"],
            }
        }
        self.workflow_reviews = [
            {
                "discovery_id": "WORKFLOW_CANDIDATE_1",
                "disposition": "workflow_spec",
            }
        ]
        self.journey = {
            "id": "J-01",
            "title": "Visible status failure",
            "surface": "Devices and Connections",
            "status": "provisional",
            "journey_source": "batch",
            "workflow_candidate_ids": ["WORKFLOW_CANDIDATE_1"],
            "invariant_ids": ["UI_STATUS_001"],
            "supporting_test_ids": ["TEST_RENDER"],
            "implementation_paths": ["editors/vscode/src/networkCanvas/panel.ts"],
            "source_transformation": False,
            "runner_paths": ["runner.js"],
            "source_commit": "a" * 40,
            "implementer": "codex",
            "evidence": self.evidence,
            "last_reviewed": "2026-07-18",
        }
        self.document = {
            "schema_version": 1,
            "batch_runner_path": "runner.js",
            "journeys": [self.journey],
        }

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_provisional_triplet_is_valid_but_does_not_accept_invariant(self) -> None:
        self.assertEqual([], self._validate())

        invariant = copy.deepcopy(self.invariants["UI_STATUS_001"])
        invariant["status"] = "validated"
        failures = self._validate(invariants={"UI_STATUS_001": invariant})
        self.assertIn("validated UI invariant requires an ux_accepted journey", "\n".join(failures))

    def test_backend_or_extension_test_cannot_replace_visual_evidence(self) -> None:
        accepted = copy.deepcopy(self.document)
        accepted["journeys"][0]["status"] = "ux_accepted"
        accepted["journeys"][0]["reviewer"] = "independent-reviewer"
        failures = self._validate(accepted)
        self.assertIn("ux_accepted requires a ui_journey_acceptance test", "\n".join(failures))

    def test_accepted_journey_requires_independent_reviewer(self) -> None:
        accepted = copy.deepcopy(self.document)
        row = accepted["journeys"][0]
        row["status"] = "ux_accepted"
        row["supporting_test_ids"] = ["TEST_RENDER", "TEST_JOURNEY"]
        row["reviewer"] = row["implementer"]
        failures = self._validate(accepted)
        self.assertIn("reviewer must differ from implementer", "\n".join(failures))

        row["reviewer"] = "independent-reviewer"
        self.assertEqual([], self._validate(accepted))

    def test_theme_digest_and_visible_source_freshness_fail_closed(self) -> None:
        tampered = copy.deepcopy(self.document)
        tampered["journeys"][0]["evidence"].pop()
        self.assertIn("theme triplet", "\n".join(self._validate(tampered)))

        tampered = copy.deepcopy(self.document)
        tampered["journeys"][0]["evidence"][0]["screenshot_sha256"] = "0" * 64
        self.assertIn("screenshot digest mismatch", "\n".join(self._validate(tampered)))

        failures = self._validate(
            changed={"J-01": ["editors/vscode/src/networkCanvas/panel.ts"]}
        )
        self.assertIn("visible implementation changed after evidence capture", "\n".join(failures))

    def test_stale_status_requires_and_reports_visible_change(self) -> None:
        stale = copy.deepcopy(self.document)
        stale["journeys"][0]["status"] = "stale"
        stale["journeys"][0]["stale_reason"] = "Panel implementation changed."
        changed = {"J-01": ["editors/vscode/src/networkCanvas/panel.ts"]}
        self.assertEqual([], self._validate(stale, changed=changed))
        self.assertIn("stale journey has no changed implementation path", "\n".join(self._validate(stale)))

    def test_batch_denominator_and_workflow_links_are_exact(self) -> None:
        missing = copy.deepcopy(self.document)
        self.assertIn(
            "missing manifest journey for batch runner ID J-02",
            "\n".join(self._validate(missing, batch_ids=["J-01", "J-02"])),
        )

        invented = copy.deepcopy(self.document)
        invented["journeys"][0]["workflow_candidate_ids"] = ["WORKFLOW_INVENTED"]
        self.assertIn("unknown workflow candidate", "\n".join(self._validate(invented)))

    def test_source_transformations_require_silent_corruption_invariant(self) -> None:
        transformed = copy.deepcopy(self.document)
        transformed["journeys"][0]["source_transformation"] = True
        failures = self._validate(transformed)
        self.assertIn("source transformation requires a silent_corruption invariant", "\n".join(failures))

        invariants = copy.deepcopy(self.invariants)
        invariants["UI_STATUS_001"]["risk"] = "silent_corruption"
        self.assertEqual([], self._validate(transformed, invariants=invariants))

    def test_full_metadata_validator_rejects_unreviewed_acceptance_promotion(self) -> None:
        validator = Validator()
        validator.load_records()
        peer = next(
            row
            for row in validator.ui_acceptance["journeys"]
            if row["id"] == "J-PEER-TOPOLOGY-FAILURE"
        )
        peer["status"] = "ux_accepted"
        peer["reviewer"] = peer["implementer"]

        validator.validate()

        messages = "\n".join(failure.message for failure in validator.failures)
        self.assertIn("reviewer must differ from implementer", messages)
        self.assertIn("ux_accepted requires a ui_journey_acceptance test", messages)

    def _validate(
        self,
        document=None,
        *,
        tests=None,
        invariants=None,
        batch_ids=None,
        changed=None,
    ) -> list[str]:
        return validate_ui_acceptance_document(
            self.root,
            document or self.document,
            tests=tests or self.tests,
            invariants=invariants or self.invariants,
            workflow_reviews=self.workflow_reviews,
            batch_journey_ids=batch_ids or ["J-01"],
            changed_paths_by_journey=changed,
        )


if __name__ == "__main__":
    unittest.main()
