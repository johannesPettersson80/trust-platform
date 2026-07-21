"""Focused catchers for the P18-002 fact-level burn-down ledger."""

from __future__ import annotations

import copy
import json
import subprocess
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path

from .p18_zero_debt.burn_down_ledger import (
    ALL_BUDGETS,
    build_payload,
    canonical_json,
    _source_commit_failures,
    validate_payload,
)
from .test_catalog_json_schema import validate_json_schema_instance


@dataclass(frozen=True)
class CodeFixture:
    code_id: str
    source_kind: str = "python"
    path: str = "scripts/example.py"
    qualified_symbol: str = "example"
    item_kind: str = "python_function"
    area: str = "verification"
    owner: str = "verification"
    risk_signals: tuple[str, ...] = ()


@dataclass(frozen=True)
class TestFixture:
    stable_id: str
    source_kind: str = "gate_script"
    path: str = "scripts/p18_burn_down_ledger_gate.py"
    name: str = "p18_burn_down_ledger_gate"
    ignore_state: str = "not_ignored"


class BurnDownLedgerTests(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).resolve().parents[2]
        self.schema = json.loads(
            (root / "verification/schemas/p18-burn-down-ledger.schema.json").read_text()
        )
        code_id = "CODE_" + "A" * 64
        discovery_id = "DISC_" + "B" * 20
        matrix = {
            "areas": [
                {
                    "id": area,
                    "owner": "verification",
                    "risk_default": "false_status",
                    "path_globs": ["scripts/**"],
                }
                for area in (
                    "runtime_safety", "control_security", "bytecode_vm",
                    "compiler_iec", "protocols", "editor_safety", "hmi_ui",
                    "plcopen_devtools", "supply_chain_platform", "release",
                    "verification",
                )
            ],
            "code_areas": [
                {
                    "id": "fixture",
                    "match_kind": "path",
                    "area_ids": ["verification"],
                    "path_globs": ["scripts/**"],
                    "required_test_classes": [],
                    "suite_tiers": [],
                    "conditional_suite_tiers": [],
                }
            ],
        }
        self.payload = build_payload(
            code_facts={code_id: CodeFixture(code_id)},
            code_mappings={
                code_id: {
                    "disposition": "spec_gap",
                    "spec_gap_ref": "SPEC_GAP_FIXTURE",
                }
            },
            spec_gaps={
                "SPEC_GAP_FIXTURE": {
                    "resolution_status": "registered_backlog",
                }
            },
            test_facts=[TestFixture(discovery_id)],
            test_reviews={
                discovery_id: {
                    "disposition": "reviewed_nonmapping",
                    "reason_code": "gate_inventory_owned",
                }
            },
            test_catalog={},
            ignored_tests={},
            matrix=matrix,
            source_sha="c" * 40,
            input_paths=["fixture"],
            input_digest="sha256:" + "d" * 64,
        )

    def test_known_good_is_closed_canonical_and_preserves_every_blocker(self) -> None:
        self.assertEqual([], validate_json_schema_instance(self.payload, self.schema))
        self.assertEqual(
            [],
            validate_payload(
                self.payload,
                expected_code_ids={"CODE_" + "A" * 64},
                expected_test_ids={"DISC_" + "B" * 20},
            ),
        )
        self.assertIn("registered_backlog", self.payload["code_slices"][0]["blockers"])
        self.assertEqual(list(ALL_BUDGETS), self.payload["test_slices"][0]["budget_refs"])
        self.assertEqual(canonical_json(self.payload), canonical_json(json.loads(canonical_json(self.payload))))

    def test_schema_rejects_missing_and_unknown_nested_fields(self) -> None:
        missing = copy.deepcopy(self.payload)
        missing["code_slices"][0].pop("owner")
        self.assertTrue(validate_json_schema_instance(missing, self.schema))
        unknown = copy.deepcopy(self.payload)
        unknown["test_slices"][0]["waiver"] = True
        self.assertTrue(validate_json_schema_instance(unknown, self.schema))

    def test_validator_rejects_omission_duplication_and_counter_drift(self) -> None:
        omitted = copy.deepcopy(self.payload)
        omitted["code_slices"] = []
        failures = validate_payload(
            omitted,
            expected_code_ids={"CODE_" + "A" * 64},
            expected_test_ids={"DISC_" + "B" * 20},
        )
        self.assertTrue(any("omitted" in failure for failure in failures))
        duplicated = copy.deepcopy(self.payload)
        duplicated["test_slices"].append(copy.deepcopy(duplicated["test_slices"][0]))
        self.assertTrue(any("duplicate test" in failure for failure in validate_payload(duplicated)))
        drifted = copy.deepcopy(self.payload)
        drifted["summary"]["code_spec_gap"] = 0
        self.assertTrue(any("code_spec_gap" in failure for failure in validate_payload(drifted)))

    def test_validator_rejects_waived_budget_environment_or_blocker(self) -> None:
        for field in ("budget_refs", "environment_refs", "blockers"):
            candidate = copy.deepcopy(self.payload)
            candidate["code_slices"][0][field] = []
            self.assertTrue(
                any(field in failure for failure in validate_payload(candidate)),
                field,
            )
        weakened = copy.deepcopy(self.payload)
        weakened["test_slices"][0]["budget_refs"] = list(ALL_BUDGETS[:-1])
        self.assertTrue(any("omits a proof budget" in failure for failure in validate_payload(weakened)))

    def test_source_sha_must_be_a_real_ancestor_with_identical_clean_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.name", "P18 Fixture"], check=True)
            subprocess.run(["git", "-C", str(root), "config", "user.email", "p18@example.invalid"], check=True)
            (root / "input.txt").write_text("frozen\n")
            subprocess.run(["git", "-C", str(root), "add", "input.txt"], check=True)
            subprocess.run(["git", "-C", str(root), "commit", "-qm", "freeze"], check=True)
            source_sha = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            self.assertEqual([], _source_commit_failures(root, source_sha, ["input.txt"]))
            self.assertTrue(_source_commit_failures(root, "f" * 40, ["input.txt"]))
            (root / "input.txt").write_text("drifted\n")
            self.assertTrue(
                any("input bytes" in failure for failure in _source_commit_failures(root, source_sha, ["input.txt"]))
            )


if __name__ == "__main__":
    unittest.main()
