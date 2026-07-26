from __future__ import annotations

import hashlib
import json
import tempfile
import unittest
from pathlib import Path

from scripts.verification.frozen_hir_catalog_plans import generate_path_plans
from scripts.verification.frozen_hir_family_batch import (
    derive_hir_family_selection,
    hir_family_selection_digest,
)


def _write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content.strip() + "\n")


def _digest(path: Path) -> str:
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


class FrozenHirCatalogPlansTests(unittest.TestCase):
    def test_generates_path_views_and_binds_assertions_by_id_not_order(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = "crates/trust-hir/tests/generic_types.rs"
            _write(root / source, "#[test]\nfn generic_case() {}")
            _write(
                root / "verification/test-catalog-denominator.toml",
                f"""
                [[reviews]]
                discovery_id = "DISC_GENERIC"
                discovery_source_kind = "rust_integration_test"
                path = "{source}"
                name = "generic_case"
                ignore_state = "not_ignored"
                disposition = "reviewed_nonmapping"
                """,
            )
            _write(root / "verification/test-catalog.toml", "tests = []")
            _write(
                root / "verification/invariants/compiler_iec/IEC_GENERIC_001.toml",
                """
                id = "IEC_GENERIC_001"
                [[behavior]]
                outcome = "accept_value"
                [[behavior]]
                outcome = "reject"
                """,
            )
            selection = derive_hir_family_selection(root)
            metadata = {
                "source_commit": "a" * 40,
                "command": "cargo nextest run -p trust-hir",
                "reviewed_nonmapping_count": 1,
                "catalog_mapped_count": 0,
                "selected_test_count": 1,
                "selection_sha256": hir_family_selection_digest(selection),
                "denominator_sha256": _digest(
                    root / "verification/test-catalog-denominator.toml"
                ),
                "catalog_sha256": _digest(
                    root / "verification/test-catalog.toml"
                ),
                "paths": [
                    {
                        "path": source,
                        "profile": "hir-remaining",
                        "target_kind": "test",
                        "binary_id": "trust-hir::generic_types",
                        "test_prefix": "",
                        "reviewed_nonmapping_count": 1,
                        "catalog_mapped_count": 0,
                        "projected_raw_results_path": (
                            "target/evidence/generic.jsonl"
                        ),
                    }
                ],
            }
            inventory = {
                "inventories": {
                    "DISC_GENERIC": {
                        "path": source,
                        "test_name": "generic_case",
                        "assertions": [
                            {"assertion_id": "ASSERT_TWO"},
                            {"assertion_id": "ASSERT_ONE"},
                        ],
                    }
                }
            }
            assignments = {
                "schema_version": 2,
                "assignments": [
                    {
                        "path": source,
                        "name": "generic_case",
                        "assertion_assignments": [
                            {
                                "assertion_id": "ASSERT_ONE",
                                "invariant_id": "IEC_GENERIC_001",
                                "behavior_index": 0,
                            },
                            {
                                "assertion_id": "ASSERT_TWO",
                                "invariant_id": "IEC_GENERIC_001",
                                "behavior_index": 1,
                            },
                        ],
                    }
                ],
            }

            index = generate_path_plans(
                root=root,
                metadata=metadata,
                inventory_document=inventory,
                assignments_document=assignments,
                output_dir=root / "target/plans",
                reviewer_ids=("codex-main", "codex-spec-scout"),
                reviewed_date="2026-07-26",
                timestamp="2026-07-26T14:00:00Z",
            )

            self.assertEqual(1, index["expected_count"])
            self.assertEqual(1, len(index["paths"]))
            record = index["paths"][0]
            plan = json.loads((root / record["plan_path"]).read_text())
            projected_inventory = json.loads(
                (root / record["inventory_path"]).read_text()
            )
            self.assertEqual(1, plan["expected_count"])
            self.assertTrue(plan["selection_digest"].startswith("sha256:"))
            self.assertEqual(
                {"IEC_GENERIC_001"},
                set(plan["invariant_behavior_digests"]),
            )
            self.assertEqual(
                [
                    {
                        "name": "generic_case",
                        "assertions": [
                            {
                                "assertion_id": "ASSERT_ONE",
                                "invariant_id": "IEC_GENERIC_001",
                                "behavior_index": 0,
                            },
                            {
                                "assertion_id": "ASSERT_TWO",
                                "invariant_id": "IEC_GENERIC_001",
                                "behavior_index": 1,
                            },
                        ],
                    }
                ],
                plan["assertion_assignments"],
            )
            self.assertEqual(
                {"DISC_GENERIC"},
                set(projected_inventory["inventories"]),
            )

    def test_rejects_incomplete_semantic_assignment(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = "crates/trust-hir/tests/generic_types.rs"
            _write(root / source, "#[test]\nfn generic_case() {}")
            _write(
                root / "verification/test-catalog-denominator.toml",
                f"""
                [[reviews]]
                discovery_id = "DISC_GENERIC"
                discovery_source_kind = "rust_integration_test"
                path = "{source}"
                name = "generic_case"
                ignore_state = "not_ignored"
                disposition = "reviewed_nonmapping"
                """,
            )
            _write(root / "verification/test-catalog.toml", "tests = []")
            selection = derive_hir_family_selection(root)
            with self.assertRaisesRegex(ValueError, "assignments do not equal"):
                generate_path_plans(
                    root=root,
                    metadata={
                        "source_commit": "a" * 40,
                        "command": "cargo nextest run -p trust-hir",
                        "reviewed_nonmapping_count": 1,
                        "catalog_mapped_count": 0,
                        "selected_test_count": 1,
                        "selection_sha256": hir_family_selection_digest(
                            selection
                        ),
                        "denominator_sha256": _digest(
                            root
                            / "verification/test-catalog-denominator.toml"
                        ),
                        "catalog_sha256": _digest(
                            root / "verification/test-catalog.toml"
                        ),
                        "paths": [
                            {
                                "path": source,
                                "profile": "hir-remaining",
                                "target_kind": "test",
                                "binary_id": "trust-hir::generic_types",
                                "test_prefix": "",
                                "reviewed_nonmapping_count": 1,
                                "catalog_mapped_count": 0,
                                "projected_raw_results_path": (
                                    "target/evidence/generic.jsonl"
                                ),
                            }
                        ],
                    },
                    inventory_document={
                        "inventories": {
                            "DISC_GENERIC": {
                                "path": source,
                                "test_name": "generic_case",
                                "assertions": [
                                    {"assertion_id": "ASSERT_ONE"},
                                ],
                            }
                        }
                    },
                    assignments_document={
                        "schema_version": 2,
                        "assignments": [],
                    },
                    output_dir=root / "target/plans",
                    reviewer_ids=("codex-main",),
                    reviewed_date="2026-07-26",
                    timestamp="2026-07-26T14:00:00Z",
                )


if __name__ == "__main__":
    unittest.main()
