"""Tests for the catalog-evidence-only test refactor assessment."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.verification.test_refactor_assessment import build_test_refactor_assessment
from scripts.verification.test_refactor_duplicates import analyze_duplicate_fixtures
from scripts.verification.test_refactor_file_metrics import analyze_test_files


def fact(stable_id: str, path: str, *, source_kind: str = "rust_integration_test") -> dict:
    return {
        "stable_id": stable_id,
        "path": path,
        "source_kind": source_kind,
        "name": "misleading_network_boundary_fuzz_name",
        "package": "trust-runtime",
        "ignore_state": "active",
    }


def catalog(
    test_id: str,
    discovery_id: str,
    path: str,
    *,
    area: str = "bytecode_vm",
    test_class: str = "unit",
    invariants: list[str] | None = None,
    suite_tiers: list[str] | None = None,
    duration_class: str = "fast",
    **extra: object,
) -> dict:
    return {
        "id": test_id,
        "subject_kind": "generated_test",
        "discovery_id": discovery_id,
        "path": path,
        "area": area,
        "test_class": test_class,
        "invariants": invariants if invariants is not None else ["INV_A"],
        "suite_tiers": suite_tiers or [],
        "duration_class": duration_class,
        **extra,
    }


class TestFileMetrics(unittest.TestCase):
    def test_large_is_inclusive_and_mixed_purpose_uses_reviewed_intent_only(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "tests").mkdir()
            (root / "tests/a.rs").write_text("one\ntwo\nthree\n")
            (root / "tests/b.rs").write_text("1\n2\n3\n4\n5\n")
            facts = [
                fact("DISC_A1", "tests/a.rs"),
                fact("DISC_A2", "tests/a.rs"),
                fact("DISC_B1", "tests/b.rs"),
            ]
            records = [
                catalog("TEST_A1", "DISC_A1", "tests/a.rs"),
                catalog(
                    "TEST_A2",
                    "DISC_A2",
                    "tests/a.rs",
                    area="runtime_safety",
                    test_class="integration",
                ),
            ]

            rows = analyze_test_files(
                root=root,
                scanner_facts=facts,
                catalog_records=records,
                large_file_threshold=5,
            )

        by_path = {row["path"]: row for row in rows}
        self.assertNotIn("large_file", by_path["tests/a.rs"]["candidate_reasons"])
        self.assertIn("reviewed_mapping_diversity", by_path["tests/a.rs"]["candidate_reasons"])
        self.assertEqual(by_path["tests/a.rs"]["reviewed_areas"], ["bytecode_vm", "runtime_safety"])
        self.assertIn("large_file", by_path["tests/b.rs"]["candidate_reasons"])
        self.assertNotIn("reviewed_mapping_diversity", by_path["tests/b.rs"]["candidate_reasons"])
        self.assertEqual(by_path["tests/b.rs"]["reviewed_areas"], [])
        self.assertEqual(by_path["tests/b.rs"]["unmapped_fact_count"], 1)

    def test_scanner_path_must_be_a_contained_committed_file(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            outside = root.parent / "outside-test-refactor.rs"
            outside.write_text("test\n")
            self.addCleanup(outside.unlink, missing_ok=True)
            with self.assertRaisesRegex(ValueError, "safe workspace-relative"):
                analyze_test_files(
                    root=root,
                    scanner_facts=[fact("DISC_ESCAPE", "../outside-test-refactor.rs")],
                    catalog_records=[],
                    large_file_threshold=10,
                )


class TestDuplicateAssessment(unittest.TestCase):
    def test_exact_and_whitespace_normalized_groups_are_content_based(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "tests").mkdir()
            (root / "tests/a.st").write_text("A := 1;\nB := 2;\n")
            (root / "tests/b.st").write_text("A := 1;\nB := 2;\n")
            (root / "tests/c.st").write_text(" A   := 1; B := 2; \n")
            analysis = analyze_duplicate_fixtures(
                root=root,
                paths=["tests/c.st", "tests/a.st", "tests/b.st"],
                catalog_records=[],
            )

        self.assertEqual(
            analysis["exact_groups"][0]["paths"],
            ["tests/a.st", "tests/b.st"],
        )
        self.assertEqual(
            analysis["whitespace_normalized_groups"][0]["paths"],
            ["tests/a.st", "tests/b.st", "tests/c.st"],
        )

    def test_malformed_overlap_requires_explicit_reviewed_class_ids(self) -> None:
        records = [
            catalog(
                "TEST_MAGIC_A",
                "DISC_A",
                "tests/a.rs",
                malformed_input_class_ids=["bad_magic", "invalid_checksum"],
            ),
            catalog(
                "TEST_MAGIC_B",
                "DISC_B",
                "tests/b.rs",
                malformed_input_class_ids=["bad_magic"],
            ),
            catalog("TEST_NAME_ONLY", "DISC_C", "tests/bad_magic.rs"),
        ]
        analysis = analyze_duplicate_fixtures(
            root=Path("."),
            paths=[],
            catalog_records=records,
        )

        self.assertEqual(
            analysis["malformed_class_overlaps"],
            [
                {
                    "malformed_input_class_id": "bad_magic",
                    "paths": ["tests/a.rs", "tests/b.rs"],
                    "test_ids": ["TEST_MAGIC_A", "TEST_MAGIC_B"],
                }
            ],
        )

    def test_case_inputs_use_exact_values_and_same_table_typed_shapes(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "verification/cases").mkdir(parents=True)
            (root / "verification/cases/a.toml").write_text(
                """
[[case]]
id = "CASE_A_INT_1"
input = { value = 1 }
[[case]]
id = "CASE_A_INT_2"
input = { value = 2 }
[[case]]
id = "CASE_A_BOOL"
input = { value = true }
""".lstrip()
            )
            (root / "verification/cases/b.toml").write_text(
                """
[[case]]
id = "CASE_B_INT_1"
input = { value = 1 }
[[case]]
id = "CASE_B_INT_3"
input = { value = 3 }
""".lstrip()
            )
            records = [
                {
                    "id": "TEST_CASE_A",
                    "path": "verification/cases/a.toml",
                    "case_file": "verification/cases/a.toml",
                },
                {
                    "id": "TEST_CASE_A_SECOND_OWNER",
                    "path": "scripts/run_a.py",
                    "case_file": "verification/cases/a.toml",
                },
                {
                    "id": "TEST_CASE_B",
                    "path": "verification/cases/b.toml",
                    "case_file": "verification/cases/b.toml",
                },
            ]
            analysis = analyze_duplicate_fixtures(
                root=root,
                paths=[],
                catalog_records=records,
            )

        self.assertEqual(
            analysis["exact_case_input_groups"][0]["case_ids"],
            ["CASE_A_INT_1", "CASE_B_INT_1"],
        )
        structural = analysis["same_table_structural_shape_groups"]
        self.assertEqual(len(structural), 2)
        self.assertEqual(structural[0]["case_file"], "verification/cases/a.toml")
        self.assertEqual(
            structural[0]["case_ids"],
            ["CASE_A_INT_1", "CASE_A_INT_2"],
        )
        self.assertNotIn("CASE_A_BOOL", structural[0]["case_ids"])
        self.assertEqual(structural[1]["case_file"], "verification/cases/b.toml")
        self.assertEqual(
            analysis["shared_case_file_reference_groups"],
            [
                {
                    "case_file": "verification/cases/a.toml",
                    "record_paths": ["scripts/run_a.py", "verification/cases/a.toml"],
                    "test_ids": ["TEST_CASE_A", "TEST_CASE_A_SECOND_OWNER"],
                }
            ],
        )
        self.assertEqual(
            analysis["case_file_paths"],
            ["verification/cases/a.toml", "verification/cases/b.toml"],
        )
        self.assertEqual(analysis["free_form_body_similarity"], "not_assessed")


class TestAssessment(unittest.TestCase):
    def test_multi_invariant_claim_is_candidate_until_catalog_supports_dimensions(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "tests").mkdir()
            (root / "tests/a.rs").write_text("test\n")
            facts = [fact("DISC_A", "tests/a.rs"), fact("DISC_B", "tests/a.rs")]
            records = [
                catalog(
                    "TEST_NO_CLAIM",
                    "DISC_C",
                    "tests/a.rs",
                    invariants=[],
                ),
                catalog(
                    "TEST_BROAD",
                    "DISC_A",
                    "tests/a.rs",
                    invariants=["INV_A", "INV_B"],
                ),
                catalog(
                    "TEST_DIMENSIONED",
                    "DISC_B",
                    "tests/a.rs",
                    invariants=["INV_A", "INV_B"],
                    coverage_dimensions={
                        "INV_A": ["magic rejection"],
                        "INV_B": ["checksum rejection"],
                    },
                ),
            ]
            result = build_test_refactor_assessment(
                root=root,
                scanner_facts=[*facts, fact("DISC_C", "tests/a.rs")],
                catalog_records=records,
                suites=[],
                vscode_registration_audit={},
                large_file_threshold=100,
            )

        self.assertEqual(
            [row for row in result["broad_claim_assessment"] if row["result"].startswith("candidate")],
            [
                {
                    "coverage_dimensions": [],
                    "invariant_count": 2,
                    "invariants": ["INV_A", "INV_B"],
                    "path": "tests/a.rs",
                    "result": "candidate_missing_coverage_dimensions",
                    "test_id": "TEST_BROAD",
                },
                {
                    "coverage_dimensions": [],
                    "invariant_count": 2,
                    "invariants": ["INV_A", "INV_B"],
                    "path": "tests/a.rs",
                    "result": "candidate_missing_coverage_dimensions",
                    "test_id": "TEST_DIMENSIONED",
                },
            ],
        )
        self.assertEqual(
            next(
                row["result"]
                for row in result["broad_claim_assessment"]
                if row["test_id"] == "TEST_NO_CLAIM"
            ),
            "no_invariant_claim",
        )

    def test_vscode_registration_large_files_and_duration_debt_remain_visible(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            path = "editors/vscode/src/test/suite/large.test.ts"
            (root / path).parent.mkdir(parents=True)
            (root / path).write_text("a\nb\nc\n")
            other = "tests/uncataloged.rs"
            (root / other).parent.mkdir()
            (root / other).write_text("test\n")
            facts = [
                fact("DISC_VS", path, source_kind="vscode_test"),
                fact("DISC_UNMAPPED", other),
            ]
            records = [
                catalog(
                    "TEST_VS",
                    "DISC_VS",
                    path,
                    test_class="vscode_extension",
                    suite_tiers=[],
                    duration_class="fast",
                ),
                {
                    "id": "TEST_ARTIFACT",
                    "subject_kind": "case_table_artifact",
                    "path": "verification/cases/x.toml",
                    "area": "bytecode_vm",
                    "test_class": "metadata_validation",
                    "invariants": ["INV_A"],
                    "suite_tiers": ["nightly"],
                    "duration_class": "slow",
                },
            ]
            suites = [
                {"id": "pr", "placeholder": True, "commands": ["metadata"]},
                {"id": "nightly", "commands": []},
            ]
            audit = {
                "index_path": "editors/vscode/src/test/suite/index.ts",
                "test_files": [path],
                "registered_files": [path],
                "entries": [
                    {
                        "specifier": "./large.test",
                        "source_line": 20,
                        "resolved_path": path,
                    }
                ],
                "unregistered_files": [],
                "missing_targets": [],
                "duplicate_targets": [],
                "fact_count": 1,
                "unregistered_fact_files": [],
                "diagnostics": [],
                "is_clean": True,
            }
            result = build_test_refactor_assessment(
                root=root,
                scanner_facts=facts,
                catalog_records=records,
                suites=suites,
                vscode_registration_audit=audit,
                large_file_threshold=3,
            )

        self.assertEqual(
            [row["path"] for row in result["vscode_registration"]["files"] if row["large_candidate"]],
            [path],
        )
        duration = result["duration_classification"]
        classified = [row for row in duration["scanner_facts"] if row["classification_source"] == "hand_catalog"]
        unclassified = [row for row in duration["scanner_facts"] if row["classification_source"] == "unclassified"]
        self.assertEqual(len(classified), 1)
        self.assertEqual([row["discovery_id"] for row in unclassified], ["DISC_UNMAPPED"])
        self.assertEqual(duration["placeholder_suite_ids"], ["pr"])
        self.assertEqual(duration["commandless_suite_ids"], ["nightly"])
        self.assertEqual(duration["unassigned_tier_test_ids"], ["TEST_VS"])
        self.assertEqual(
            duration["artifact_catalog_records"],
            [
                {
                    "duration_class": "slow",
                    "path": "verification/cases/x.toml",
                    "subject_kind": "case_table_artifact",
                    "suite_tiers": ["nightly"],
                    "test_id": "TEST_ARTIFACT",
                },
            ],
        )

    def test_no_refactor_proposal_is_supported_only_without_observed_signals(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "tests").mkdir()
            (root / "tests/bytecode_container.rs").write_text("one\ntwo\n")
            (root / "tests/large.rs").write_text("one\ntwo\nthree\n")
            facts = [
                fact("DISC_BYTECODE", "tests/bytecode_container.rs"),
                fact("DISC_LARGE", "tests/large.rs"),
            ]
            records = [catalog("TEST_BYTECODE", "DISC_BYTECODE", "tests/bytecode_container.rs")]
            proposals = [
                {
                    "id": "PROPOSAL_BYTECODE",
                    "source_paths": ["tests/bytecode_container.rs"],
                    "disposition": "no_refactor_needed",
                    "rationale": "No reviewed assessment signal supports a move or split.",
                },
                {
                    "id": "PROPOSAL_LARGE_NO",
                    "source_paths": ["tests/large.rs"],
                    "disposition": "no_refactor_needed",
                    "rationale": "Contradicted by the size signal.",
                },
                {
                    "id": "PROPOSAL_LARGE_YES",
                    "source_paths": ["tests/large.rs"],
                    "disposition": "move",
                    "rationale": "The committed file meets the reviewed size threshold.",
                },
            ]
            result = build_test_refactor_assessment(
                root=root,
                scanner_facts=facts,
                catalog_records=records,
                suites=[],
                vscode_registration_audit={},
                large_file_threshold=3,
                proposals=proposals,
            )

        by_id = {row["proposal_id"]: row for row in result["proposal_evaluations"]}
        self.assertTrue(by_id["PROPOSAL_BYTECODE"]["supported"])
        self.assertEqual(by_id["PROPOSAL_BYTECODE"]["observed_signals"], [])
        self.assertFalse(by_id["PROPOSAL_LARGE_NO"]["supported"])
        self.assertFalse(by_id["PROPOSAL_LARGE_YES"]["supported"])
        self.assertEqual(
            by_id["PROPOSAL_LARGE_YES"]["observed_signals"],
            ["large_file:tests/large.rs"],
        )

    def test_result_is_canonical_regardless_of_input_order(self) -> None:
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            (root / "tests").mkdir()
            (root / "tests/a.rs").write_text("a\n")
            (root / "tests/b.rs").write_text("b\n")
            facts = [fact("DISC_B", "tests/b.rs"), fact("DISC_A", "tests/a.rs")]
            records = [
                catalog("TEST_B", "DISC_B", "tests/b.rs"),
                catalog("TEST_A", "DISC_A", "tests/a.rs"),
            ]
            kwargs = {
                "root": root,
                "suites": [],
                "vscode_registration_audit": {},
                "large_file_threshold": 10,
            }
            first = build_test_refactor_assessment(
                scanner_facts=facts,
                catalog_records=records,
                **kwargs,
            )
            second = build_test_refactor_assessment(
                scanner_facts=list(reversed(facts)),
                catalog_records=list(reversed(records)),
                **kwargs,
            )

        self.assertEqual(first, second)


if __name__ == "__main__":
    unittest.main()
