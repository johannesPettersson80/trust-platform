"""Focused tests for complete campaign routing and command safety."""

from __future__ import annotations

import unittest
from pathlib import Path

from scripts.verification.test_catalog_campaign import (
    campaign_commands,
    partition_campaign_facts,
    rank_traceable_rust_ground_truth,
    select_pilot_ids,
)
from scripts.verification.test_catalog_common import make_fact


def _fact(source_kind: str, name: str, *, ignored: bool = False):
    return make_fact(
        source_kind=source_kind,
        name=name,
        path=f"tests/{name}.test",
        line=1,
        package=None,
        command_hint=f"run {name}",
        command_hint_authority="exact",
        discovery_confidence="fixture",
        ignore_state="ignored" if ignored else "not_ignored",
    )


class TestCatalogCampaignTests(unittest.TestCase):
    def test_every_live_source_kind_has_exactly_one_campaign_route(self) -> None:
        facts = [
            _fact("rust_unit_test", "rust"),
            _fact("rust_integration_test", "integration"),
            _fact("structured_text_test", "st"),
            _fact("conformance_case", "conformance"),
            _fact("vscode_test", "vscode"),
            _fact("fuzz_target", "fuzz"),
            _fact("gate_script", "gate"),
            _fact("github_workflow_job", "workflow"),
            _fact("vscode_test", "ignored", ignored=True),
        ]

        routes = partition_campaign_facts(facts)

        self.assertEqual(4, len(routes["structural"]))
        self.assertEqual(2, len(routes["rust"]))
        self.assertEqual(2, len(routes["direct"]))
        self.assertEqual(1, len(routes["vscode"]))
        self.assertEqual(len(facts), sum(len(rows) for rows in routes.values()))

    def test_full_campaign_contains_no_final_full_test_gate(self) -> None:
        commands = campaign_commands(
            root=Path("/repo"),
            artifact_root=Path("/artifacts"),
            cargo_target_dir=Path("/target"),
            llvm_tools_dir=Path("/llvm"),
            temporary_dir=Path("/temporary"),
            mode="full",
            pilot_ids=None,
            resume=True,
        )

        flattened = [argument for _, command in commands for argument in command]
        self.assertNotIn("test-all", flattened)
        self.assertIn("--require-complete-denominator", flattened)
        self.assertIn("--all-rust", flattened)
        self.assertIn("--all-direct", flattened)
        self.assertIn("--all-vscode", flattened)
        direct_command = dict(commands)["direct trace"]
        self.assertEqual("2", direct_command[direct_command.index("--jobs") + 1])

    def test_pilot_samples_existing_ground_truth_and_each_adapter_kind(self) -> None:
        rust = [_fact("rust_unit_test", f"rust_{index}") for index in range(20)]
        direct = [
            _fact("conformance_case", f"conformance_{index}")
            for index in range(10)
        ]
        direct.append(_fact("structured_text_test", "st"))
        vscode = [_fact("vscode_test", f"vscode_{index}") for index in range(3)]
        facts = [*rust, *direct, *vscode]
        indexed = {fact.stable_id: fact for fact in facts}
        routes = partition_campaign_facts(facts)
        mapped_ids = {
            fact.stable_id
            for fact in [*rust, *direct[:-1], *vscode]
        }
        catalog = {
            discovery_id: {
                "subject_kind": "generated_test",
                "discovery_id": discovery_id,
            }
            for discovery_id in mapped_ids
        }

        selected = select_pilot_ids(
            routes=routes,
            facts=indexed,
            catalog_tests=catalog,
        )

        self.assertEqual(16, len(selected["rust"]))
        self.assertEqual(9, len(selected["direct"]))
        self.assertEqual(3, len(selected["vscode"]))
        self.assertIn(direct[-1].stable_id, selected["direct"])

    def test_positive_rust_sample_joins_catalog_and_code_invariant_mappings(
        self,
    ) -> None:
        rust_a = _fact("rust_unit_test", "rust_a")
        rust_b = _fact("rust_unit_test", "rust_b")
        facts = {
            rust_a.stable_id: rust_a,
            rust_b.stable_id: rust_b,
        }
        catalog = {
            "TEST_A": {
                "subject_kind": "generated_test",
                "discovery_id": rust_a.stable_id,
                "area": "area_a",
                "invariants": ["INV_A"],
            },
            "TEST_B": {
                "subject_kind": "generated_test",
                "discovery_id": rust_b.stable_id,
                "area": "area_b",
                "invariants": ["INV_B"],
            },
        }
        code_facts = {
            "CODE_A": {"path": rust_a.path},
            "CODE_B": {"path": "crates/other/src/lib.rs"},
        }
        code_mappings = {
            "CODE_A": {
                "disposition": "specified",
                "invariant_ids": ["INV_A"],
            },
            "CODE_B": {
                "disposition": "specified",
                "invariant_ids": ["INV_B"],
            },
        }

        selected = rank_traceable_rust_ground_truth(
            facts=facts,
            catalog_tests=catalog,
            code_mappings=code_mappings,
            code_facts=code_facts,
            limit=2,
        )

        self.assertEqual([rust_a.stable_id], selected)


if __name__ == "__main__":
    unittest.main()
