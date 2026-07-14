"""Tests for the report-only Phase 8 runtime-anomaly audit."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.runtime_anomaly_contract import (
    TAXONOMY_PATH,
    load_runtime_anomaly_taxonomy,
)
from scripts.verification.runtime_anomaly_cli import _validated_output_path
from scripts.verification.runtime_anomaly_live import (
    REPORT_SCHEMA_PATH,
    REQUIRED_OPEN_ROWS,
    build_live_runtime_anomaly_state,
    validate_open_board_rows,
    validate_source_revision,
)
from scripts.verification.runtime_anomaly_report import (
    RuntimeAnomalyProvenance,
    RuntimeAnomalyReport,
    render_markdown,
)
from scripts.verification.runtime_anomaly_report_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from scripts.verification.runtime_anomaly_validation import validate_report_files


SCHEMA_PATH = ROOT / REPORT_SCHEMA_PATH
REPORT_JSON = "target/gate-artifacts/verification/runtime-anomaly-audit.json"
REPORT_MARKDOWN = (
    "docs/internal/testing/evidence/plc-verification-program/2026-07-11/"
    "p8-runtime-anomaly-audit.md"
)
EXPECTED_SUMMARY = {
    "taxonomy_classes": 19,
    "mapping_records": 38,
    "scanner_denominator": 3101,
    "effectively_runnable_mappings": 28,
    "ignored_or_conditional_mappings": 1,
    "gap_classes": 9,
    "by_state": {
        "mapped_runnable": 10,
        "mapped_non_runnable_or_partial": 4,
        "unmapped": 5,
    },
    "by_primary_suite": {"pr": 9, "nightly": 8, "release": 2, "hardware_lab": 0},
    "by_association_kind": {
        "direct": 28,
        "partial": 7,
        "protective_red": 0,
        "context_only": 3,
    },
}


def resolved_restart_review() -> dict[str, object]:
    return {
        "outcome": "resolved_source",
        "source_ref": "SPEC_IEC_STANDARD_FBS_CANDIDATE_001",
        "source_path": "docs/specs/08-standard-function-blocks.md",
        "superseded_gap_id": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
        "rationale": "The reviewed product source now owns restart and time-base semantics.",
    }


def report_for(state) -> RuntimeAnomalyReport:
    return RuntimeAnomalyReport(
        provenance=RuntimeAnomalyProvenance(
            command=(
                "python3",
                "scripts/report_runtime_anomaly_audit.py",
                "--json-out",
                REPORT_JSON,
                "--markdown-out",
                REPORT_MARKDOWN,
                "--timestamp",
                state.timestamp,
            ),
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json=REPORT_JSON,
            output_markdown=REPORT_MARKDOWN,
        ),
        input_digest=state.input_digest,
        spec_gap_reviews=state.spec_gap_reviews,
        analysis=state.analysis,
    )


class RuntimeAnomalyReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.state = build_live_runtime_anomaly_state(
            ROOT,
            timestamp="2026-07-11T14:00:00+02:00",
        )
        cls.analysis = cls.state.analysis
        cls.report = report_for(cls.state)

    def test_live_taxonomy_and_mapping_census_is_explicit(self) -> None:
        summary = self.analysis["summary"]

        self.assertEqual(EXPECTED_SUMMARY, summary)
        self.assertEqual(
            summary["taxonomy_classes"],
            sum(summary["by_state"].values()),
        )
        self.assertEqual(
            summary["mapping_records"],
            sum(summary["by_association_kind"].values()),
        )
        self.assertEqual(summary["gap_classes"], len(self.analysis["gap_rows"]))
        self.assertEqual(
            [row["class_id"] for row in self.analysis["gap_rows"]],
            [
                row["class_id"]
                for row in self.analysis["classes"]
                if row["state"] != "mapped_runnable"
            ],
        )

    def test_expected_direct_partial_and_unmapped_posture(self) -> None:
        classes = {row["class_id"]: row for row in self.analysis["classes"]}

        for class_id in (
            "panic",
            "timeout",
            "deadline",
            "watchdog",
            "slow_device",
            "disconnect",
            "stale_data",
            "corrupt_retain",
            "malformed_bytecode",
            "bad_config",
        ):
            self.assertEqual("mapped_runnable", classes[class_id]["state"])
        for class_id in (
            "queue_full",
            "partial_web_request",
            "disk_error",
            "clock_step",
        ):
            self.assertEqual(
                "mapped_non_runnable_or_partial",
                classes[class_id]["state"],
            )
        for class_id in (
            "bad_signal",
            "monotonic_wall_clock_divergence",
            "suspend_resume",
            "timer_duration_overflow",
            "allocation_failure_oom",
        ):
            self.assertEqual("unmapped", classes[class_id]["state"])

    def test_ignored_associations_never_count_as_runnable(self) -> None:
        ignored = [
            row
            for row in self.analysis["mappings"]
            if row["ignore_state"] != "not_ignored"
        ]

        self.assertTrue(ignored)
        self.assertTrue(all(row["ignored_registry_id"] for row in ignored))
        self.assertTrue(all(not row["effectively_runnable"] for row in ignored))

    def test_p8_001a_reuses_written_sources_and_resolved_gap(self) -> None:
        reviews = self.state.spec_gap_reviews

        allocation = reviews["scan_cycle_allocation_policy"]
        self.assertEqual("written_contract_present", allocation["outcome"])
        self.assertEqual("SPEC_RUNTIME_ENGINE_001", allocation["source_ref"])
        restart = reviews["restart_timebase"]
        self.assertEqual("resolved_source", restart["outcome"])
        self.assertEqual(
            "SPEC_IEC_STANDARD_FBS_CANDIDATE_001",
            restart["source_ref"],
        )
        self.assertEqual(
            "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
            restart["superseded_gap_id"],
        )

    def test_future_resolved_restart_state_flows_through_live_report_contract(self) -> None:
        taxonomy = copy.deepcopy(load_runtime_anomaly_taxonomy(ROOT))
        taxonomy["spec_gap_reviews"]["restart_timebase"] = resolved_restart_review()

        with patch(
            "scripts.verification.runtime_anomaly_live.load_runtime_anomaly_taxonomy",
            return_value=taxonomy,
        ):
            state = build_live_runtime_anomaly_state(
                ROOT,
                timestamp="2026-07-11T14:00:00+02:00",
            )
        report = report_for(state)
        payload = report.to_dict()

        self.assertIn("docs/specs/08-standard-function-blocks.md", state.input_paths)
        self.assertEqual([], validate_report_payload(payload, expected_state=state))
        self.assertIn(
            "resolved_source", report.to_markdown(json_digest="0" * 64)
        )
        self.assertIn(
            "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
            report.to_markdown(json_digest="0" * 64),
        )

    def test_report_rejects_restart_variant_hybrid_and_claim_language(self) -> None:
        hybrid = copy.deepcopy(self.report.to_dict())
        hybrid["spec_gap_reviews"]["restart_timebase"] = {
            **resolved_restart_review(),
            "spec_gap_ref": "SPEC_GAP_IEC_TIMER_RESTART_TIMEBASE_001",
        }
        claim = copy.deepcopy(self.report.to_dict())
        claim["spec_gap_reviews"]["restart_timebase"] = {
            **resolved_restart_review(),
            "rationale": "This source proves full test coverage.",
        }

        self.assertTrue(
            any(
                "restart_timebase report fields drift" in failure
                for failure in validate_report_payload(hybrid)
            )
        )
        self.assertTrue(
            any(
                "forbidden proof/coverage language" in failure
                for failure in validate_report_payload(claim)
            )
        )

    def test_report_contract_is_canonical_and_schema_bound(self) -> None:
        payload = self.report.to_dict()
        json_bytes = self.report.to_json().encode()
        digest = hashlib.sha256(json_bytes).hexdigest()
        markdown = self.report.to_markdown(json_digest=digest)

        self.assertEqual([], validate_schema_contract(json.loads(SCHEMA_PATH.read_text())))
        self.assertEqual([], validate_report_payload(payload, expected_state=self.state))
        self.assertEqual([], validate_markdown_binding(payload, json_bytes, markdown))
        self.assertEqual(
            json.dumps(payload, indent=2, sort_keys=True) + "\n",
            json_bytes.decode(),
        )

    def test_schema_contract_pins_honesty_critical_definitions(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        mutations = []
        boundary = copy.deepcopy(schema)
        boundary["$defs"]["boundaries"]["properties"]["report_creates_proof"] = {
            "const": True
        }
        mutations.append((boundary, "boundary const for report_creates_proof drifts"))
        scope = copy.deepcopy(schema)
        scope["$defs"]["scope"]["properties"]["mapping_basis"] = {"type": "string"}
        mutations.append((scope, "scope const for mapping_basis drifts"))
        digest = copy.deepcopy(schema)
        digest["$defs"]["digest"]["pattern"] = ".*"
        mutations.append((digest, "digest pattern drifts"))
        mapping_id = copy.deepcopy(schema)
        mapping_id["$defs"]["mapping"]["properties"]["mapping_id"]["pattern"] = ".*"
        mutations.append((mapping_id, "mapping ID pattern drifts"))
        ignore_state = copy.deepcopy(schema)
        ignore_state["$defs"]["mapping"]["properties"]["ignore_state"]["enum"].append(
            "unknown"
        )
        mutations.append((ignore_state, "ignore_state enum drifts"))
        count_map = copy.deepcopy(schema)
        count_map["$defs"]["count_map_state"]["required"].remove("unmapped")
        mutations.append((count_map, "count_map_state required fields drift"))
        timestamp = copy.deepcopy(schema)
        timestamp["properties"]["timestamp"] = {"type": "string", "minLength": 1}
        mutations.append((timestamp, "timestamp pattern drifts"))
        restart_union = copy.deepcopy(schema)
        restart_union["$defs"]["restart_review"]["oneOf"].reverse()
        mutations.append((restart_union, "restart review union drifts"))
        restart_ref_sibling = copy.deepcopy(schema)
        restart_ref_sibling["$defs"]["restart_review"]["oneOf"][0]["type"] = (
            "string"
        )
        mutations.append((restart_ref_sibling, "restart review union drifts"))
        resolved_restart = copy.deepcopy(schema)
        resolved_restart["$defs"]["restart_resolved_source_v1"]["properties"][
            "superseded_gap_id"
        ] = {"type": "string"}
        mutations.append(
            (resolved_restart, "resolved restart const for superseded_gap_id drifts")
        )

        for changed, signal in mutations:
            with self.subTest(signal=signal):
                self.assertTrue(
                    any(signal in failure for failure in validate_schema_contract(changed)),
                    validate_schema_contract(changed),
                )

    def test_output_paths_are_contained_before_write(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            outside = root.parent / f"{root.name}-escape.json"
            symlink = root / "linked"
            symlink.symlink_to(root.parent, target_is_directory=True)

            for value in (Path("../phase8-escape.json"), outside, Path("linked/report.json")):
                with self.subTest(value=value):
                    with self.assertRaisesRegex(ValueError, "output path"):
                        _validated_output_path(root, value, "JSON")
            self.assertFalse(outside.exists())

    def test_semantic_tamper_fails_after_consistent_summary_edit(self) -> None:
        payload = copy.deepcopy(self.report.to_dict())
        row = next(item for item in payload["classes"] if item["class_id"] == "panic")
        row["state"] = "unmapped"
        payload["summary"]["by_state"]["mapped_runnable"] -= 1
        payload["summary"]["by_state"]["unmapped"] += 1

        failures = validate_report_payload(payload, expected_state=self.state)

        self.assertTrue(
            any("current runtime-anomaly analysis" in item for item in failures),
            failures,
        )

    def test_at_rest_validation_rejects_non_iso_timestamp(self) -> None:
        payload = copy.deepcopy(self.report.to_dict())
        payload["timestamp"] = "not-a-time"
        payload["command"][-1] = "not-a-time"

        failures = self._validate_payload_at_rest(payload)

        self.assertTrue(any("ISO-8601" in failure for failure in failures), failures)

    def test_at_rest_validation_returns_failures_for_malformed_mapping_ids(self) -> None:
        mutations = (
            ("mapping_id", None, "mapping_id"),
            ("discovery_id", [], "discovery_id"),
        )
        for field, value, signal in mutations:
            with self.subTest(field=field):
                payload = copy.deepcopy(self.report.to_dict())
                payload["mappings"][0][field] = value

                failures = self._validate_payload_at_rest(payload)

                self.assertTrue(any(signal in failure for failure in failures), failures)

    def test_at_rest_validation_returns_failures_for_unhashable_leaf_types(self) -> None:
        mutations = (
            ("classes", "primary_suite", [], "primary_suite"),
            ("classes", "state", {}, "state"),
            ("mappings", "association_kind", [], "association_kind"),
            ("mappings", "ignore_state", {}, "ignore_state"),
            ("gap_rows", "class_id", [], "class_id"),
        )
        for collection, field, value, signal in mutations:
            with self.subTest(collection=collection, field=field):
                payload = copy.deepcopy(self.report.to_dict())
                payload[collection][0][field] = value

                failures = self._validate_payload_at_rest(payload)

                self.assertTrue(any(signal in failure for failure in failures), failures)

    def test_at_rest_validation_returns_failures_for_missing_leaf_fields(self) -> None:
        class_fields = (
            "class_id",
            "mapping_ids",
            "primary_suite",
            "state",
            "title",
        )
        for field in class_fields:
            with self.subTest(collection="classes", field=field):
                payload = copy.deepcopy(self.report.to_dict())
                row = next(
                    item for item in payload["classes"] if item["class_id"] == "queue_full"
                )
                row.pop(field)

                failures = self._validate_payload_at_rest(payload)

                self.assertTrue(failures)

        for field in ("mapping_id", "effectively_runnable"):
            with self.subTest(collection="mappings", field=field):
                payload = copy.deepcopy(self.report.to_dict())
                payload["mappings"][0].pop(field)

                failures = self._validate_payload_at_rest(payload)

                self.assertTrue(failures)

    def _validate_payload_at_rest(self, payload: dict) -> list[str]:
        with tempfile.TemporaryDirectory() as temp:
            json_path = Path(temp) / "report.json"
            markdown_path = Path(temp) / "report.md"
            json_text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
            json_path.write_text(json_text)
            try:
                markdown = render_markdown(
                    payload, json_digest=hashlib.sha256(json_text.encode()).hexdigest()
                )
            except (KeyError, TypeError):
                markdown = "malformed payload cannot be rendered\n"
            markdown_path.write_text(markdown)
            with (
                patch(
                    "scripts.verification.runtime_anomaly_validation."
                    "build_live_runtime_anomaly_state",
                    return_value=self.state,
                ),
                patch(
                    "scripts.verification.runtime_anomaly_validation.validate_source_revision",
                    return_value=[],
                ),
            ):
                return validate_report_files(
                    ROOT,
                    json_path,
                    markdown_path,
                    SCHEMA_PATH,
                    allow_external_test_outputs=True,
                )

    def test_at_rest_validation_rejects_fully_rendered_semantic_tamper(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            json_path = Path(temp) / "report.json"
            markdown_path = Path(temp) / "report.md"
            json_path.write_text(self.report.to_json())
            digest = hashlib.sha256(json_path.read_bytes()).hexdigest()
            markdown_path.write_text(self.report.to_markdown(json_digest=digest))
            with (
                patch(
                    "scripts.verification.runtime_anomaly_validation."
                    "build_live_runtime_anomaly_state",
                    return_value=self.state,
                ),
                patch(
                    "scripts.verification.runtime_anomaly_validation.validate_source_revision",
                    return_value=[],
                ),
            ):
                self.assertEqual(
                    [],
                    validate_report_files(
                        ROOT,
                        json_path,
                        markdown_path,
                        SCHEMA_PATH,
                        allow_external_test_outputs=True,
                    ),
                )
                payload = copy.deepcopy(self.report.to_dict())
                payload["mappings"][0]["assertion_summary"] = "Forged association."
                json_text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
                json_path.write_text(json_text)
                markdown_path.write_text(
                    render_markdown(
                        payload,
                        json_digest=hashlib.sha256(json_text.encode()).hexdigest(),
                    )
                )
                failures = validate_report_files(
                    ROOT,
                    json_path,
                    markdown_path,
                    SCHEMA_PATH,
                    allow_external_test_outputs=True,
                )

        self.assertTrue(
            any("current runtime-anomaly analysis" in item for item in failures),
            failures,
        )

    def test_standing_fault_hook_rows_remain_open(self) -> None:
        board = (
            ROOT
            / "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
        ).read_text()
        self.assertEqual([], validate_open_board_rows(board))
        for row_id in ("VERIF-P8-002", "VERIF-P8-005", "VERIF-P8-006"):
            self.assertIn(row_id, REQUIRED_OPEN_ROWS)
            changed = board.replace(f"- [ ] `{row_id}`", f"- [x] `{row_id}`", 1)
            self.assertTrue(
                any(row_id in failure for failure in validate_open_board_rows(changed))
            )

    def test_source_revision_requires_clean_full_sha(self) -> None:
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_source_revision(ROOT, "dirty:" + "a" * 40, ()),
        )
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_source_revision(ROOT, "a" * 12, ()),
        )

    def test_input_closure_binds_taxonomy_scanner_and_mapped_sources(self) -> None:
        taxonomy = load_runtime_anomaly_taxonomy(ROOT)

        self.assertIn(TAXONOMY_PATH, self.state.input_paths)
        self.assertIn(
            "scripts/verification/test_catalog_rust.py",
            self.state.input_paths,
        )
        for mapping in taxonomy["mappings"]:
            self.assertIn(mapping["path"], self.state.input_paths)


if __name__ == "__main__":
    unittest.main()
