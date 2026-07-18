"""Protective tests for the combined Phase 5 suite audit report."""

from __future__ import annotations

import copy
import hashlib
import json
import subprocess
import tempfile
import unittest
from contextlib import contextmanager
from pathlib import Path
from unittest import mock

from .phase5_audit_cli import default_command
from .phase5_audit_live import REPORT_SCHEMA_PATH, build_live_phase5_state, validate_source_revision
from .phase5_audit_report import Phase5AuditProvenance, Phase5AuditReport
from .phase5_audit_validation import (
    validate_report_files,
    validate_report_payload,
    validate_schema_contract,
)
from .test_catalog_json_schema import validate_json_schema_instance


ROOT = Path(__file__).resolve().parents[2]
TIMESTAMP = "2026-07-10T12:00:00+00:00"


class Phase5AuditReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.state = build_live_phase5_state(ROOT, timestamp=TIMESTAMP)

    def test_live_state_has_reviewed_phase5_denominators(self) -> None:
        state = self.state
        self.assertEqual(62, len(state.inventory_rows))
        self.assertEqual(59, sum(row["discovery_id"] is not None for row in state.inventory_rows))
        self.assertEqual(6, len(state.suite_rows))
        self.assertEqual(11, len(state.area_rows))
        self.assertEqual(29, len(state.route_rows))
        self.assertEqual(list(range(1, 30)), [row["order"] for row in state.route_rows])
        self.assertTrue(
            {
                "scripts/plan_tests.py",
                "scripts/run_verification_focused_tests.py",
                "scripts/verification/focused_test_suite.py",
                "scripts/verification/planner.py",
                "scripts/verification/report_gate.py",
                "scripts/verification_report_gate.py",
                "scripts/verification/test_catalog_json_schema.py",
                "scripts/verification/test_catalog_validation.py",
            }.issubset(state.input_paths)
        )
        self.assertEqual(
            {
                "verification_gate_enforcing": True,
                "report_emits_proof": False,
                "report_closes_spec_gaps": False,
                "suite_includes_interpreted": False,
                "p5_000b_remains_open": True,
            },
            state.boundaries,
        )

    def test_report_round_trip_and_summary(self) -> None:
        report = fixture_report(self.state)
        payload = report.to_dict()
        self.assertEqual([], validate_report_payload(payload, expected_state=self.state))
        summary = payload["summary"]
        self.assertEqual(62, summary["inventory_records"])
        self.assertEqual(29, summary["taxonomy_routes"])
        self.assertEqual(11, summary["canonical_areas"])
        self.assertEqual(
            {"assigned": 51, "excluded": 8, "report_only": 1, "supporting": 2},
            keyed_counts(summary["by_disposition"]),
        )

    def test_semantic_tampering_fails_closed(self) -> None:
        baseline = fixture_report(self.state).to_dict()
        cases = (
            ("inventory", lambda p: p["inventory"][0].update(enforcement="excluded"), "live Phase 5 state"),
            ("suite", lambda p: p["suites"][0]["direct_commands"].append("echo forged"), "live Phase 5 state"),
            ("area", lambda p: p["areas"][0].update(id="invented"), "live Phase 5 state"),
            ("route", lambda p: p["routes"][0].update(order=29), "live Phase 5 state"),
            (
                "boundary",
                lambda p: p["boundaries"].update(report_emits_proof=True),
                "Phase 5 enforcement boundary contract",
            ),
            ("summary", lambda p: p["summary"].update(inventory_records=60), "summary does not match"),
        )
        for label, mutate, expected in cases:
            with self.subTest(label=label):
                payload = copy.deepcopy(baseline)
                mutate(payload)
                failures = validate_report_payload(payload, expected_state=self.state)
                self.assertTrue(any(expected in failure for failure in failures), failures)

    def test_schema_contract_is_closed_and_drift_pinned(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        payload = fixture_report(self.state).to_dict()
        self.assertEqual([], validate_schema_contract(schema))
        self.assertEqual([], validate_json_schema_instance(payload, schema))

        schema["$defs"]["boundary"]["properties"].pop("report_emits_proof")
        failures = validate_schema_contract(schema)
        self.assertTrue(any("boundary fields drift" in failure for failure in failures), failures)

    def test_at_rest_rejects_noncanonical_json_markdown_and_digest_tampering(self) -> None:
        report = fixture_report(self.state)
        payload = report.to_dict()
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            json_path = root / "report.json"
            markdown_path = root / "report.md"
            schema_path = ROOT / REPORT_SCHEMA_PATH
            canonical = report.to_json().encode()
            digest = hashlib.sha256(canonical).hexdigest()
            json_path.write_bytes(canonical)
            markdown_path.write_text(report.to_markdown(json_digest=digest))

            with patched_live_validation(self.state):
                self.assertEqual(
                    [],
                    validate_report_files(
                        ROOT,
                        json_path,
                        markdown_path,
                        schema_path,
                        allow_external_test_outputs=True,
                    ),
                )

                json_path.write_text(json.dumps(payload) + "\n")
                failures = validate_report_files(
                    ROOT, json_path, markdown_path, schema_path, allow_external_test_outputs=True
                )
                self.assertTrue(any("canonical serialization" in failure for failure in failures), failures)

                json_path.write_bytes(canonical)
                markdown_path.write_text(report.to_markdown(json_digest="0" * 64))
                failures = validate_report_files(
                    ROOT, json_path, markdown_path, schema_path, allow_external_test_outputs=True
                )
                self.assertTrue(any("Markdown" in failure for failure in failures), failures)

    def test_source_revision_requires_clean_full_sha_and_bound_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "test@example.com"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "Test"], check=True)
            (root / "input.txt").write_text("one\n")
            subprocess.run(["git", "-C", str(root), "add", "input.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "fixture"], check=True)
            commit = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()

            self.assertEqual([], validate_source_revision(root, commit, ("input.txt",)))
            self.assertTrue(validate_source_revision(root, "dirty:" + commit, ("input.txt",)))
            (root / "input.txt").write_text("two\n")
            self.assertTrue(
                any("differ" in failure for failure in validate_source_revision(root, commit, ("input.txt",)))
            )

    def test_cli_entrypoints_are_directly_runnable(self) -> None:
        for script in (
            "scripts/report_phase5_suite_audit.py",
            "scripts/validate_phase5_suite_audit_report.py",
        ):
            result = subprocess.run(["python3", script, "--help"], capture_output=True, text=True)
            self.assertEqual(0, result.returncode, result.stderr)

    def test_default_command_binds_outputs_and_timestamp(self) -> None:
        self.assertEqual(
            (
                "python3",
                "scripts/report_phase5_suite_audit.py",
                "--json-out",
                "target/gate-artifacts/verification/phase5-suite-audit.json",
                "--markdown-out",
                "target/gate-artifacts/verification/phase5-suite-audit.md",
                "--timestamp",
                TIMESTAMP,
            ),
            default_command(
                Path("target/gate-artifacts/verification/phase5-suite-audit.json"),
                Path("target/gate-artifacts/verification/phase5-suite-audit.md"),
                TIMESTAMP,
            ),
        )


def fixture_report(state) -> Phase5AuditReport:
    return Phase5AuditReport(
        provenance=Phase5AuditProvenance(
            command=default_command(
                Path("target/gate-artifacts/verification/phase5-suite-audit.json"),
                Path("target/gate-artifacts/verification/phase5-suite-audit.md"),
                TIMESTAMP,
            ),
            commit="a" * 40,
            timestamp=TIMESTAMP,
            platform="test-platform",
            input_paths=state.input_paths,
            output_json="target/gate-artifacts/verification/phase5-suite-audit.json",
            output_markdown="target/gate-artifacts/verification/phase5-suite-audit.md",
        ),
        input_digest=state.input_digest,
        inventory=state.inventory_rows,
        suites=state.suite_rows,
        areas=state.area_rows,
        routes=state.route_rows,
        boundaries=state.boundaries,
    )


def keyed_counts(rows: list[dict]) -> dict[str, int]:
    return {row["name"]: row["count"] for row in rows}


@contextmanager
def patched_live_validation(state):
    with (
        mock.patch(
            "scripts.verification.phase5_audit_validation.build_live_phase5_state",
            return_value=state,
        ),
        mock.patch(
            "scripts.verification.phase5_audit_validation.validate_source_revision",
            return_value=[],
        ),
        mock.patch(
            "scripts.verification.phase5_audit_validation.input_digest",
            return_value=state.input_digest,
        ),
    ):
        yield


if __name__ == "__main__":
    unittest.main()
