"""Report and at-rest tests for the Phase 10 mutation program."""

from __future__ import annotations

import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from .metadata_validator.constants import ROOT
from .mutation_program_cli import report_main
from .mutation_program_live import (
    REPORT_SCHEMA_PATH,
    REQUIRED_OPEN_ROWS,
    build_live_mutation_program_state,
    _normalize_focused_result,
    validate_open_board_rows,
    validate_timestamp,
    _survivor_resolution_paths,
)
from .mutation_program_report import MutationProgramReport, render_markdown
from .mutation_program_report_contract import (
    validate_report_payload,
    validate_schema_contract,
)
from .test_catalog_json_schema import validate_json_schema_instance
from .mutation_program_validation import validate_report_files


class MutationProgramReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        (ROOT / "target").mkdir(exist_ok=True)
        cls.state = build_live_mutation_program_state(ROOT, require_clean_commit=False)
        cls.report = MutationProgramReport.from_state(cls.state)
        cls.payload = cls.report.payload

    def test_report_distinguishes_measured_from_planned(self) -> None:
        self.assertEqual(6, self.payload["summary"]["shards"])
        self.assertEqual(5, self.payload["summary"]["measured_shards"])
        self.assertEqual(1, self.payload["summary"]["planned_shards"])
        self.assertEqual(6, self.payload["summary"]["measured_mutants"])
        self.assertEqual(6, self.payload["summary"]["caught"])
        self.assertEqual(0, self.payload["summary"]["survived"])
        self.assertEqual([], self.payload["survivors"])
        for result in self.payload["shards"][0]["results"]:
            self.assertEqual(
                next(
                    mutation["association_ids"]
                    for mutation in self.payload["shards"][0]["mutations"]
                    if mutation["id"] == result["id"]
                ),
                result["association_ids"],
            )
            self.assertNotIn("related_case_ids", result)
            self.assertNotIn("survivor_action", result)
        for row in self.payload["shards"][1:5]:
            self.assertEqual("measured", row["execution_status"])
            self.assertEqual(1, len(row["results"]))
            self.assertEqual("caught", row["results"][0]["result"])
        self.assertEqual("planned", self.payload["shards"][5]["execution_status"])
        self.assertEqual([], self.payload["shards"][5]["results"])

    def test_focused_artifact_results_are_normalized_without_raw_logs(self) -> None:
        result = _normalize_focused_result(
            {
                "id": "MUTANT_X",
                "source_file": "crates/example.rs",
                "function": "example",
                "genre": "FnValue",
                "replacement": "false",
                "generated_mutant_name": "generated",
                "build_command": ["cargo", "test", "--no-run"],
                "build_exit_status": 0,
                "build_stdout": "x" * 5000,
                "build_stderr": "build-end",
                "build_timed_out": False,
                "test_command": ["cargo", "test", "case"],
                "test_exit_status": 101,
                "test_stdout": "y" * 5000,
                "test_stderr": "test-end",
                "test_timed_out": False,
                "duration_seconds": 1.0,
                "result": "caught",
                "association_ids": ["DISC_X"],
            }
        )
        self.assertLessEqual(len(result["build_output_tail"]), 4000)
        self.assertTrue(result["build_output_tail"].endswith("build-end"))
        self.assertTrue(result["test_output_tail"].endswith("test-end"))
        self.assertNotIn("build_stdout", result)
        self.assertNotIn("test_stderr", result)

    def test_program_boundaries_create_no_proof_coverage_or_release_claim(self) -> None:
        self.assertEqual(
            "validated_bytecode_pilot_and_four_source_execution_artifacts",
            self.payload["scope"]["measured_basis"],
        )
        self.assertEqual(
            "single_file_list_and_bound_source_execution_artifacts",
            self.payload["tool"]["selection_mode"],
        )
        boundaries = self.payload["boundaries"]
        self.assertFalse(boundaries["report_creates_proof"])
        self.assertFalse(boundaries["report_creates_invariant_coverage"])
        self.assertFalse(boundaries["report_closes_spec_gaps"])
        self.assertFalse(boundaries["report_is_release_evidence"])
        self.assertFalse(boundaries["ci_enforcement_changed"])
        self.assertEqual(0, self.payload["coverage"]["runs"])

    def test_semantic_tamper_fails_live_recompute(self) -> None:
        corrupted = copy.deepcopy(self.payload)
        corrupted["shards"][1]["owner"] = "invented-owner"
        failures = validate_report_payload(corrupted, expected_state=self.state)
        self.assertTrue(any("live Phase 10" in item for item in failures), failures)

    def test_survivor_requires_resolved_allowed_disposition_and_durable_ref(self) -> None:
        corrupted = copy.deepcopy(self.payload)
        survivor = {
            "shard_id": corrupted["shards"][0]["id"],
            "mutation_id": corrupted["shards"][0]["results"][0]["id"],
            "owner": "verification",
            "action": "consider_later",
            "resolution_status": "open",
            "rationale": "future work",
            "resolution_ref": None,
        }
        corrupted["survivors"] = [survivor]
        corrupted["summary"]["survived"] = 1
        failures = validate_report_payload(corrupted)
        self.assertTrue(any("survivor" in item for item in failures), failures)

    def test_infrastructure_and_impossible_phase_outcomes_are_rejected(self) -> None:
        corrupted = copy.deepcopy(self.payload)
        result = corrupted["shards"][0]["results"][0]
        result["result"] = "caught"
        result["build_exit_status"] = -9
        failures = validate_report_payload(corrupted)
        self.assertTrue(any("infrastructure" in item or "derived" in item for item in failures), failures)
        result["build_exit_status"] = 1
        result["test_exit_status"] = 1
        failures = validate_report_payload(corrupted)
        self.assertTrue(any("after build" in item or "derived" in item for item in failures), failures)

    def test_delivered_binary_result_requires_digest_and_direct_execution(self) -> None:
        corrupted = copy.deepcopy(self.payload)
        row = corrupted["shards"][5]
        row["execution_status"] = "measured"
        row["results"] = copy.deepcopy(corrupted["shards"][0]["results"][:1])
        row["delivered_build_confirmation"] = None
        failures = validate_report_payload(corrupted)
        self.assertTrue(any("delivered" in item for item in failures), failures)

    def test_json_is_canonical_and_markdown_is_exact_digest_bound_render(self) -> None:
        text = self.report.to_json()
        self.assertEqual(json.dumps(json.loads(text), indent=2, sort_keys=True) + "\n", text)
        digest = hashlib.sha256(text.encode()).hexdigest()
        markdown = self.report.to_markdown(json_digest=digest)
        self.assertEqual(render_markdown(self.payload, json_digest=digest), markdown)
        self.assertIn(digest, markdown)

    def test_report_schema_can_represent_future_survivor_counts(self) -> None:
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        summary = schema["$defs"]["summary"]["properties"]
        for field in ("caught", "survived", "unviable", "timeout", "error"):
            self.assertEqual("integer", summary[field]["type"])
            self.assertEqual(0, summary[field]["minimum"])
            self.assertNotIn("const", summary[field])

    def test_generic_report_represents_a_resolved_future_survivor(self) -> None:
        future = copy.deepcopy(self.payload)
        result = future["shards"][0]["results"][0]
        result["test_exit_status"] = 0
        result["result"] = "survived"
        future["summary"]["caught"] -= 1
        future["summary"]["survived"] += 1
        future["survivors"] = [
            {
                "shard_id": future["shards"][0]["id"],
                "mutation_id": result["id"],
                "owner": "trust-runtime",
                "action": "add_test",
                "resolution_status": "resolved",
                "rationale": "A focused regression is required before acceptance.",
                "resolution_ref": "verification/mutation-program.toml",
            }
        ]
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        self.assertEqual([], validate_json_schema_instance(future, schema))
        self.assertEqual([], validate_report_payload(future))

    def test_output_paths_cannot_overwrite_bound_inputs(self) -> None:
        corrupted = copy.deepcopy(self.payload)
        input_path = "verification/mutation-program.toml"
        corrupted["output_paths"]["json"] = input_path
        corrupted["command"][3] = input_path
        failures = validate_report_payload(corrupted)
        self.assertTrue(any("bound input" in item for item in failures), failures)

        original = (ROOT / input_path).read_bytes()
        with mock.patch(
            "scripts.verification.mutation_program_cli.build_live_mutation_program_state",
            return_value=self.state,
        ):
            exit_code = report_main(
                [
                    "--json-out",
                    input_path,
                    "--markdown-out",
                    "target/gate-artifacts/verification/collision-probe.md",
                    "--timestamp",
                    self.state.timestamp,
                ]
            )
        self.assertEqual(2, exit_code)
        self.assertEqual(original, (ROOT / input_path).read_bytes())
        self.assertFalse(
            (ROOT / "target/gate-artifacts/verification/collision-probe.md").exists()
        )

    def test_invalid_shapes_never_raise(self) -> None:
        mutations = []
        for key in self.payload:
            candidate = copy.deepcopy(self.payload)
            candidate.pop(key)
            mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"] = None
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["invariant_ids"] = [{}]
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["associated_tests"][0]["id"] = {}
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["survivors"] = [
            {
                "shard_id": {},
                "mutation_id": {},
                "owner": "owner",
                "action": "add_test",
                "resolution_status": "resolved",
                "rationale": "rationale",
                "resolution_ref": "verification/mutation-program.toml",
            }
        ]
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["results"] = None
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["id"] = {}
        candidate["shards"][0]["results"][0]["id"] = {}
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["execution_status"] = {}
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["delivered_build_requirement"] = []
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["shards"][0]["associated_tests"][0]["id_kind"] = {}
        mutations.append(candidate)
        candidate = copy.deepcopy(self.payload)
        candidate["survivors"] = [
            {
                "shard_id": "shard",
                "mutation_id": "mutant",
                "owner": "owner",
                "action": {},
                "resolution_status": "resolved",
                "rationale": "rationale",
                "resolution_ref": "verification/mutation-program.toml",
            }
        ]
        mutations.append(candidate)
        for payload in mutations:
            try:
                failures = validate_report_payload(payload, expected_state=self.state)
            except Exception as exc:  # pragma: no cover
                self.fail(f"invalid payload raised {type(exc).__name__}: {exc}")
            self.assertTrue(failures)

    def test_survivor_resolution_refs_enter_the_provenance_closure(self) -> None:
        self.assertEqual(
            {
                "docs/internal/testing/example.md",
                "verification/mutation-program.toml",
            },
            _survivor_resolution_paths(
                {
                    "survivor_resolutions": [
                        {"resolution_ref": "docs/internal/testing/example.md"},
                        {"resolution_ref": "verification/mutation-program.toml"},
                    ]
                }
            ),
        )

    def test_hostile_schema_shapes_never_raise(self) -> None:
        schemas = []
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        schema["required"] = [{}]
        schema["$defs"]["survivor"]["properties"]["action"]["enum"] = [{}]
        schemas.append(schema)
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        schema["properties"]["shards"] = []
        schemas.append(schema)
        schema = json.loads((ROOT / REPORT_SCHEMA_PATH).read_text())
        schema["$defs"]["summary"]["properties"]["caught"] = []
        schemas.append(schema)
        for schema in schemas:
            try:
                failures = validate_schema_contract(schema)
            except Exception as exc:  # pragma: no cover
                self.fail(f"hostile schema raised {type(exc).__name__}: {exc}")
            self.assertTrue(failures)

    def test_timestamp_and_remaining_standing_rows_are_guarded(self) -> None:
        with self.assertRaisesRegex(ValueError, "timezone"):
            validate_timestamp("2026-07-11T20:00:00")
        board = (ROOT / "docs/internal/testing/checklists/plc-verification-program/implementation-board.md").read_text()
        self.assertNotIn("VERIF-P10-001", REQUIRED_OPEN_ROWS)
        self.assertNotIn("VERIF-P10-003", REQUIRED_OPEN_ROWS)
        self.assertNotIn("VERIF-P16-001", REQUIRED_OPEN_ROWS)
        self.assertNotIn("VERIF-P16-000D", REQUIRED_OPEN_ROWS)
        self.assertEqual([], validate_open_board_rows(board))
        row = REQUIRED_OPEN_ROWS[0]
        self.assertTrue(validate_open_board_rows(board.replace(f"- [ ] `{row}`", f"- [x] `{row}`")))

    def test_production_at_rest_path_rejects_markdown_tamper(self) -> None:
        with tempfile.TemporaryDirectory(dir=ROOT / "target") as directory:
            base = Path(directory)
            json_path = base / "report.json"
            markdown_path = base / "report.md"
            json_text = self.report.to_json()
            json_path.write_text(json_text)
            digest = hashlib.sha256(json_text.encode()).hexdigest()
            markdown_path.write_text(self.report.to_markdown(json_digest=digest) + "tamper\n")
            with mock.patch(
                "scripts.verification.mutation_program_validation.validate_source_revision",
                return_value=[],
            ):
                failures = validate_report_files(
                    ROOT,
                    json_path.relative_to(ROOT),
                    markdown_path.relative_to(ROOT),
                    Path(REPORT_SCHEMA_PATH),
                )
            self.assertTrue(any("Markdown" in item for item in failures), failures)


if __name__ == "__main__":
    unittest.main()
