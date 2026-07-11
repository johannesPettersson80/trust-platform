"""At-rest and rendering tests for the Phase 9 fuzz-program report."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from .fuzz_program_cli import _validated_output_path
from .fuzz_program_live import (
    REPORT_SCHEMA_PATH,
    REQUIRED_OPEN_ROWS,
    build_live_fuzz_program_state,
    validate_open_board_rows,
    validate_timestamp,
)
from .fuzz_program_report import FuzzProgramReport, report_from_state, render_markdown
from .fuzz_program_report_contract import validate_report_payload, validate_schema_contract
from .fuzz_program_validation import validate_report_files
from .metadata_validator.constants import ROOT
from .test_catalog_json_schema import validate_json_schema_instance


EXPECTED_STATES = {
    "st_lexer_parser": "cargo_fuzz_target",
    "hir_lowering_input": "partial_only",
    "plcopen_xml": "unmapped",
    "bytecode_container_instructions": "smoke_only",
    "protocol_payloads": "cargo_fuzz_target",
    "config_files": "unmapped",
    "lsp_incremental_edits": "partial_only",
    "hmi_schema_payloads": "unmapped",
}


class FuzzProgramReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.state = build_live_fuzz_program_state(
            ROOT,
            timestamp="2026-07-11T18:00:00+02:00",
            require_clean_commit=False,
        )
        cls.report: FuzzProgramReport = report_from_state(
            cls.state,
            output_json="target/gate-artifacts/verification/fuzz-program-audit.json",
            output_markdown="target/gate-artifacts/verification/fuzz-program-audit.md",
        )
        cls.payload = cls.report.to_dict()

    def test_live_report_reconciles_inventory_and_surface_gaps(self) -> None:
        summary = self.payload["summary"]
        self.assertEqual(11, summary["inventory_targets"])
        self.assertEqual(5, summary["cargo_fuzz_targets"])
        self.assertEqual(6, summary["bounded_rust_smokes"])
        self.assertEqual(8, summary["required_surfaces"])
        self.assertEqual(6, summary["gap_surfaces"])
        self.assertEqual(
            EXPECTED_STATES,
            {row["surface_id"]: row["state"] for row in self.payload["surfaces"]},
        )
        self.assertTrue(
            all(
                row.get("ignore_state") == "not_ignored"
                for row in self.payload["targets"]
                if row["target_kind"] == "bounded_rust_smoke"
            )
        )

    def test_report_is_explicitly_non_proof_and_non_execution(self) -> None:
        boundaries = self.payload["boundaries"]
        self.assertFalse(boundaries["report_creates_proof"])
        self.assertFalse(boundaries["report_creates_invariant_coverage"])
        self.assertFalse(boundaries["fuzz_campaign_executed"])
        self.assertTrue(boundaries["p9_005_crash_regression_row_remains_open"])

    def test_payload_and_schema_validate(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        self.assertEqual([], validate_schema_contract(schema))
        self.assertEqual(
            [],
            validate_report_payload(self.payload, expected_state=self.state),
        )

    def test_schema_const_and_enum_drift_fail(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        schema["$defs"]["boundaries"]["properties"]["report_creates_proof"]["const"] = True
        schema["$defs"]["surface_id"]["enum"].append("invented")
        failures = validate_schema_contract(schema)
        self.assertTrue(any("boundaries consts drifted" in item for item in failures), failures)
        self.assertTrue(any("surface_id enum drifted" in item for item in failures), failures)

    def test_schema_shape_and_pattern_weakening_fail(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        schema["properties"]["generator"].pop("const")
        schema["properties"]["targets"]["minItems"] = 0
        schema["properties"]["command"] = {"type": "string"}
        schema["$defs"]["target"]["required"].remove("command")
        schema["$defs"]["target"]["properties"]["path"] = {"type": "integer"}
        schema["$defs"]["target"]["properties"]["command"].pop("minLength")
        schema["$defs"]["target"]["properties"]["id"] = {"type": "string"}
        schema["$defs"]["surface"]["properties"]["state"] = {"type": "string"}
        schema["$defs"]["surface"]["properties"]["area"] = {"type": "string"}
        schema["properties"]["platform"].pop("minLength")
        schema["$defs"]["digest"]["pattern"] = ".*"
        schema["$defs"]["output_paths"]["properties"]["json"] = {"type": "integer"}
        schema["$defs"]["count_map_tier"]["additionalProperties"] = True
        failures = validate_schema_contract(schema)
        self.assertTrue(any("generator const drifted" in item for item in failures), failures)
        self.assertTrue(any("targets array contract drifted" in item for item in failures), failures)
        self.assertTrue(any("command array contract drifted" in item for item in failures), failures)
        self.assertTrue(any("target required fields drifted" in item for item in failures), failures)
        self.assertTrue(any("target path binding drifted" in item for item in failures), failures)
        self.assertTrue(any("target command binding drifted" in item for item in failures), failures)
        self.assertTrue(any("target id binding drifted" in item for item in failures), failures)
        self.assertTrue(any("surface state binding drifted" in item for item in failures), failures)
        self.assertTrue(any("surface area binding drifted" in item for item in failures), failures)
        self.assertTrue(any("platform contract drifted" in item for item in failures), failures)
        self.assertTrue(any("digest contract drifted" in item for item in failures), failures)
        self.assertTrue(any("output path types drifted" in item for item in failures), failures)
        self.assertTrue(any("count_map_tier must be a closed object" in item for item in failures), failures)
        self.assertTrue(any("semantic digest drifted" in item for item in failures), failures)

    def test_timestamp_requires_real_timezone_aware_iso_value(self) -> None:
        for value in (
            "2026-07-11T18:00:00",
            "2026-99-99T18:00:00+02:00",
            "not-a-time",
        ):
            with self.subTest(value=value):
                with self.assertRaisesRegex(ValueError, "timezone"):
                    validate_timestamp(value)

    def test_semantic_tamper_fails_live_recompute(self) -> None:
        corrupted = copy.deepcopy(self.payload)
        corrupted["surfaces"][2]["state"] = "cargo_fuzz_target"
        corrupted["surfaces"][2]["target_ids"] = ["FUZZ_TARGET_SYNTAX_PARSE"]
        failures = validate_report_payload(corrupted, expected_state=self.state)
        self.assertTrue(any("live Phase 9" in item for item in failures), failures)

    def test_invalid_shapes_return_failures_instead_of_exceptions(self) -> None:
        mutations = []
        for key in self.payload:
            candidate = copy.deepcopy(self.payload)
            candidate.pop(key)
            mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["targets"] = None
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["summary"]["inventory_targets"] = "eleven"
        mutations.append(candidate)
        for payload in mutations:
            try:
                failures = validate_report_payload(payload, expected_state=self.state)
            except Exception as exc:  # pragma: no cover - assertion explains the contract
                self.fail(f"invalid payload raised {type(exc).__name__}: {exc}")
            self.assertTrue(failures)

    def test_recursive_type_and_key_mutations_never_raise_or_validate(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        mutations: list[dict] = []

        def visit(value: object, path: tuple[object, ...]) -> None:
            if len(mutations) >= 1200:
                return
            if isinstance(value, dict):
                for key, child in value.items():
                    candidate = copy.deepcopy(self.payload)
                    parent = candidate
                    for part in path:
                        parent = parent[part]
                    parent.pop(key, None)
                    mutations.append(candidate)
                    visit(child, (*path, key))
            elif isinstance(value, list):
                for index, child in enumerate(value[:12]):
                    visit(child, (*path, index))
            else:
                candidate = copy.deepcopy(self.payload)
                parent = candidate
                for part in path[:-1]:
                    parent = parent[part]
                if path:
                    parent[path[-1]] = None if value is not None else "invented"
                    mutations.append(candidate)

        visit(self.payload, ())
        self.assertGreater(len(mutations), 250)
        for index, payload in enumerate(mutations):
            try:
                failures = validate_json_schema_instance(payload, schema)
                failures.extend(validate_report_payload(payload, expected_state=self.state))
            except Exception as exc:  # pragma: no cover - failure includes mutation index
                self.fail(f"mutation {index} raised {type(exc).__name__}: {exc}")
            self.assertTrue(failures, f"mutation {index} unexpectedly validated")

    def test_markdown_is_an_exact_digest_bound_render(self) -> None:
        text = self.report.to_json()
        digest = hashlib.sha256(text.encode()).hexdigest()
        markdown = self.report.to_markdown(json_digest=digest)
        self.assertEqual(render_markdown(self.payload, json_digest=digest), markdown)
        self.assertIn(f"Generated JSON SHA-256: `{digest}`", markdown)

    def test_p9_005_and_standing_rows_are_live_guarded(self) -> None:
        board = (ROOT / "docs/internal/testing/checklists/plc-verification-program/implementation-board.md").read_text()
        self.assertIn("VERIF-P9-005", REQUIRED_OPEN_ROWS)
        self.assertEqual([], validate_open_board_rows(board))
        self.assertTrue(validate_open_board_rows(board.replace("- [ ] `VERIF-P9-005`", "- [x] `VERIF-P9-005`")))

    def test_output_paths_reject_escape_absolute_symlink_and_collision(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as directory:
            base = Path(directory)
            link = base / "link"
            link.symlink_to(ROOT / "target", target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "workspace-relative"):
                _validated_output_path(ROOT, Path("../escape.json"), "JSON")
            with self.assertRaisesRegex(ValueError, "workspace-relative"):
                _validated_output_path(ROOT, Path("/tmp/escape.json"), "JSON")
            relative_link = link.relative_to(ROOT) / "report.json"
            with self.assertRaisesRegex(ValueError, "symlink"):
                _validated_output_path(ROOT, relative_link, "JSON")

    def test_production_at_rest_path_detects_markdown_corruption(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as directory:
            directory_path = Path(directory)
            json_path = directory_path / "report.json"
            markdown_path = directory_path / "report.md"
            json_text = self.report.to_json()
            json_path.write_text(json_text)
            digest = hashlib.sha256(json_text.encode()).hexdigest()
            markdown_path.write_text(self.report.to_markdown(json_digest=digest) + "tamper\n")
            with mock.patch(
                "scripts.verification.fuzz_program_validation.validate_source_revision",
                return_value=[],
            ):
                failures = validate_report_files(
                    ROOT,
                    json_path.relative_to(ROOT),
                    markdown_path.relative_to(ROOT),
                    Path(REPORT_SCHEMA_PATH),
                )
            self.assertTrue(any("Markdown" in item for item in failures), failures)

    def test_at_rest_validator_rejects_escaping_and_absolute_paths_before_read(self) -> None:
        for path in (Path("../escape.json"), Path("/tmp/escape.json")):
            with self.subTest(path=path):
                failures = validate_report_files(
                    ROOT,
                    path,
                    Path("target/gate-artifacts/verification/fuzz-program-audit.md"),
                    Path(REPORT_SCHEMA_PATH),
                )
                self.assertTrue(any("workspace-relative" in item for item in failures), failures)


if __name__ == "__main__":
    unittest.main()
