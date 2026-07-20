"""Tests for the report-only Phase 7 conformance alignment audit."""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts.verification.conformance_alignment import (
    COMMS_REVIEWED_SOURCE_PATHS,
    RUNNER_REVIEWED_SOURCE_PATHS,
    V1_CATEGORIES,
    V2_CATEGORIES,
    _coverage_gap,
    analyze_conformance_alignment,
)
from scripts.verification.conformance_alignment_contract import (
    validate_markdown_binding,
    validate_report_payload,
    validate_schema_contract,
)
from scripts.verification.conformance_alignment_live import (
    REPORT_SCHEMA_PATH,
    REQUIRED_OPEN_ROWS,
    _conformance_input_paths,
    build_live_conformance_alignment_state,
    validate_open_board_rows,
    validate_source_revision,
)
from scripts.verification.conformance_alignment_report import (
    ConformanceAlignmentProvenance,
    ConformanceAlignmentReport,
    render_markdown,
)
from scripts.verification.conformance_alignment_validation import validate_report_files
from scripts.verification.metadata_validator.constants import ROOT
from scripts.verification.metadata_validator.core import Validator
from scripts.verification.test_catalog_json_schema import validate_json_schema_instance


SCHEMA_PATH = ROOT / REPORT_SCHEMA_PATH
EXPECTED_SUMMARY = {
    "categories": 16,
    "v1_categories": 6,
    "v2_categories": 10,
    "cases": 21,
    "v1_cases": 11,
    "v2_cases": 10,
    "runtime_cases": 19,
    "compile_error_cases": 1,
    "connector_status_trace_cases": 1,
    "program_sources": 20,
    "expected_artifacts": 21,
    "missing_expected_artifacts": 0,
    "orphan_expected_artifacts": 0,
    "explicitly_linked_cases": 21,
    "unlinked_cases": 0,
    "coverage_gaps": 0,
}


def loaded_validator() -> Validator:
    validator = Validator()
    validator.load_records()
    validator.validate()
    if validator.failures:
        raise AssertionError([failure.message for failure in validator.failures])
    return validator


def fixture_root() -> tempfile.TemporaryDirectory[str]:
    temp = tempfile.TemporaryDirectory()
    root = Path(temp.name)
    shutil.copytree(ROOT / "conformance", root / "conformance")
    for relative in (
        ".github/workflows/ci.yml",
        ".gitignore",
        "docs/public/reference/conformance.md",
        *RUNNER_REVIEWED_SOURCE_PATHS,
        "crates/trust-runtime/src/connectors/mapping.rs",
    ):
        destination = root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT / relative, destination)
    return temp


def fixture_spec_sources() -> dict[str, dict]:
    return {
        "SPEC_CONFORMANCE_CONTRACT_001": {
            "id": "SPEC_CONFORMANCE_CONTRACT_001",
            "area": "release",
            "owner": "verification",
            "status": "mapped",
            "authority": "normative_product",
            "source_status": "active",
            "oracle_eligible": False,
            "visibility": "public",
            "path": "conformance/contract.md",
            "covers": [
                "conformance_categories",
                "summary_profiles",
                "result_classification",
                "deterministic_ordering",
                "generated_report_artifact_policy",
            ],
        }
    }


def analyze_fixture(
    root: Path,
    *,
    tests: dict[str, dict] | None = None,
    tracked_reports: tuple[str, ...] = ("conformance/reports/.gitkeep",),
) -> dict:
    return analyze_conformance_alignment(
        root,
        tests=tests or {},
        spec_sources=fixture_spec_sources(),
        tracked_report_paths=tracked_reports,
    )


def report_for(state) -> ConformanceAlignmentReport:
    return ConformanceAlignmentReport(
        provenance=ConformanceAlignmentProvenance(
            command=(
                "python3",
                "scripts/report_conformance_alignment.py",
                "--json-out",
                "target/gate-artifacts/verification/conformance-alignment.json",
                "--markdown-out",
                "docs/internal/testing/evidence/plc-verification-program/2026-07-11/p7-conformance-alignment.md",
                "--timestamp",
                state.timestamp,
            ),
            commit=state.commit,
            timestamp=state.timestamp,
            platform=state.platform,
            input_paths=state.input_paths,
            output_json="target/gate-artifacts/verification/conformance-alignment.json",
            output_markdown=(
                "docs/internal/testing/evidence/plc-verification-program/"
                "2026-07-11/p7-conformance-alignment.md"
            ),
        ),
        input_digest=state.input_digest,
        analysis=state.analysis,
    )


class ConformanceAlignmentAnalysisTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.validator = loaded_validator()
        cls.state = build_live_conformance_alignment_state(
            ROOT,
            timestamp="2026-07-11T12:00:00+02:00",
        )
        cls.analysis = cls.state.analysis

    def test_live_census_is_exhaustive_and_explicitly_mapped(self) -> None:
        self.assertEqual(EXPECTED_SUMMARY, self.analysis["summary"])
        self.assertEqual(21, len(self.analysis["cases"]))
        self.assertEqual(16, len(self.analysis["categories"]))
        self.assertEqual([], self.analysis["unlinked_case_ids"])
        self.assertTrue(
            all(row["catalog_test_id"] is not None for row in self.analysis["cases"])
        )
        self.assertTrue(
            all(row["invariant_ids"] for row in self.analysis["cases"])
        )

    def test_contract_categories_and_case_counts_match_written_profiles(self) -> None:
        counts = {row["category"]: row["case_count"] for row in self.analysis["categories"]}

        self.assertEqual(6, len(V1_CATEGORIES))
        self.assertEqual(10, len(V2_CATEGORIES))
        self.assertEqual(3, counts["timers"])
        self.assertEqual(2, counts["arithmetic"])
        self.assertEqual(2, counts["init_reset"])
        self.assertEqual(2, counts["memory_map"])
        self.assertTrue(
            all(counts[category] == 1 for category in V2_CATEGORIES)
        )

    def test_v2_cases_bind_explicit_eligible_semantic_oracles(self) -> None:
        v2_cases = [row for row in self.analysis["cases"] if row["profile"] == "v2"]

        self.assertEqual(10, len(v2_cases))
        self.assertEqual([], self.analysis["coverage_gaps"])
        self.assertTrue(all(row["oracle_ref"] for row in v2_cases))
        self.assertTrue(all(row["expected_result"] for row in v2_cases))

    def test_partial_category_mapping_remains_a_coverage_gap(self) -> None:
        cases = [
            {
                "category": "strings",
                "case_id": "linked_case",
                "expected_artifact_path": "conformance/expected/linked.json",
                "invariant_ids": ["IEC_STRING_001"],
            },
            {
                "category": "strings",
                "case_id": "unlinked_case",
                "expected_artifact_path": "conformance/expected/unlinked.json",
                "invariant_ids": [],
            },
        ]

        gap = _coverage_gap("strings", cases)

        self.assertEqual("missing", gap["invariant_mapping_state"])

    def test_catalog_join_is_discovery_id_only_and_never_lexical(self) -> None:
        with fixture_root() as temp:
            root = Path(temp)
            baseline = analyze_fixture(root)
            timer = next(row for row in baseline["cases"] if row["category"] == "timers")
            decoy = {
                "TEST_DECOY": {
                    "id": "TEST_DECOY",
                    "subject_kind": "generated_test",
                    "discovery_id": "DISC_DECOY",
                    "discovery_source_kind": "conformance_case",
                    "name": timer["case_id"],
                    "path": timer["manifest_path"],
                    "invariants": ["IEC_TIMER_001"],
                }
            }
            after_decoy = analyze_fixture(root, tests=decoy)
            self.assertEqual(0, after_decoy["summary"]["explicitly_linked_cases"])

            decoy["TEST_DECOY"]["discovery_id"] = timer["discovery_id"]
            decoy["TEST_DECOY"]["invariants"] = []
            registered = analyze_fixture(root, tests=decoy)
            registered_row = next(
                row for row in registered["cases"] if row["case_id"] == timer["case_id"]
            )
            self.assertEqual("TEST_DECOY", registered_row["catalog_test_id"])
            self.assertEqual([], registered_row["invariant_ids"])
            self.assertEqual(0, registered["summary"]["explicitly_linked_cases"])

            decoy["TEST_DECOY"]["invariants"] = ["IEC_TIMER_001"]
            exact = analyze_fixture(root, tests=decoy)
            linked = next(row for row in exact["cases"] if row["case_id"] == timer["case_id"])
            self.assertEqual("TEST_DECOY", linked["catalog_test_id"])
            self.assertEqual(["IEC_TIMER_001"], linked["invariant_ids"])

    def test_live_catalog_explicitly_links_every_conformance_case(self) -> None:
        validator = loaded_validator()
        analysis = analyze_conformance_alignment(
            ROOT,
            tests=validator.tests,
            spec_sources=validator.spec_sources,
            tracked_report_paths=("conformance/reports/.gitkeep",),
        )

        self.assertEqual(21, analysis["summary"]["explicitly_linked_cases"])
        self.assertEqual([], analysis["unlinked_case_ids"])

    def test_v2_oracle_assessment_rejects_ineligible_or_incomplete_catalog_claims(self) -> None:
        catalog = copy.deepcopy(self.validator.tests)
        v2_id = next(
            test_id
            for test_id, row in catalog.items()
            if row.get("name") == "cfm_strings_slice_concat_001"
        )

        catalog[v2_id]["oracle_ref"] = "SPEC_CONFORMANCE_CONTRACT_001"
        with self.assertRaisesRegex(ValueError, "oracle-eligible"):
            analyze_conformance_alignment(
                ROOT,
                tests=catalog,
                spec_sources=self.validator.spec_sources,
                tracked_report_paths=("conformance/reports/.gitkeep",),
            )

        catalog = copy.deepcopy(self.validator.tests)
        catalog[v2_id]["expected_result"] = ""
        with self.assertRaisesRegex(ValueError, "expected_result"):
            analyze_conformance_alignment(
                ROOT,
                tests=catalog,
                spec_sources=self.validator.spec_sources,
                tracked_report_paths=("conformance/reports/.gitkeep",),
            )

    def test_exact_discovery_join_rejects_source_kind_path_and_name_rebinding(self) -> None:
        with fixture_root() as temp:
            root = Path(temp)
            baseline = analyze_fixture(root)
            timer = next(row for row in baseline["cases"] if row["category"] == "timers")
            record = {
                "id": "TEST_REBOUND",
                "subject_kind": "generated_test",
                "discovery_id": timer["discovery_id"],
                "discovery_source_kind": "conformance_case",
                "name": timer["case_id"],
                "path": timer["manifest_path"],
                "invariants": ["IEC_TIMER_001"],
            }
            for field, value in (
                ("discovery_source_kind", "rust_integration_test"),
                ("path", "crates/trust-runtime/tests/bytecode_container.rs"),
                ("name", "header_validation"),
            ):
                with self.subTest(field=field):
                    tampered = copy.deepcopy(record)
                    tampered[field] = value
                    with self.assertRaisesRegex(ValueError, "catalog identity"):
                        analyze_fixture(root, tests={"TEST_REBOUND": tampered})

    def test_duplicate_category_mismatch_and_unknown_category_fail_closed(self) -> None:
        for label in ("duplicate", "path_mismatch", "unknown"):
            with self.subTest(label=label), fixture_root() as temp:
                root = Path(temp)
                source = next((root / "conformance/cases/timers").glob("*/manifest.toml"))
                if label == "duplicate":
                    destination = root / "conformance/cases/timers/duplicate/manifest.toml"
                    destination.parent.mkdir(parents=True)
                    shutil.copy2(source, destination)
                elif label == "path_mismatch":
                    text = source.read_text().replace('category = "timers"', 'category = "edges"')
                    source.write_text(text)
                else:
                    text = source.read_text().replace('category = "timers"', 'category = "unknown"')
                    source.write_text(text)
                with self.assertRaises(ValueError):
                    analyze_fixture(root)

    def test_missing_orphan_and_mismatched_expected_artifacts_fail_closed(self) -> None:
        for label in ("missing", "orphan", "mismatch"):
            with self.subTest(label=label), fixture_root() as temp:
                root = Path(temp)
                expected = next((root / "conformance/expected/timers").glob("*.json"))
                if label == "missing":
                    expected.unlink()
                elif label == "orphan":
                    (expected.parent / "cfm_timers_orphan_case_999.json").write_text("{}\n")
                else:
                    payload = json.loads(expected.read_text())
                    payload["case_id"] = "cfm_timers_wrong_case_999"
                    expected.write_text(json.dumps(payload))
                with self.assertRaises(ValueError):
                    analyze_fixture(root)

    def test_symlinked_expected_artifact_fails_as_a_symlink(self) -> None:
        with fixture_root() as temp:
            root = Path(temp)
            expected_files = sorted((root / "conformance/expected/timers").glob("*.json"))
            expected = expected_files[0]
            target = expected_files[1]
            expected.unlink()
            expected.symlink_to(target.name)

            with self.assertRaisesRegex(ValueError, "symlink"):
                analyze_fixture(root)

    def test_comms_case_is_scripted_in_process_and_rejects_network_shape(self) -> None:
        comms = self.analysis["comms_determinism"]

        self.assertEqual("connector_status_trace", comms["kind"])
        self.assertEqual("scripted_in_process", comms["execution_mode"])
        self.assertEqual(8, comms["scripted_steps"])
        self.assertFalse(comms["program_source_present"])
        self.assertFalse(comms["live_socket_dependency"])
        self.assertRegex(comms["reviewed_source_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual(
            list(COMMS_REVIEWED_SOURCE_PATHS),
            comms["reviewed_source_paths"],
        )

        with fixture_root() as temp:
            root = Path(temp)
            manifest = next((root / "conformance/cases/comms_determinism").glob("*/manifest.toml"))
            manifest.write_text(manifest.read_text() + '\nendpoint = "tcp://127.0.0.1:1234"\n')
            with self.assertRaisesRegex(ValueError, "network field"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            execution = (
                root
                / "crates/trust-runtime/src/bin/trust-runtime/conformance/execution.rs"
            )
            execution.write_text(
                execution.read_text().replace(
                    "fn execute_connector_status_trace_case(case: &CaseDefinition) -> anyhow::Result<CaseArtifact> {",
                    "fn execute_connector_status_trace_case(case: &CaseDefinition) -> anyhow::Result<CaseArtifact> {\n    call_external_io()?;",
                )
            )
            with self.assertRaisesRegex(ValueError, "source digest"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            mapping = root / "crates/trust-runtime/src/connectors/mapping.rs"
            mapping.write_text(mapping.read_text() + "\n// projection drift\n")
            with self.assertRaisesRegex(ValueError, "source digest"):
                analyze_fixture(root)

    def test_complete_runner_source_contract_is_reviewed_and_provenance_bound(self) -> None:
        contract = self.analysis["contract"]
        self.assertEqual(
            list(RUNNER_REVIEWED_SOURCE_PATHS),
            contract["reviewed_runner_source_paths"],
        )
        self.assertEqual(
            {
                *RUNNER_REVIEWED_SOURCE_PATHS,
                *COMMS_REVIEWED_SOURCE_PATHS,
                "crates/trust-runtime/tests/conformance_cli_command.rs",
            }
            - set(self.state.input_paths),
            set(),
        )
        mutations = (
            (
                "crates/trust-runtime/src/bin/trust-runtime/conformance.rs",
                'include!("conformance/models.rs");',
                '// include!("conformance/models.rs");',
            ),
            (
                "crates/trust-runtime/src/bin/trust-runtime/conformance/models.rs",
                '"comms_determinism",',
                '"comms_determinism_drift",',
            ),
            (
                "crates/trust-runtime/src/bin/trust-runtime/conformance/discovery.rs",
                "for category in CATEGORIES {",
                "for category in V1_CATEGORIES {",
            ),
            (
                "crates/trust-runtime/src/bin/trust-runtime/conformance/runner.rs",
                "cases.sort_by(|left, right| left.id.cmp(&right.id));",
                "// case ordering removed",
            ),
            (
                "crates/trust-runtime/src/bin/trust-runtime/conformance/runner.rs",
                "Ok(expected) if expected == artifact.payload => {",
                "Ok(_expected) => {",
            ),
        )
        for relative, old, new in mutations:
            with self.subTest(relative=relative, old=old), fixture_root() as temp:
                root = Path(temp)
                source = root / relative
                self.assertIn(old, source.read_text())
                source.write_text(source.read_text().replace(old, new, 1))
                with self.assertRaisesRegex(ValueError, "runner source digest"):
                    analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            runner = (
                root
                / "crates/trust-runtime/src/bin/trust-runtime/conformance/runner.rs"
            )
            fragment = "cases.sort_by(|left, right| left.id.cmp(&right.id));"
            runner.write_text(runner.read_text().replace(fragment, "// " + fragment, 1))
            digest = hashlib.sha256()
            for relative in RUNNER_REVIEWED_SOURCE_PATHS:
                digest.update(relative.encode())
                digest.update(b"\0")
                digest.update((root / relative).read_bytes())
                digest.update(b"\0")
            with patch(
                "scripts.verification.conformance_alignment.RUNNER_REVIEWED_SOURCE_DIGEST",
                "sha256:" + digest.hexdigest(),
            ), self.assertRaisesRegex(ValueError, "runner contract"):
                analyze_fixture(root)

    def test_ignored_generated_reports_do_not_contaminate_source_provenance(self) -> None:
        with fixture_root() as temp:
            root = Path(temp)
            generated = root / "conformance/reports/stale-summary.json"
            generated.write_text("{}\n")
            paths = _conformance_input_paths(root)
            self.assertIn("conformance/reports/.gitkeep", paths)
            self.assertNotIn("conformance/reports/stale-summary.json", paths)

    def test_public_contract_and_ci_artifact_posture_are_machine_bound(self) -> None:
        contract = self.analysis["contract"]
        publication = self.analysis["publication"]

        self.assertEqual("SPEC_CONFORMANCE_CONTRACT_001", contract["spec_source_id"])
        self.assertFalse(contract["oracle_eligible"])
        self.assertTrue(contract["public_page_bound"])
        self.assertEqual("conformance-suite", publication["ci_artifact_name"])
        self.assertRegex(publication["ci_job_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertEqual("ci_artifact_only", publication["generated_report_policy"])
        self.assertEqual(["conformance/reports/.gitkeep"], publication["tracked_report_files"])
        self.assertFalse(publication["public_page_embeds_generated_result"])
        self.assertRegex(
            publication["public_page_digest"],
            r"^sha256:[0-9a-f]{64}$",
        )

        for field, value in (
            ("area", "protocols"),
            ("owner", "attacker"),
            ("status", "planned"),
            ("covers", ["unrelated"]),
        ):
            with self.subTest(field=field), fixture_root() as temp:
                source = fixture_spec_sources()["SPEC_CONFORMANCE_CONTRACT_001"]
                source[field] = value
                with self.assertRaisesRegex(ValueError, field):
                    analyze_conformance_alignment(
                        Path(temp),
                        tests={},
                        spec_sources={"SPEC_CONFORMANCE_CONTRACT_001": source},
                        tracked_report_paths=("conformance/reports/.gitkeep",),
                    )

        with fixture_root() as temp:
            root = Path(temp)
            workflow = root / ".github/workflows/ci.yml"
            workflow.write_text(workflow.read_text().replace("name: conformance-suite", "name: drift"))
            with self.assertRaisesRegex(ValueError, "job digest"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            workflow = root / ".github/workflows/ci.yml"
            workflow.write_text(
                workflow.read_text().replace(
                    "  conformance:\n",
                    "  conformance:\n    # reviewed job drift\n",
                    1,
                )
            )
            with self.assertRaisesRegex(ValueError, "job digest"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            ignore = root / ".gitignore"
            ignore.write_text(
                ignore.read_text().replace(
                    "conformance/reports/*\n!conformance/reports/.gitkeep\n",
                    "",
                )
            )
            with self.assertRaisesRegex(ValueError, "ignore"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            ignore = root / ".gitignore"
            ignore.write_text(
                ignore.read_text() + "\n!conformance/reports/special.json\n"
            )
            with self.assertRaisesRegex(ValueError, "later negation"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            workflow = root / ".github/workflows/ci.yml"
            text = workflow.read_text().replace(
                "      - name: Upload conformance artifacts\n"
                "        if: ${{ always() }}\n"
                "        uses: actions/upload-artifact@v7",
                "      - name: Upload conformance artifacts\n"
                "        if: ${{ always() }}\n"
                "        # uses: actions/upload-artifact@v7",
                1,
            )
            workflow.write_text(text)
            start = text.index("  conformance:\n")
            end = text.index("\n  architecture-safety:\n", start)
            digest = "sha256:" + hashlib.sha256(text[start:end].encode()).hexdigest()
            with patch(
                "scripts.verification.conformance_alignment.CI_JOB_REVIEWED_DIGEST",
                digest,
            ), self.assertRaisesRegex(ValueError, "upload action"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            workflow = root / ".github/workflows/ci.yml"
            text = workflow.read_text().replace(
                "      - name: Upload conformance artifacts\n"
                "        if: ${{ always() }}\n"
                "        uses: actions/upload-artifact@v7",
                "      - name: Upload conformance artifacts\n"
                "        if: ${{ always() }}\n"
                "        run: |\n"
                "          uses: actions/upload-artifact@v7",
                1,
            )
            workflow.write_text(text)
            start = text.index("  conformance:\n")
            end = text.index("\n  architecture-safety:\n", start)
            digest = "sha256:" + hashlib.sha256(text[start:end].encode()).hexdigest()
            with patch(
                "scripts.verification.conformance_alignment.CI_JOB_REVIEWED_DIGEST",
                digest,
            ), self.assertRaisesRegex(ValueError, "upload action"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            public_page = root / "docs/public/reference/conformance.md"
            public_page.write_text(
                public_page.read_text() + "\n## Latest Results\n\n21 passed, 0 failed.\n"
            )
            with self.assertRaisesRegex(ValueError, "generated result"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            public_page = root / "docs/public/reference/conformance.md"
            public_page.write_text(public_page.read_text() + "\nReviewed prose drift.\n")
            with self.assertRaisesRegex(ValueError, "public page digest"):
                analyze_fixture(root)

        with fixture_root() as temp:
            root = Path(temp)
            with self.assertRaisesRegex(ValueError, "generated report"):
                analyze_fixture(
                    root,
                    tracked_reports=(
                        "conformance/reports/.gitkeep",
                        "conformance/reports/committed-summary.json",
                    ),
                )

    def test_standing_mapping_guards_remain_open(self) -> None:
        board = (
            ROOT
            / "docs/internal/testing/checklists/plc-verification-program/implementation-board.md"
        ).read_text()
        self.assertEqual([], validate_open_board_rows(board))
        self.assertNotIn("VERIF-P7-002", REQUIRED_OPEN_ROWS)
        for row_id in REQUIRED_OPEN_ROWS:
            with self.subTest(row_id=row_id):
                tampered = board.replace(f"- [ ] `{row_id}`", f"- [x] `{row_id}`", 1)
                self.assertIn(row_id, "\n".join(validate_open_board_rows(tampered)))


class ConformanceAlignmentReportContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.state = build_live_conformance_alignment_state(
            ROOT,
            timestamp="2026-07-11T12:00:00+02:00",
        )
        cls.report = report_for(cls.state)

    def test_schema_payload_and_markdown_are_closed_and_exact(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())
        payload = self.report.to_dict()
        json_bytes = self.report.to_json().encode()
        markdown = self.report.to_markdown(
            json_digest=hashlib.sha256(json_bytes).hexdigest()
        )

        self.assertEqual([], validate_schema_contract(schema))
        self.assertEqual(
            [],
            validate_report_payload(payload, expected_analysis=self.state.analysis),
        )
        self.assertEqual([], validate_json_schema_instance(payload, schema))
        self.assertEqual([], validate_markdown_binding(payload, json_bytes, markdown))
        self.assertTrue(
            validate_markdown_binding(
                payload,
                json_bytes,
                markdown.replace("Coverage Gaps", "Coverage Debt", 1),
            )
        )

    def test_schema_honesty_enums_and_boundaries_are_drift_pinned(self) -> None:
        schema = json.loads(SCHEMA_PATH.read_text())

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["case"]["properties"]["category"]["enum"].append(
            "invented"
        )
        self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["coverage_gap"]["properties"]["category"][
            "enum"
        ].append("timers")
        self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["boundaries"]["properties"][
            "report_creates_proof"
        ] = {"const": True}
        self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["contract"]["properties"]["authority"] = {
            "enum": ["normative_product", "public_claim"]
        }
        self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["comms_determinism"]["properties"][
            "live_socket_dependency"
        ] = {"type": "boolean"}
        self.assertTrue(validate_schema_contract(tampered))

        for field in ("case_present", "expected_artifact_present"):
            with self.subTest(field=field):
                tampered = copy.deepcopy(schema)
                tampered["$defs"]["coverage_gap"]["properties"][field] = {
                    "type": "boolean"
                }
                self.assertTrue(validate_schema_contract(tampered))

        tampered = copy.deepcopy(schema)
        tampered["$defs"]["category"]["properties"]["category"]["enum"].append(
            "invented"
        )
        self.assertTrue(validate_schema_contract(tampered))

    def test_semantic_tamper_fails_after_recomputed_summary_and_markdown(self) -> None:
        payload = copy.deepcopy(self.report.to_dict())
        payload["cases"][0]["catalog_test_id"] = "TEST_INVENTED"
        payload["cases"][0]["invariant_ids"] = ["IEC_TIMER_001"]
        payload["summary"]["explicitly_linked_cases"] = 1
        payload["summary"]["unlinked_cases"] = 20
        payload["unlinked_case_ids"] = payload["unlinked_case_ids"][1:]

        failures = validate_report_payload(
            payload,
            expected_analysis=self.state.analysis,
        )
        self.assertTrue(
            any("current conformance alignment" in failure for failure in failures),
            failures,
        )

    def test_at_rest_validator_recomputes_live_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            json_path = Path(temp) / "report.json"
            markdown_path = Path(temp) / "report.md"
            json_path.write_text(self.report.to_json())
            digest = hashlib.sha256(json_path.read_bytes()).hexdigest()
            markdown_path.write_text(self.report.to_markdown(json_digest=digest))
            with patch(
                "scripts.verification.conformance_alignment_validation.validate_source_revision",
                return_value=[],
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
                v2_case = next(row for row in payload["cases"] if row["profile"] == "v2")
                v2_case["oracle_ref"] = "SPEC_CONFORMANCE_CONTRACT_001"
                text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
                json_path.write_text(text)
                markdown_path.write_text(
                    render_markdown(
                        payload,
                        json_digest=hashlib.sha256(text.encode()).hexdigest(),
                    )
                )
                failures = validate_report_files(
                    ROOT,
                    json_path,
                    markdown_path,
                    SCHEMA_PATH,
                    allow_external_test_outputs=True,
                )
            self.assertTrue(failures)

    def test_source_revision_requires_clean_full_sha(self) -> None:
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_source_revision(ROOT, "dirty:" + "a" * 40, ()),
        )
        self.assertIn(
            "commit must identify a clean full Git SHA",
            validate_source_revision(ROOT, "a" * 12, ()),
        )


if __name__ == "__main__":
    unittest.main()
